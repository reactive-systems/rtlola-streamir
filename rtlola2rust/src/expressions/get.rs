use streamir_lib::ir::StreamReference;

use crate::{constructs::FunctionDefinition, RustType};

pub(crate) struct GetAccess(pub(crate) StreamReference);

impl FunctionDefinition for GetAccess {
    fn name(&self, f: &crate::RustFormatter) -> String {
        f.get_access_function_name(self.0)
    }

    fn body(self, f: &crate::RustFormatter) -> String {
        f.require_struct(crate::memory::StreamMemoryStruct);
        let parameter = f.parameter_arguments(self.0);
        let instance: Vec<_> = parameter.iter().map(|(n, _)| n.as_str()).collect();
        let buffer = f.get_stream_buffer_mut(self.0, &instance);
        let sync = f.get_stream_value_async(self.0, "0");
        format!("Ok(if {buffer}.is_fresh() {{\n{sync}\n}} else {{ None }})")
    }

    fn arguments(&self, f: &crate::RustFormatter) -> Vec<(String, RustType)> {
        f.parameter_arguments(self.0)
    }

    fn key(&self) -> crate::constructs::RequirementKey {
        crate::constructs::RequirementKey::StreamAccess(
            crate::constructs::StreamAccessType::GetAccess(self.0),
        )
    }

    fn method_of(&self, f: &crate::RustFormatter) -> Option<String> {
        Some(f.monitor_struct_name())
    }

    fn returns(&self, f: &crate::RustFormatter) -> Option<RustType> {
        Some(f.stream_type(self.0).optional().result())
    }

    fn visibility(&self) -> crate::constructs::FunctionVisibility {
        crate::constructs::FunctionVisibility::Crate
    }

    fn file(&self, _f: &crate::RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
