//! Provides a formatter for the StreamIR to generate Solidity code
//! Requires the streamir-lib to parse a specification into StreamIR.
//! At the moment, only event-based streams are supported.
//!
#![forbid(unused_must_use)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

mod expressions;
mod functions;
mod guards;
pub mod interface;
mod memory;
mod statements;
mod types;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use clap::ValueEnum;
use interface::InterfaceConfig;
use memory::Memory;
use streamir_lib::{
    formatter::{
        files::{ConstructStore, FilesFormatter},
        StreamIrFormatter,
    },
    ir::{
        memory::{Parameter, StreamMemory},
        Accesses, OutputReference, StreamIr, StreamReference, Type,
    },
};

#[derive(Debug)]
/// The main struct holding the required information for generating Rust code
pub struct SolidityFormatter {
    config: InterfaceConfig,
    contract_name: String,
    streams_with_iteration: HashSet<OutputReference>,
    trigger_action: TriggerAction,
    trigger_function_mode: TriggerFunctionMode,
    sr2name: HashMap<StreamReference, String>,
    sr2memory: HashMap<StreamReference, StreamMemory>,
    sr2ty: HashMap<StreamReference, Type>,
    file: PathBuf,
    construct_store: ConstructStore<Self>,
    overwrite: bool,
    accesses: HashMap<StreamReference, Accesses>,
    triggers: HashMap<OutputReference, usize>,
    outputs_verdict: Vec<StreamReference>,
}

#[derive(ValueEnum, Debug, Clone, Copy, Eq, PartialEq)]
/// Defined the action if a trigger is violated in the contract
pub enum TriggerAction {
    /// Reverts the last input to the contract
    Revert,
    /// Creates an event if a trigger is violated. The event is shared between all triggers.
    EmitSingle,
    /// Creates an event if a trigger is violated. Each trigger has its own event.
    EmitMultiple,
}

#[derive(ValueEnum, Debug, Clone, Copy, Eq, PartialEq)]
/// The function interface for trigger functions
pub enum TriggerFunctionMode {
    /// One function for all triggers
    Single,
    /// All trigger share a common function
    Multiple,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[allow(missing_docs)]
/// Sorts the construct in the resulting files
///
/// Constructs with keys listed higher at the enum declaration below are sorted higher in the resulting output files.
pub enum RequirementKey {
    SolidityHeader,
    TupleDefinition(Vec<Type>),
    Memory(StreamReference),
    SpawnFunction(StreamReference),
    CloseFunction(StreamReference),
    EvalFunction(StreamReference, usize),
    ShiftFunction(StreamReference),
    #[allow(dead_code)]
    SyncFunction(StreamReference),
    GetFunction(StreamReference),
    TriggerEvent(usize),
    TriggerFunction(usize),
    UnpartialCycle,
    VerdictEvent,
    Cycle(String),
    CloseContract,
}

impl SolidityFormatter {
    /// Construct a new SolidityFormatter for the given StreamIR, writing to `file`, optionally overwriting existing files.
    ///
    /// The `config` arguments specifies the general interface for the resulting contract, while `trigger_action` the interaction if a trigger is violated
    /// that are included in the verdict.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        streamir: &StreamIr,
        config: InterfaceConfig,
        contract_name: String,
        trigger_action: TriggerAction,
        trigger_function_mode: TriggerFunctionMode,
        file: PathBuf,
        overwrite: bool,
        outputs_verdict: Vec<StreamReference>,
    ) -> Self {
        let streams_with_iteration = streamir
            .outputs()
            .filter(|sr| streamir.stmt.contains_interate(*sr))
            .collect();
        let (sr2name, sr2ty) = streamir
            .sr2memory
            .iter()
            .map(|(sr, m)| ((*sr, m.name.clone()), (*sr, m.ty.clone())))
            .collect();

        let sr2memory = streamir
            .sr2memory
            .iter()
            .map(|(sr, m)| (*sr, m.buffer.clone()))
            .collect();
        let triggers = streamir.triggers.clone();
        let construct_store = ConstructStore::default();
        Self {
            config,
            contract_name,
            streams_with_iteration,
            trigger_action,
            trigger_function_mode,
            sr2name,
            sr2memory,
            sr2ty,
            file,
            construct_store,
            overwrite,
            accesses: streamir.accesses.clone(),
            triggers,
            outputs_verdict,
        }
    }

    fn name(&self, sr: StreamReference) -> &str {
        &self.sr2name[&sr]
    }

    fn stream_memory(&self, sr: StreamReference) -> &StreamMemory {
        &self.sr2memory[&sr]
    }

    fn stream_parameter(&self, sr: StreamReference) -> Option<&[Parameter]> {
        self.stream_memory(sr).parameters()
    }

    fn stream_type(&self, sr: StreamReference) -> &Type {
        &self.sr2ty[&sr]
    }

    fn file(&self) -> &PathBuf {
        &self.file
    }
}

impl StreamIrFormatter for SolidityFormatter {
    type Return = Result<(), String>;

    fn id(&self) -> String {
        "solidity-formatter".into()
    }

    fn format(self, ir: StreamIr) -> Self::Return {
        self.add_requirement_string(
            self.file(),
            RequirementKey::SolidityHeader,
            format!(
                "pragma solidity ^0.8.24;\ncontract {} {{",
                self.contract_name
            ),
        );
        self.add_requirement_string(self.file(), RequirementKey::CloseContract, "}".into());
        for stream in ir.streams() {
            self.add_requirement(Memory(stream));
        }
        self.format_cycle_functions(&self.config, ir)?;

        self.generate_files().unwrap();
        Ok(())
    }
}
