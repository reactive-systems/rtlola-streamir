use std::fmt::Write;
use streamir_lib::formatter::{files::Requirement, types::TypeFormatter};
use streamir_lib::ir::{
    memory::{Parameter, StreamBuffer, StreamMemory},
    Accesses, StreamReference,
};

use crate::{RequirementKey, SolidityFormatter};

impl SolidityFormatter {
    pub(crate) fn check_buffer_value(&self, sr: StreamReference, offset: &str) -> String {
        let memory = self.stream_memory(sr);
        let name = self.name(sr);
        match memory {
            StreamMemory::NoMemory => "true".into(),
            StreamMemory::Static(buffer) | StreamMemory::Dynamic { buffer, .. } => match buffer {
                StreamBuffer::SingleValue => format!("{name}_valid"),
                StreamBuffer::Bounded(bound) => {
                    format!(
                        "{name}_valid[({name}_current + {offset}) % {bound}]",
                        bound = bound.max(&1)
                    )
                }
                StreamBuffer::UnBounded => panic!("unsupported"),
            },
            StreamMemory::Instances { buffer, .. } => {
                let param_access = self.param_access(sr);
                match buffer {
                    StreamBuffer::SingleValue => {
                        format!("{name}_buffer{param_access}.{name}_valid")
                    }
                    StreamBuffer::Bounded(bound) => format!("{name}_buffer{param_access}.{name}_valid[({name}_buffer{param_access}.{name}_current + {offset}) % {bound}]", bound=bound.max(&1)),
                    StreamBuffer::UnBounded => {
                        unimplemented!("Unbounded stream buffer required in parametrization ")
                    }
                }
            }
        }
    }

    pub(crate) fn param_access(&self, sr: StreamReference) -> String {
        self.sr2memory[&sr]
            .parameters()
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, ty: _ }| format!("[{name}]"))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn set_buffer_value(&self, sr: StreamReference, value: &str) -> String {
        let memory = self.stream_memory(sr);
        let name = self.name(sr);
        match memory {
            StreamMemory::NoMemory => "".into(),
            StreamMemory::Static(buffer) | StreamMemory::Dynamic { buffer, .. } => match buffer {
                StreamBuffer::SingleValue => {
                    format!("{name}_buffer = {value};\n{name}_valid = true;")
                }
                StreamBuffer::Bounded(_) => format!("{name}_buffer[{name}_current] = {value};\n{name}_valid[{name}_current] = true;"),
                StreamBuffer::UnBounded => unimplemented!("Unbounded streambuffer not implemented"),
            },
            StreamMemory::Instances { buffer, parameter: _ } => {
                let parameter = self.param_access(sr);
                match buffer {
                    StreamBuffer::SingleValue => {
                        let mut res = format!("{name}_buffer{parameter}.{name}_buffer = {value};\n");
                        write!(res, "{name}_buffer{parameter}.{name}_valid = true;").unwrap();
                        res
                    }
                    StreamBuffer::Bounded(_) => {
                        let mut res = format!("{name}_buffer{parameter}.{name}_buffer[{name}_buffer{parameter}.{name}_current] = {value};");
                        write!(res, "\n{name}_buffer{parameter}.{name}_valid[{name}_buffer{parameter}.{name}_current] = true;").unwrap();
                        res
                    },
                    StreamBuffer::UnBounded => unimplemented!("Unbounded streambuffer not implemented"),
                }
            },
        }
    }

    pub(crate) fn access_buffer_value(
        &self,
        sr: StreamReference,
        offset: &str,
        param_access: String,
    ) -> String {
        let memory = self.stream_memory(sr);
        let name = self.name(sr);
        match memory {
            StreamMemory::NoMemory => name.into(),
            StreamMemory::Static(buffer) | StreamMemory::Dynamic { buffer, .. } => match buffer {
                StreamBuffer::SingleValue => format!("{name}_buffer"),
                StreamBuffer::Bounded(bound) => {
                    format!(
                        "{name}_buffer[({name}_current + {offset}) % {}]",
                        bound.max(&1)
                    )
                }
                StreamBuffer::UnBounded => panic!("unsupported"),
            },
            StreamMemory::Instances { buffer, .. } => {
                match buffer {
                StreamBuffer::SingleValue => format!("{name}_buffer{param_access}.{name}_buffer"),
                StreamBuffer::Bounded(bound) => format!("{name}_buffer{param_access}.{name}_buffer[({name}_buffer{param_access}.{name}_current + {offset}) % {bound}]", bound=bound.max(&1)),
                StreamBuffer::UnBounded => panic!("unsupported")
            }
            }
        }
    }
}

pub(crate) struct Memory(pub StreamReference);

impl Requirement<SolidityFormatter> for Memory {
    fn key(&self) -> <SolidityFormatter as streamir_lib::formatter::files::FilesFormatter>::Key {
        RequirementKey::Memory(self.0)
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().into()
    }

    fn format(self, formatter: &SolidityFormatter) -> String {
        let name = formatter.name(self.0);
        match formatter.stream_memory(self.0) {
            StreamMemory::NoMemory => {
                if formatter.stream_parameter(self.0).is_some() {
                    formatter.format_static_memory(self.0, &StreamBuffer::SingleValue)
                } else {
                    "".into()
                }
            }
            StreamMemory::Static(buffer) => formatter.format_static_memory(self.0, buffer),
            StreamMemory::Dynamic { buffer, .. } => format!(
                "{}\nbool {name}_spawned;",
                formatter.format_static_memory(self.0, buffer)
            ),
            StreamMemory::Instances { buffer, parameter } => {
                let parameter_ty = parameter
                    .iter()
                    .map(|Parameter { name: _, ty }| formatter.ty(ty.clone()))
                    .collect::<Vec<_>>();
                let mapping = parameter_ty
                    .iter()
                    .fold(format!("Buffer{name}"), |inner, ty| {
                        format!("mapping ({ty} => {inner})")
                    });
                let mut res = format!(
                    "struct Buffer{name} {{\n{}\nbool {name}_spawned;\n}}",
                    formatter.format_static_memory(self.0, buffer)
                );
                writeln!(res, "{mapping} {name}_buffer;").unwrap();
                if formatter.streams_with_iteration.contains(&self.0.out_idx()) {
                    let param_struct_fields = parameter
                        .iter()
                        .map(|Parameter { name, ty }| {
                            format!("{ty} {name};", ty = formatter.ty(ty.clone()))
                        })
                        .collect::<String>();
                    writeln!(res, "struct {name}Param {{ {param_struct_fields} }}\n{name}Param[] {name}_params;\n").unwrap();
                }
                res
            }
        }
    }
}

impl SolidityFormatter {
    fn format_static_memory(&self, sr: StreamReference, buffer: &StreamBuffer) -> String {
        let name = self.name(sr);
        match buffer {
            StreamBuffer::SingleValue => format!(
                "{} {name}_buffer;bool {name}_valid;",
                self.ty(self.stream_type(sr).to_owned())
            ),
            StreamBuffer::Bounded(bound) => format!(
                "{}[{bound}] {name}_buffer;bool[{bound}] {name}_valid;uint64 {name}_current;",
                self.ty(self.stream_type(sr).to_owned()),
                bound = bound.max(&1)
            ),
            StreamBuffer::UnBounded => panic!("unsupported"),
        }
    }

    pub(crate) fn get_parameter_from_iterator(&self, sr: StreamReference) -> Vec<String> {
        self.stream_parameter(sr)
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, .. }| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn accesses(&self, sr: StreamReference) -> &Accesses {
        &self.accesses[&sr]
    }
}
