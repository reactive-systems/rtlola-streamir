//! A module to define the interaction with the resulting contract
use std::collections::HashMap;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use streamir_lib::{
    formatter::{files::FilesFormatter, statements::StmtFormatter, types::TypeFormatter},
    ir::{InputReference, Stmt, StreamIr, StreamReference},
    optimize,
    rewrite_rules::EvaluateGuards,
};

use crate::{
    functions::{FunctionDefinition, OutputsVerdict, Visibility},
    RequirementKey, SolidityFormatter,
};

pub(crate) fn partial_evaluation(
    config: &InterfaceConfig,
    lir: &StreamIr,
) -> Result<Vec<(InterfaceFunction, Stmt)>, String> {
    let functions = config.check(lir)?;
    let functions_with_stmts = functions
        .into_iter()
        .map(|function| {
            let inputs = function.inputs();
            let stmt = optimize(
                lir.clone(),
                vec![Box::new(EvaluateGuards::only_streams(inputs))],
            )
            .unwrap()
            .stmt;
            (function, stmt)
        })
        .collect::<_>();

    Ok(functions_with_stmts)
}

impl FunctionDefinition for (InterfaceFunction, Stmt) {
    fn name(&self, _f: &SolidityFormatter) -> String {
        self.0.name.clone()
    }

    fn header(&self, f: &SolidityFormatter) -> String {
        format!(
            "{}({})",
            self.name(f),
            self.0
                .arguments
                .iter()
                .filter(|arg| arg.parameter)
                .map(|arg| format!(
                    "{} {}",
                    f.ty(f.stream_type(StreamReference::In(arg.stream_ref)).clone()),
                    arg.name,
                ))
                .join(", ")
        )
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let code_inputs = self
            .0
            .arguments
            .iter()
            .filter(|arg| !arg.parameter)
            .map(|arg| {
                format!(
                    "{} {} = {};",
                    f.ty(f.stream_type(StreamReference::In(arg.stream_ref)).clone()),
                    arg.name,
                    arg.source
                )
            })
            .join("\n");
        vec![
            Some(code_inputs),
            Some(f.stmt(self.1.clone())),
            (!f.outputs_verdict.is_empty()).then(|| OutputsVerdict.emit(f)),
        ]
        .into_iter()
        .flatten()
        .join("\n")
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Cycle(self.0.name.clone())
    }

    fn visibility(&self) -> Visibility {
        Visibility::Public
    }

    fn payable(&self) -> bool {
        self.0.tags.contains(&"payable".to_string())
    }
}

impl SolidityFormatter {
    pub(crate) fn format_cycle_functions(
        &self,
        config: &InterfaceConfig,
        lir: StreamIr,
    ) -> Result<(), String> {
        let functions = partial_evaluation(config, &lir)?;
        for function in functions {
            self.add_requirement((function,));
        }
        Ok(())
    }
}

/// The configuration of the function interface.
/// Can be deserialized from TOML, JSON or YAML.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct InterfaceConfig {
    /// A list of functions that form the interface.
    function: Vec<FunctionConfig>,
    /// A list of additional inputs that are always computed
    #[serde(default = "Vec::new")]
    additional_input: Vec<AdditionalInput>,
}

/// The configuration of a single interface function.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FunctionConfig {
    /// The name of the function.
    name: String,
    /// Optionally, the name of a unit-typed stream.
    stream_name: Option<String>,
    /// A list of function arguments that form monitor inputs.
    #[serde(default = "Vec::new")]
    argument: Vec<ParameterConfig>,
    /// A list of additional inputs that are sent in addition to the arguments
    #[serde(default = "Vec::new")]
    additional_input: Vec<AdditionalInput>,
    /// A list of tags usable in the template
    #[serde(default)]
    tags: Vec<String>,
}

/// The configuration of a single function parameter.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ParameterConfig {
    /// The name of the function parameter.
    name: String,
    /// The name of the corresponding stream.
    /// (defaults to the parameter name)
    stream_name: Option<String>,
    /// The code that is used to fill that input stream
    code: Option<String>,
}

/// Additional input stream value's that are computed through code
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AdditionalInput {
    /// The name of the input stream
    input: String,
    /// The source code to generate the input's value.
    source: String,
    /// Whether this value is given as a parameter
    parameter: bool,
}

/// The definition of a function providing inputs to the monitor.
///
/// Can be constructed from the [FunctionConfig] with `config.check()`
/// by providing the `IntermediateRepresentation` of the specification.
#[derive(Debug, Clone)]
pub struct InterfaceFunction {
    /// The name of the function.
    pub name: String,
    /// The arguments of the function.
    pub arguments: Vec<InputSource>,
    /// Arbitary tags copied from the config to use in the template.
    pub tags: Vec<String>,
}

/// The source of a value for an input stream.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InputSource {
    /// The code evaluating to the new value.
    /// For function arguments, this simply is the name
    /// of the argument.
    pub source: String,
    /// The reference of the input stream the value is provided to.
    pub stream_ref: InputReference,
    /// The name of the input stream
    pub name: String,
    /// Whether this value comes from a function parameter
    pub parameter: bool,
}

impl InterfaceConfig {
    /// Parses the given TOML config file into the interface configuration.
    pub fn from_toml(toml: &str) -> Result<Self, String> {
        toml::from_str(toml).map_err(|e| e.to_string())
    }
}

impl InterfaceConfig {
    fn check(&self, lir: &StreamIr) -> Result<Vec<InterfaceFunction>, String> {
        let InterfaceConfig {
            function,
            additional_input,
        } = self;
        let functions = function
            .iter()
            .map(|function| {
                let function = function.check(additional_input, lir)?;
                Ok(function)
            })
            .collect::<Result<Vec<InterfaceFunction>, String>>()?;

        Ok(functions)
    }
}

impl InterfaceFunction {
    fn inputs(&self) -> Vec<StreamReference> {
        self.arguments
            .iter()
            .map(|argument| StreamReference::In(argument.stream_ref))
            .collect()
    }
}

impl FunctionConfig {
    fn check(
        &self,
        global_additional_inputs: &[AdditionalInput],
        ir: &StreamIr,
    ) -> Result<InterfaceFunction, String> {
        let FunctionConfig {
            name,
            stream_name,
            argument,
            additional_input,
            tags,
        } = self;

        let arguments: Vec<_> = argument
            .iter()
            .map(|argument| argument.to_input(ir))
            .collect::<Result<Vec<InputSource>, String>>()?;

        let additional_inputs: Vec<_> = global_additional_inputs
            .iter()
            .chain(additional_input)
            .map(|input| input.check(ir))
            .collect::<Result<Vec<InputSource>, String>>()?;

        let inputs: Vec<_> = arguments.iter().cloned().chain(additional_inputs).collect();

        let mut inputs = inputs.iter().fold(HashMap::new(), |mut inputs, argument| {
            inputs.insert(argument.stream_ref, argument.source.clone());
            inputs
        });

        if let Some(stream_name) = stream_name {
            let idx = match ir.stream_by_name(stream_name) {
                Some(StreamReference::In(idx)) => idx,
                Some(StreamReference::Out(_)) => return Err(format!("Function \"{}\": The stream \"{}\" is expected to be an input stream, but was an output stream instead.", name, stream_name)),
                None => return Err(format!("Function \"{}\": The stream \"{}\" does not exist in the specification.", name, stream_name)),
            };
            inputs.insert(idx, "()".into());
        }

        Ok(InterfaceFunction {
            name: name.into(),
            arguments,
            tags: tags.clone(),
        })
    }
}

impl ParameterConfig {
    fn to_input(&self, lir: &StreamIr) -> Result<InputSource, String> {
        let ParameterConfig {
            name,
            stream_name,
            code,
        } = self;
        let stream_name = stream_name.as_ref().unwrap_or(name).clone();
        AdditionalInput {
            input: stream_name,
            source: code.as_ref().cloned().unwrap_or_else(|| name.clone()),
            parameter: code.is_none(),
        }
        .check(lir)
    }
}

impl AdditionalInput {
    fn check(&self, lir: &StreamIr) -> Result<InputSource, String> {
        let sr = match lir.stream_by_name(&self.input) {
            Some(StreamReference::Out(_)) => {
                return Err(format!(
                    "The stream \"{}\" is expected to be an input stream, but is output stream instead.",
                    self.input
                ))
            },
            Some(StreamReference::In(idx)) => idx,
            None => {
                return Err(format!(
                    "The stream \"{}\" does not exist in specification.",
                    self.input,
                ))
            },
        };
        Ok(InputSource {
            source: self.source.clone(),
            stream_ref: sr,
            name: self.input.clone(),
            parameter: self.parameter,
        })
    }
}
