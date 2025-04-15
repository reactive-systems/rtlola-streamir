use itertools::Itertools;
use std::fmt::Write;
use streamir_lib::{
    formatter::types::TypeFormatter,
    ir::{memory::Parameter, OutputReference},
};

use crate::{functions::FunctionDefinition, RequirementKey, SolidityFormatter};

pub(super) struct SpawnFunction {
    pub(super) sr: OutputReference,
}

impl FunctionDefinition for SpawnFunction {
    fn header(&self, f: &SolidityFormatter) -> String {
        let parameter = f
            .stream_parameter(streamir_lib::ir::StreamReference::Out(self.sr))
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, ty }| format!("{} {name}", f.ty(ty.clone())))
                    .join(",")
            })
            .unwrap_or_default();
        format!("{}({parameter})", self.name(f))
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let name = f.name(streamir_lib::ir::StreamReference::Out(self.sr));
        match self.sr {
            OutputReference::Unparameterized(_) => format!("{name}_spawned = true;"),
            OutputReference::Parameterized(_) => {
                let param_access = f.param_access(streamir_lib::ir::StreamReference::Out(self.sr));

                let mut res = String::new();
                if f.streams_with_iteration.contains(&self.sr) {
                    writeln!(res, "if (!{name}_buffer{param_access}.{name}_spawned) {{").unwrap();
                    let p = &format!(
                        "{name}Param({{ {} }})",
                        f.stream_parameter(streamir_lib::ir::StreamReference::Out(self.sr))
                            .unwrap()
                            .iter()
                            .map(|Parameter { name, .. }| format!("{name}: {name}"))
                            .join(",")
                    );
                    writeln!(res, "{name}_params.push({p});",).unwrap();
                    writeln!(res, "}}").unwrap();
                }
                writeln!(res, "{name}_buffer{param_access}.{name}_spawned = true;").unwrap();
                res
            }
        }
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::SpawnFunction(streamir_lib::ir::StreamReference::Out(self.sr))
    }

    fn name(&self, f: &SolidityFormatter) -> String {
        format!(
            "spawn_{}",
            f.name(streamir_lib::ir::StreamReference::Out(self.sr))
        )
    }
}
