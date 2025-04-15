use std::time::Duration;

use streamir_lib::{
    formatter::{expressions::ExprFormatter, guards::DefaultGuardFormatter},
    ir::{expressions::Expr, LocalFreqRef, StreamReference},
};

use crate::SolidityFormatter;

impl DefaultGuardFormatter for SolidityFormatter {
    fn stream(&self, _sr: StreamReference) -> String {
        unreachable!("partial evaluation")
    }

    fn alive(&self, sr: StreamReference) -> String {
        format!("{}_spawned", self.name(sr))
    }

    fn dynamic(&self, expr: Expr) -> String {
        self.expr(expr)
    }

    fn global_freq(&self, _duration: Duration) -> String {
        unimplemented!("no time-based in solidity")
    }

    fn local_freq(&self, _freq_ref: LocalFreqRef) -> String {
        unimplemented!("no time-based in solidity")
    }

    fn constant(&self, b: bool) -> String {
        match b {
            true => "true",
            false => "false",
        }
        .into()
    }
}
