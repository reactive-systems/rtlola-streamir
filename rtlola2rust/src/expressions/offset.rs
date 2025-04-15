use streamir_lib::ir::StreamReference;

use crate::{constructs::FunctionDefinition, RustType};

pub(super) struct OffsetAccess(pub(super) StreamReference);

impl FunctionDefinition for OffsetAccess {
    fn name(&self, f: &crate::RustFormatter) -> String {
        f.offset_access_function_name(self.0)
    }

    fn body(self, f: &crate::RustFormatter) -> String {
        f.require_struct(crate::memory::StreamMemoryStruct);
        let get = f.get_stream_value(self.0, "offset");
        format!("Ok({get})")
    }

    fn arguments(&self, f: &crate::RustFormatter) -> Vec<(String, RustType)> {
        let mut res = f.parameter_arguments(self.0);
        res.insert(0, ("offset".to_string(), RustType::Usize));
        res
    }

    fn key(&self) -> crate::constructs::RequirementKey {
        crate::constructs::RequirementKey::StreamAccess(
            crate::constructs::StreamAccessType::OffsetAccess(self.0),
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
