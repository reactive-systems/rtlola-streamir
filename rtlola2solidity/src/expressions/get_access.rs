use streamir_lib::{
    formatter::types::TypeFormatter,
    ir::{memory::Parameter, StreamReference, Type},
};

use std::fmt::Write;

use crate::{
    functions::{FunctionDefinition, FunctionStateMutability},
    RequirementKey,
};

pub(crate) struct GetAccessFunction {
    pub sr: StreamReference,
}

impl FunctionDefinition for GetAccessFunction {
    fn name(&self, f: &crate::SolidityFormatter) -> String {
        format!("get_{}", f.name(self.sr))
    }

    fn header(&self, f: &crate::SolidityFormatter) -> String {
        let parameter = f
            .stream_parameter(self.sr)
            .map(|p| {
                p.iter()
                    .fold(String::new(), |mut res, Parameter { name, ty }| {
                        write!(res, "{} {name}, ", f.ty(ty.clone())).unwrap();
                        res
                    })
            })
            .unwrap_or_default();

        format!(
            "{}({}uint64 offset, {} def)",
            self.name(f),
            parameter,
            f.ty(f.stream_type(self.sr).to_owned())
        )
    }

    fn body(self, f: &crate::SolidityFormatter) -> String {
        format!(
            "if ({}) {{\n\
            value = {};
        }} else {{\n\
            value = def;\n\
    }}",
            f.check_buffer_value(self.sr, "offset"),
            f.access_buffer_value(self.sr, "offset", f.param_access(self.sr))
        )
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::GetFunction(self.sr)
    }

    fn returns(&self, f: &crate::SolidityFormatter) -> Vec<(Type, String)> {
        vec![(f.stream_type(self.sr).clone(), "value".into())]
    }

    fn mutability(&self) -> FunctionStateMutability {
        FunctionStateMutability::View
    }
}
