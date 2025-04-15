use std::fmt::Write;

use crate::expressions::get_access::GetAccessFunction;
use itertools::Itertools;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        types::TypeFormatter,
    },
    ir::Type,
};

use crate::{RequirementKey, SolidityFormatter, TriggerAction};

impl SolidityFormatter {
    pub(crate) fn call_function<F: FunctionDefinition>(
        &self,
        f: F,
        parameter: Vec<String>,
    ) -> String {
        let c = format!("{}({})", f.name(self), parameter.join(","));
        self.add_requirement((f,));
        c
    }
}

impl<F: FunctionDefinition> Requirement<SolidityFormatter> for (F,) {
    fn key(&self) -> <SolidityFormatter as FilesFormatter>::Key {
        self.0.key()
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().clone()
    }

    fn format(self, formatter: &SolidityFormatter) -> String {
        let returns = match self.0.returns(formatter).as_slice() {
            [] => String::new(),
            returns => {
                let returns = returns
                    .iter()
                    .map(|(ty, name)| format!("{} {name}", formatter.type_with_storage(ty.clone())))
                    .join(", ");
                format!(" returns ({returns})")
            }
        };
        let mut res = format!(
            "function {}{}{}{}{} {{\n",
            self.0.header(formatter),
            self.0.visibility(),
            self.0.mutability(),
            if self.0.payable() { " payable" } else { "" },
            returns
        );
        writeln!(&mut res, "{}", self.0.body(formatter)).unwrap();
        writeln!(&mut res, "}}").unwrap();
        res
    }
}

impl FilesFormatter for SolidityFormatter {
    type Key = RequirementKey;

    fn get_construct_store(&self) -> &streamir_lib::formatter::files::ConstructStore<Self> {
        &self.construct_store
    }

    fn overwrite(&self) -> bool {
        self.overwrite
    }
}

pub(crate) trait FunctionDefinition {
    fn name(&self, f: &SolidityFormatter) -> String;

    fn header(&self, f: &SolidityFormatter) -> String;

    fn body(self, f: &SolidityFormatter) -> String;

    fn key(&self) -> RequirementKey;

    fn visibility(&self) -> Visibility {
        Visibility::Private
    }

    fn payable(&self) -> bool {
        false
    }

    fn returns(&self, _f: &SolidityFormatter) -> Vec<(Type, String)> {
        vec![]
    }

    fn mutability(&self) -> FunctionStateMutability {
        FunctionStateMutability::None
    }
}

pub(crate) enum FunctionStateMutability {
    None, // no annotation
    View,
}

impl std::fmt::Display for FunctionStateMutability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionStateMutability::None => Ok(()),
            FunctionStateMutability::View => write!(f, " view"),
        }
    }
}

pub(crate) enum Visibility {
    Private,
    Public,
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Visibility::Private => " private",
            Visibility::Public => " public",
        };
        write!(f, "{}", s)
    }
}

pub(crate) struct TriggerFunction(pub usize, pub String);

impl FunctionDefinition for TriggerFunction {
    fn name(&self, _f: &SolidityFormatter) -> String {
        format!("trigger{}", self.0)
    }

    fn header(&self, f: &SolidityFormatter) -> String {
        format!("{}()", self.name(f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let action = match f.trigger_action {
            TriggerAction::Revert => {
                format!("revert(\"{}\");", self.1)
            }
            TriggerAction::EmitSingle => {
                f.add_requirement(SingleTriggerEvent);
                "emit Trigger();".into()
            }
            TriggerAction::EmitMultiple => {
                f.add_requirement(TriggerEvent(self.0));
                format!("emit Trigger{}();", self.0)
            }
        };
        format!("// {}\n{action}", self.1)
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::TriggerFunction(self.0)
    }
}

pub(crate) struct SingleTriggerFunction;

impl FunctionDefinition for SingleTriggerFunction {
    fn name(&self, _f: &SolidityFormatter) -> String {
        "trigger".into()
    }

    fn header(&self, _f: &SolidityFormatter) -> String {
        format!("{}()", self.name(_f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        match f.trigger_action {
            TriggerAction::Revert => "revert();".into(),
            TriggerAction::EmitSingle => {
                f.add_requirement(SingleTriggerEvent);
                "emit Trigger();".into()
            }
            TriggerAction::EmitMultiple => unreachable!(),
        }
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::TriggerFunction(0)
    }
}

pub(crate) struct SingleTriggerEvent;

impl Requirement<SolidityFormatter> for SingleTriggerEvent {
    fn key(&self) -> <SolidityFormatter as FilesFormatter>::Key {
        RequirementKey::TriggerEvent(0)
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().into()
    }

    fn format(self, _formatter: &SolidityFormatter) -> String {
        "event Trigger();".to_string()
    }
}

pub(crate) struct TriggerEvent(usize);

impl Requirement<SolidityFormatter> for TriggerEvent {
    fn key(&self) -> <SolidityFormatter as FilesFormatter>::Key {
        RequirementKey::TriggerEvent(self.0)
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().into()
    }

    fn format(self, _formatter: &SolidityFormatter) -> String {
        format!("event Trigger{}();", self.0)
    }
}

pub(crate) struct OutputsVerdict;

impl OutputsVerdict {
    pub(crate) fn emit(&self, f: &SolidityFormatter) -> String {
        f.add_requirement(Self);
        let fields = f
            .outputs_verdict
            .iter()
            .map(|sr| {
                f.call_function(
                    GetAccessFunction { sr: *sr },
                    vec!["0".into(), f.default_value(f.stream_type(*sr)).into()],
                )
            })
            .join(", ");
        format!("emit Verdict( {fields} );")
    }
}

impl SolidityFormatter {
    fn default_value(&self, ty: &Type) -> &'static str {
        match ty {
            Type::Int(_) | Type::UInt(_) => "0",
            Type::Bool => "false",
            Type::Option(_) | Type::String => unreachable!(),
            Type::Float32 | Type::Float64 => "0.0",
            Type::Bytes | Type::Fixed(_) | Type::Tuple(_) | Type::UFixed(_) => {
                panic!("unsupported in solidity")
            }
        }
    }
}

impl Requirement<SolidityFormatter> for OutputsVerdict {
    fn key(&self) -> <SolidityFormatter as FilesFormatter>::Key {
        RequirementKey::VerdictEvent
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().into()
    }

    fn format(self, formatter: &SolidityFormatter) -> String {
        let fields = formatter
            .outputs_verdict
            .iter()
            .map(|s| {
                format!(
                    "{} {}",
                    formatter.ty(formatter.stream_type(*s).clone()),
                    formatter.name(*s)
                )
            })
            .join(", ");
        format!("event Verdict ( {fields} );")
    }
}
