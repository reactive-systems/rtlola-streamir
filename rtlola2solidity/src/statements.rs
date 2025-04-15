use close::CloseFunction;
use eval::EvalFunction;
use input::EvalInputFunction;
use itertools::Itertools;
use shift::ShiftFunction;
use spawn::SpawnFunction;
use std::fmt::Write;
use streamir_lib::{
    formatter::{
        expressions::ExprFormatter,
        guards::GuardFormatter,
        statements::{DefaultStmtFormatter, StmtFormatter},
        types::TypeFormatter,
    },
    ir::{
        expressions::{Constant, Expr, ExprKind},
        memory::{Parameter, StreamMemory},
        InputReference, Origin, OutputReference, StreamAccessKind, StreamReference,
    },
};
mod close;
mod eval;
mod input;
mod shift;
mod spawn;

use crate::{
    functions::{SingleTriggerFunction, TriggerFunction},
    SolidityFormatter, TriggerFunctionMode,
};

impl DefaultStmtFormatter for SolidityFormatter {
    fn shift(&self, sr: StreamReference) -> String {
        let parameter = self.get_parameter_from_iterator(sr);
        format!("{};", self.call_function(ShiftFunction(sr), parameter))
    }

    fn input(&self, sr: InputReference) -> String {
        match self.stream_memory(StreamReference::In(sr)) {
            StreamMemory::NoMemory => "".into(),
            StreamMemory::Static(_) => format!(
                "{};",
                self.call_function(
                    EvalInputFunction(sr),
                    vec![self.name(StreamReference::In(sr)).to_string()],
                )
            ),

            _ => unreachable!("for inputs"),
        }
    }

    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<streamir_lib::ir::LocalFreqRef>,
        windows: Vec<streamir_lib::ir::WindowReference>,
    ) -> String {
        assert!(windows.is_empty());
        assert!(local_frequencies.is_empty());
        let parameter = with
            .map(|expr| expr.into_iter().map(|expr| self.expr(expr)).collect())
            .unwrap_or_default();
        format!("{};", self.call_function(SpawnFunction { sr }, parameter))
    }

    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> String {
        if let Some(trigger_idx) = self.triggers.get(&sr) {
            let ExprKind::Constant(Constant::Str(msg)) = &with.kind else {
                panic!("Trigger messages have to be static")
            };
            format!(
                "{};",
                match self.trigger_function_mode {
                    TriggerFunctionMode::Multiple => {
                        self.call_function(TriggerFunction(*trigger_idx, msg.into()), vec![])
                    }
                    TriggerFunctionMode::Single => {
                        self.call_function(SingleTriggerFunction, vec![])
                    }
                }
            )
        } else {
            let parameter = self.get_parameter_from_iterator(StreamReference::Out(sr));
            let syn_accesses = self
                .accesses(StreamReference::Out(sr))
                .iter()
                .filter(|(sr, _)| self.stream_parameter(*sr).is_none())
                .filter_map(|(sr, accesses)| {
                    let sync_accesses = accesses
                        .iter()
                        .filter(|(o, _a)| matches!(o, Origin::EvalWith(i) if *i == idx))
                        .filter(|(_o, a)| matches!(a, StreamAccessKind::Sync))
                        .collect::<Vec<_>>();
                    (!sync_accesses.is_empty()).then_some(sr)
                })
                .copied()
                .collect::<Vec<_>>();

            let parameter = parameter
                .into_iter()
                .chain(syn_accesses.iter().map(|sr| self.name(*sr).to_string()))
                .collect();

            let ty = with.ty.clone();
            let c = self.call_function(
                EvalFunction {
                    sr,
                    with,
                    idx,
                    syn_accesses,
                },
                parameter,
            );
            format!(
                "{} {} = {c};",
                self.type_with_storage(ty),
                self.name(sr.sr())
            )
        }
    }

    fn close(
        &self,
        sr: OutputReference,
        local_frequencies: Vec<streamir_lib::ir::LocalFreqRef>,
        windows: Vec<streamir_lib::ir::WindowReference>,
    ) -> String {
        assert!(local_frequencies.is_empty());
        assert!(windows.is_empty());
        let parameter = self.get_parameter_from_iterator(StreamReference::Out(sr));
        format!("{};", self.call_function(CloseFunction(sr), parameter))
    }

    fn r#if(
        &self,
        guard: streamir_lib::ir::Guard,
        cons: streamir_lib::ir::Stmt,
        alt: Option<streamir_lib::ir::Stmt>,
    ) -> String {
        let cond = self.guard(guard);
        let cons = self.stmt(cons);
        if let Some(alt) = alt {
            let alt = self.stmt(alt);
            format!("if ({cond}) {{ {cons} }} else {{ {alt} }}")
        } else {
            format!("if ({cond}) {{ {cons} }}")
        }
    }

    fn iterate(&self, sr: Vec<OutputReference>, inner: streamir_lib::ir::Stmt) -> String {
        assert!(sr.iter().all(|sr| self.streams_with_iteration.contains(sr)));
        assert!(sr
            .iter()
            .all(|o| self.stream_parameter(StreamReference::Out(*o))
                == self.stream_parameter(StreamReference::Out(sr[0]))));
        let name = self.name(sr[0].sr());
        let parameter = self.stream_parameter(sr[0].sr()).unwrap();
        let param_assignment = parameter
            .iter()
            .map(|Parameter { name: pname, ty }| {
                format!("{} {pname} = param.{pname};", self.ty(ty.clone()))
            })
            .join("\n");
        let mut res = format!("for (uint i = 0; i < {name}_params.length; i++) {{\n");
        writeln!(res, "{name}Param memory param = {name}_params[i];").unwrap();
        writeln!(res, "{param_assignment}").unwrap();
        writeln!(res, "{}\n}}", self.stmt(inner)).unwrap();
        res
    }

    fn assign(
        &self,
        sr: Vec<OutputReference>,
        parameter_expr: Vec<Expr>,
        inner: streamir_lib::ir::Stmt,
    ) -> String {
        assert!(sr
            .iter()
            .all(|o| self.stream_parameter(StreamReference::Out(*o))
                == self.stream_parameter(StreamReference::Out(sr[0]))));
        let param_access = self.param_access(sr[0].sr());
        let assignments = parameter_expr
            .into_iter()
            .zip(self.stream_parameter(sr[0].sr()).unwrap())
            .map(|(expr, param)| {
                format!(
                    "{} {} = {};",
                    self.ty(expr.ty.clone()),
                    param.name,
                    self.expr(expr)
                )
            })
            .join("\n");
        let name = self.name(sr[0].sr());
        format!(
            "{{
            {assignments}\nif ({name}_buffer{param_access}.{name}_spawned) {{\n{}\n}}
        }}",
            self.stmt(inner)
        )
    }
}
