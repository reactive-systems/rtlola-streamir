use streamir_lib::{
    formatter::types::TypeFormatter,
    ir::{InputReference, StreamReference},
};

use crate::{functions::FunctionDefinition, RequirementKey, SolidityFormatter};

pub(super) struct EvalInputFunction(pub(super) InputReference);

impl FunctionDefinition for EvalInputFunction {
    fn header(&self, f: &SolidityFormatter) -> String {
        let ty = f.ty(f.stream_type(StreamReference::In(self.0)).clone());
        format!("{}({ty} value)", self.name(f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        f.set_buffer_value(StreamReference::In(self.0), "value")
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::EvalFunction(StreamReference::In(self.0), 0)
    }

    fn name(&self, f: &SolidityFormatter) -> String {
        format!("eval_{}", f.name(StreamReference::In(self.0)))
    }
}
