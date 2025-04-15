use itertools::Itertools;
use std::fmt::Write;
use streamir_lib::{
    formatter::{expressions::ExprFormatter, types::TypeFormatter},
    ir::{expressions::Expr, memory::Parameter, OutputReference, StreamReference},
};

use crate::{functions::FunctionDefinition, RequirementKey, SolidityFormatter};

pub(super) struct EvalFunction {
    pub(super) sr: OutputReference,
    pub(super) with: Expr,
    pub(super) idx: usize,
    pub(super) syn_accesses: Vec<StreamReference>,
}

impl FunctionDefinition for EvalFunction {
    fn header(&self, f: &SolidityFormatter) -> String {
        let local_streams = self
            .syn_accesses
            .iter()
            .map(|sr| format!("{} {}", f.ty(f.stream_type(*sr).clone()), f.name(*sr)))
            .join(" ,");
        let parameter = f
            .stream_parameter(StreamReference::Out(self.sr))
            .map(|p| {
                p.iter()
                    .map(|Parameter { name, ty }| format!("{} {name}", f.ty(ty.clone())))
                    .join(",")
            })
            .unwrap_or_default();
        format!(
            "{}({}{local_streams})",
            self.name(f),
            if parameter.is_empty() || local_streams.is_empty() {
                parameter
            } else {
                format!("{parameter}, ")
            },
        )
    }

    fn body(self, f: &SolidityFormatter) -> String {
        let mut res = String::new();
        let with = f.expr(self.with);
        let new_value = f.name(self.sr.sr());
        writeln!(res, "{new_value} = {with};").unwrap();
        writeln!(res, "{}", f.set_buffer_value(self.sr.sr(), new_value)).unwrap();
        res
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::EvalFunction(self.sr.sr(), self.idx)
    }

    fn name(&self, f: &SolidityFormatter) -> String {
        format!("eval_{}_{}", f.name(self.sr.sr()), self.idx)
    }

    fn returns(&self, f: &SolidityFormatter) -> Vec<(streamir_lib::ir::Type, String)> {
        vec![(
            f.stream_type(self.sr.sr()).clone(),
            f.name(self.sr.sr()).to_string(),
        )]
    }
}
