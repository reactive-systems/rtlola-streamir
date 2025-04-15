use streamir_lib::{
    formatter::names::GetStreamName,
    ir::{InputReference, StreamReference},
};

use crate::{constructs::FunctionDefinition, StreamMemoryStruct, StructDefinition};

pub(super) struct InputStatement(pub(crate) InputReference);

impl FunctionDefinition for InputStatement {
    fn name(&self, f: &crate::RustFormatter) -> String {
        f.input_statement_function_name(StreamReference::In(self.0))
    }

    fn body(self, f: &crate::RustFormatter) -> String {
        let name = f.stream_name(StreamReference::In(self.0));
        let window_updates = f.update_windows(StreamReference::In(self.0));

        format!(
            "{window_updates}\nself.{}.{name}.update(new_value)",
            StreamMemoryStruct.argument_name(f)
        )
    }

    fn arguments(&self, f: &crate::RustFormatter) -> Vec<(String, crate::RustType)> {
        vec![(
            "new_value".to_string(),
            f.stream_type(StreamReference::In(self.0)),
        )]
    }

    fn key(&self) -> crate::constructs::RequirementKey {
        crate::constructs::RequirementKey::Statement(crate::constructs::StatementType::Input(
            self.0,
        ))
    }

    fn method_of(&self, f: &crate::RustFormatter) -> Option<String> {
        Some(f.monitor_struct_name())
    }

    fn returns(&self, _f: &crate::RustFormatter) -> Option<crate::RustType> {
        Some(crate::RustType::Unit.result())
    }

    fn visibility(&self) -> crate::constructs::FunctionVisibility {
        crate::constructs::FunctionVisibility::Crate
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn file(&self, _f: &crate::RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
