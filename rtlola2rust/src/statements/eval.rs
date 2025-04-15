use itertools::Itertools;
use streamir_lib::{
    formatter::expressions::ExprFormatter,
    ir::{expressions::Expr, memory::Parameter, OutputReference, StreamReference},
};

use crate::constructs::FunctionDefinition;

pub(super) struct EvalStatement {
    pub(super) sr: OutputReference,
    pub(super) expr: Expr,
    pub(super) i: usize,
}

impl FunctionDefinition for EvalStatement {
    fn name(&self, f: &crate::RustFormatter) -> String {
        f.eval_statement_function_name(self.sr.sr(), self.i)
    }

    fn body(self, f: &crate::RustFormatter) -> String {
        let EvalStatement { sr, expr, i: _ } = self;
        let expr = f.expr(expr);
        let instance = f
            .stream_parameter(sr.sr())
            .iter()
            .map(|Parameter { name, .. }| name.as_str())
            .collect::<Vec<_>>();
        let buffer = f.get_stream_buffer_mut(sr.sr(), &instance);
        let window_updates = f.update_windows(StreamReference::Out(self.sr));
        vec![
            format!("let new_value = {expr}"),
            window_updates,
            format!("{buffer}.update(new_value)?"),
            "Ok(())".to_string(),
        ]
        .into_iter()
        .join(";\n")
    }

    fn arguments(&self, f: &crate::RustFormatter) -> Vec<(String, crate::RustType)> {
        f.parameter_arguments(self.sr.sr())
    }

    fn key(&self) -> crate::constructs::RequirementKey {
        crate::constructs::RequirementKey::Statement(crate::constructs::StatementType::Eval(
            self.sr, self.i,
        ))
    }

    fn method_of(&self, f: &crate::RustFormatter) -> Option<String> {
        Some(f.monitor_struct_name())
    }

    fn returns(&self, _f: &crate::RustFormatter) -> Option<crate::RustType> {
        Some(crate::RustType::Unit.result())
    }

    fn visibility(&self) -> crate::constructs::FunctionVisibility {
        crate::constructs::FunctionVisibility::Crate
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn file(&self, _f: &crate::RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
