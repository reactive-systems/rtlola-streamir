use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        names::GetStreamName,
    },
    ir::StreamReference,
};

use crate::{constructs::RequirementKey, RustFormatter};

pub(crate) struct MonitorError;

impl MonitorError {
    pub(crate) fn add_requirement(f: &RustFormatter) {
        f.add_requirement(MonitorError);
    }

    pub(crate) fn instance_not_found(
        stream: StreamReference,
        instance: &str,
        f: &RustFormatter,
    ) -> String {
        Self::add_requirement(f);
        let stream_name = f.stream_name(stream);
        format!("{}::InstanceNotFound {{ stream: \"{stream_name}\", instance: format!(\"{{:?}}\", ({instance})) }}", f.error_name())
    }
}

impl Requirement<RustFormatter> for MonitorError {
    fn key(&self) -> <RustFormatter as FilesFormatter>::Key {
        RequirementKey::MonitorError
    }

    fn format(self, formatter: &RustFormatter) -> String {
        format!(
            "#[derive(Debug, Clone)]
			pub enum {} {{
			InstanceNotFound {{ stream: &'static str, instance: String }},
			OutOfBoundsAccess {{ accessed_offset: usize, buffer_size: usize }}
		}}",
            formatter.error_name()
        )
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.main_file()
    }
}
