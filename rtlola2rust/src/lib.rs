//! Provides a formatter for the StreamIR to generate Rust code
//! Requires the streamir-lib to parse a specification into StreamIR.

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

mod api;
mod constructs;
mod error;
mod expressions;
mod guards;
mod io;
mod main_function;
mod memory;
mod names;
mod schedule;
mod statements;
mod types;
mod windows;

use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Duration};

use api::{AcceptEventFunction, MonitorConstructor};
use constructs::{
    EnumDefinition, FunctionDefinition, FunctionVisibility, RequirementKey, RustType,
    StructDefinition,
};
use include_dir::{include_dir, Dir, DirEntry};
use itertools::Itertools;
pub use main_function::MainFunction;
use memory::StreamMemoryStruct;
use schedule::{DeadlineEnum, QueueStruct, StreamReferenceEnum};
use statements::CycleFunction;
use streamir_lib::{
    formatter::{
        files::{ConstructStore, ConstructWriteError, FilesFormatter},
        StreamIrFormatter,
    },
    ir::{
        expressions::Expr,
        memory::{Memory, Parameter, StreamMemory},
        windows::Window,
        LocalFreq, LocalFreqRef, StreamIr, StreamReference, Type, WindowReference,
    },
};
use tera::Tera;
use windows::WindowMemory;

#[derive(Debug)]
/// The main struct holding the required information for generating Rust code
pub struct RustFormatter {
    sr2name: HashMap<StreamReference, String>,
    sr2ty: HashMap<StreamReference, Type>,
    sr2parameters: HashMap<StreamReference, Vec<Parameter>>,
    sr2memory: HashMap<StreamReference, StreamMemory>,
    lfreq2lfreq: HashMap<LocalFreqRef, LocalFreq>,
    wref2window: HashMap<WindowReference, Window>,
    static_deadlines: Vec<Duration>,
    dynamic_deadlines: Vec<Duration>,
    construct_store: ConstructStore<Self>,
    output_folder: PathBuf,
    // Whether to overwrite existing files
    overwrite: bool,
    // for each expression we generate a separate function
    // this counter is incremented for each expression that is generated
    expr_counter: Mutex<HashMap<(Expr, Option<StreamReference>), usize>>,
    num_exprs: Mutex<usize>,
    tera: Tera,
    main: MainFunction,
    verdict_streams: Vec<StreamReference>,
}

impl StreamIrFormatter for RustFormatter {
    type Return = Result<(), ConstructWriteError>;

    fn id(&self) -> String {
        "rust-formatter".into()
    }

    fn format(self, ir: StreamIr) -> Self::Return {
        let StreamIr { stmt, .. } = ir;
        let _ = self.call_self_function::<_, String>(CycleFunction(stmt), &[]);
        let _ = self.call_self_function::<_, String>(AcceptEventFunction, &[]);
        self.require_struct(MonitorStruct);
        self.main.insert_requirement(&self);
        self.generate_files()
    }
}

impl RustFormatter {
    /// Construct a new RustFormatter for the given StreamIR, writing the files to `output_folder`, optionally overwriting existing files.
    ///
    /// The `main` arguments specifies the kind of main function to generate, while `verdict_streams` contains a list of (unparameterized) stream references
    /// that are included in the verdict.
    pub fn new(
        ir: &StreamIr,
        output_folder: PathBuf,
        overwrite: bool,
        main: MainFunction,
        verdict_streams: Vec<StreamReference>,
    ) -> Self {
        let (sr2name, sr2ty, sr2parameters, sr2memory) = ir
            .sr2memory
            .iter()
            .map(|(sr, m)| {
                let Memory { buffer, ty, name } = m;
                (
                    (*sr, name.clone()),
                    (*sr, ty.clone()),
                    (*sr, m.parameters().unwrap_or(&[]).to_owned()),
                    (*sr, buffer.clone()),
                )
            })
            .multiunzip();
        let mut tera = Tera::default();
        static TEMPLATE_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");

        for entry in TEMPLATE_DIR.find("**/*").unwrap() {
            if let DirEntry::File(template) = entry {
                tera.add_raw_template(
                    template.path().to_str().unwrap(),
                    template.contents_utf8().unwrap(),
                )
                .unwrap();
            }
        }

        let (static_deadlines, dynamic_deadlines) = ir.all_periodic_pacings();
        let dynamic_deadlines = dynamic_deadlines
            .into_iter()
            .map(|d| d.dur)
            .unique()
            .collect();

        let lfreq2lfreq = ir.lref2lfreq.clone();
        let wref2window = ir.wref2window.clone();

        Self {
            sr2name,
            sr2ty,
            sr2parameters,
            sr2memory,
            static_deadlines,
            dynamic_deadlines,
            lfreq2lfreq,
            wref2window,
            construct_store: ConstructStore::default(),
            output_folder,
            expr_counter: Mutex::new(HashMap::new()),
            num_exprs: Mutex::new(0),
            tera,
            overwrite,
            main,
            verdict_streams,
        }
    }

    pub(crate) fn streams(&self) -> impl Iterator<Item = StreamReference> + '_ {
        self.sr2name.keys().sorted().copied()
    }

    pub(crate) fn inputs(&self) -> impl Iterator<Item = StreamReference> + '_ {
        self.sr2name
            .keys()
            .filter(|o| matches!(o, StreamReference::In(_)))
            .sorted()
            .copied()
    }

    pub(crate) fn outputs(&self) -> impl Iterator<Item = StreamReference> + '_ {
        self.sr2name
            .keys()
            .filter(|o| matches!(o, StreamReference::Out(_)))
            .sorted()
            .copied()
    }

    pub(crate) fn stream_type(&self, sr: StreamReference) -> RustType {
        self.lola_stream_type(sr).clone().into()
    }

    pub(crate) fn lola_stream_type(&self, sr: StreamReference) -> &Type {
        &self.sr2ty[&sr]
    }

    pub(crate) fn stream_memory(&self, sr: StreamReference) -> &StreamMemory {
        &self.sr2memory[&sr]
    }

    pub(crate) fn stream_parameter(&self, sr: StreamReference) -> &[Parameter] {
        &self.sr2parameters[&sr]
    }

    pub(crate) fn parameter_ty(&self, sr: StreamReference) -> Option<RustType> {
        if let Some(parameters) = self.stream_memory(sr).parameters() {
            let rust_tys = parameters
                .iter()
                .map(|p| RustType::from(p.ty.clone()))
                .collect::<Vec<_>>();
            Some(match rust_tys.len() {
                0 => unreachable!(),
                1 => rust_tys.into_iter().next().unwrap(),
                2.. => RustType::Tuple(rust_tys),
            })
        } else {
            None
        }
    }

    pub(crate) fn windows(&self) -> impl Iterator<Item = WindowReference> + '_ {
        self.wref2window.keys().sorted().copied()
    }

    pub(crate) fn sliding_windows(&self) -> impl Iterator<Item = usize> + '_ {
        self.windows().filter_map(|w| match w {
            WindowReference::Sliding(i) => Some(i),
            _ => None,
        })
    }
}

struct MonitorStruct;

impl StructDefinition for MonitorStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::MonitorStruct
    }

    fn struct_name(&self, f: &RustFormatter) -> String {
        f.monitor_struct_name()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.require_struct(StreamMemoryStruct);
        f.require_enum(DeadlineEnum);
        f.call_function::<_, String>(MonitorConstructor, &[]);
        [
            Some(StreamMemoryStruct.as_argument(f)),
            (!(f.dynamic_deadlines.is_empty() && f.static_deadlines.is_empty()))
                .then(|| QueueStruct.as_argument(f)),
            (!f.wref2window.is_empty()).then(|| WindowMemory.as_argument(f)),
            Some(f.time_argument()),
            Some((
                f.spawned_argument_name(),
                RustType::Vec(Box::new(DeadlineEnum.as_ty(f))),
            )),
            Some((
                f.closed_argument_name(),
                RustType::Vec(Box::new(StreamReferenceEnum.as_ty(f))),
            )),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> PathBuf {
        _f.main_file()
    }
}
