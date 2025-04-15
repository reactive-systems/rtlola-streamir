use itertools::Itertools;
use std::fmt::Write;
use streamir_lib::{
    formatter::types::TypeFormatter,
    ir::{
        memory::{Parameter, StreamBuffer, StreamMemory},
        OutputReference, StreamReference,
    },
};

use crate::{functions::FunctionDefinition, RequirementKey, SolidityFormatter};

pub(super) struct CloseFunction(pub(super) OutputReference);

impl FunctionDefinition for CloseFunction {
    fn header(&self, f: &SolidityFormatter) -> String {
        let parameter = f
            .stream_parameter(StreamReference::Out(self.0))
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, ty }| format!("{} {name}", f.ty(ty.clone())))
                    .join(",")
            })
            .unwrap_or_default();
        format!("{}({parameter})", self.name(f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let name = f.name(StreamReference::Out(self.0));
        let res = match self.0 {
            OutputReference::Unparameterized(_) => format!("{name}_spawned = false;"),
            OutputReference::Parameterized(_) => {
                let param_access = f.param_access(StreamReference::Out(self.0));

                let mut res = String::new();
                writeln!(res, "{name}_buffer{param_access}.{name}_spawned = false;").unwrap();
                if f.streams_with_iteration.contains(&self.0) {
                    writeln!(res, "for (uint i = 0; i < {name}_params.length; i++) {{").unwrap();
                    let condition = f
                        .stream_parameter(StreamReference::Out(self.0))
                        .unwrap()
                        .iter()
                        .map(|Parameter { name: pname, .. }| {
                            format!("{name}_params[i].{pname} == {pname}")
                        })
                        .join(" && ");
                    writeln!(res, "if ({condition}) {{").unwrap();
                    writeln!(res, "delete {name}_params[i];",).unwrap();
                    writeln!(res, "return;",).unwrap();
                    writeln!(res, "}}").unwrap();
                    writeln!(res, "}}").unwrap();
                }
                res
            }
        };
        format!("{}{res}", f.close_buffer(self.0.sr()))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::CloseFunction(StreamReference::Out(self.0))
    }

    fn name(&self, f: &SolidityFormatter) -> String {
        format!("close_{}", f.name(StreamReference::Out(self.0)))
    }
}

impl SolidityFormatter {
    fn close_buffer(&self, sr: StreamReference) -> String {
        match self.stream_memory(sr) {
            StreamMemory::NoMemory | StreamMemory::Static(_) => unreachable!(),
            StreamMemory::Dynamic {
                buffer,
                has_spawn: _,
                has_close: _,
            } => match buffer {
                StreamBuffer::SingleValue => format!("{}_valid = false;", self.name(sr)),
                StreamBuffer::Bounded(b) => {
                    format!(
                        "for (uint i = 0; i < {b}; i++) {{ {}_valid[i] = false; }}",
                        self.name(sr)
                    )
                }
                StreamBuffer::UnBounded => unimplemented!(),
            },
            StreamMemory::Instances { buffer, .. } => match buffer {
                StreamBuffer::SingleValue => format!(
                    "{name}_buffer{params}.{name}_valid = false; }}",
                    name = self.name(sr),
                    params = self.param_access(sr)
                ),
                StreamBuffer::Bounded(b) => {
                    format!(
                        "for (uint i = 0; i < {b}; i++) {{ {name}_buffer{params}.{name}_valid[i] = false; }}",
                        name=self.name(sr),
                        params=self.param_access(sr)
                    )
                }
                StreamBuffer::UnBounded => unimplemented!(),
            },
        }
    }
}
