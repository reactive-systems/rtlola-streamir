use itertools::Itertools;
use streamir_lib::{
    formatter::types::TypeFormatter,
    ir::{memory::Parameter, OutputReference, StreamReference},
};

use crate::{functions::FunctionDefinition, RequirementKey, SolidityFormatter};

pub(super) struct ShiftFunction(pub(super) StreamReference);

impl FunctionDefinition for ShiftFunction {
    fn header(&self, f: &SolidityFormatter) -> String {
        let parameter = f
            .stream_parameter(self.0)
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, ty }| format!("{} {name}", f.ty(ty.clone())))
                    .join(",")
            })
            .unwrap_or_default();
        format!("{}({parameter})", self.name(f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let name = f.name(self.0);
        let bound = f
            .stream_memory(self.0)
            .buffer()
            .map(|b| {
                b.bound()
                    .expect("The current implementation only supports a bounded streambuffer")
            })
            .unwrap_or_else(|| 0);
        match self.0 {
            StreamReference::In(_) | StreamReference::Out(OutputReference::Unparameterized(_)) => {
                format!(
                    "{name}_current = ({name}_current + 1) % {bound};",
                    bound = bound.max(1)
                )
            }
            StreamReference::Out(OutputReference::Parameterized(_)) => {
                let param_access = f.param_access(self.0);
                format!("{name}_buffer{param_access}.{name}_current = ({name}_buffer{param_access}.{name}_current + 1) % {bound};", bound=bound.max(1))
            }
        }
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::ShiftFunction(self.0)
    }

    fn name(&self, f: &SolidityFormatter) -> String {
        format!("shift_{}", f.name(self.0))
    }
}
