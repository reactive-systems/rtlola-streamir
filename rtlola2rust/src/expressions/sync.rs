use streamir_lib::ir::StreamReference;

use crate::{
    constructs::{FunctionDefinition, RequirementKey},
    memory::StreamMemoryStruct,
    RustFormatter, RustType,
};

pub(crate) struct SyncAccess(pub(crate) StreamReference);

impl FunctionDefinition for SyncAccess {
    fn name(&self, f: &RustFormatter) -> String {
        f.sync_access_function_name(self.0)
    }

    fn body(self, f: &RustFormatter) -> String {
        f.require_struct(StreamMemoryStruct);
        let get = f.get_stream_value(self.0, "0");
        format!("Ok({get}.expect(\"sync access\"))")
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.parameter_arguments(self.0)
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::StreamAccess(crate::constructs::StreamAccessType::SyncAccess(self.0))
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(f.monitor_struct_name())
    }

    fn returns(&self, f: &RustFormatter) -> Option<RustType> {
        Some(f.stream_type(self.0).result())
    }

    fn visibility(&self) -> crate::constructs::FunctionVisibility {
        crate::constructs::FunctionVisibility::Crate
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
