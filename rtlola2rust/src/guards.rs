use std::time::Duration;

use streamir_lib::{
    formatter::{expressions::ExprFormatter, guards::DefaultGuardFormatter, names::GetStreamName},
    ir::{expressions::Expr, LocalFreqRef, OutputReference, StreamReference},
};

use crate::{
    constructs::{FunctionDefinition, RequirementKey, StructDefinition},
    io::InternalEventStruct,
    MonitorStruct, RustFormatter, RustType, StreamMemoryStruct,
};

impl DefaultGuardFormatter for RustFormatter {
    fn stream(&self, sr: StreamReference) -> String {
        format!(
            "{}.{}.is_some()",
            InternalEventStruct.argument_name(self),
            self.stream_name(sr)
        )
    }

    fn alive(&self, sr: StreamReference) -> String {
        format!(
            "self.{}.{}.is_alive()",
            StreamMemoryStruct.argument_name(self),
            self.stream_name(sr)
        )
    }

    fn dynamic(&self, expr: Expr) -> String {
        let args: Vec<_> = expr
            .contains_parameter_access()
            .map(|sr| {
                (0..self.parameter_arguments(sr).len())
                    .map(|i| self.cycle_parameter_name(i))
                    .collect()
            })
            .unwrap_or_default();
        format!(
            "{}?",
            self.call_self_function(DynamicGuard::new(expr, self), &args)
        )
    }

    fn global_freq(&self, duration: Duration) -> String {
        format!(
            "{}.{}",
            InternalEventStruct.argument_name(self),
            self.static_deadline_event_name(duration)
        )
    }

    fn local_freq(&self, freq_ref: LocalFreqRef) -> String {
        let freq = &self.lfreq2lfreq[&freq_ref];
        let duration = freq.dur;
        let sr = freq.sr;
        let variant = match sr {
            OutputReference::Unparameterized(_) => {
                format!(
                    "StreamReference::{}",
                    self.stream_reference_variant(sr.sr())
                )
            }
            OutputReference::Parameterized(_) => {
                format!(
                    "StreamReference::{}({})",
                    self.stream_reference_variant(sr.sr()),
                    self.cycle_parameters_variable(sr.sr())
                )
            }
        };
        format!(
            "{}.{}.contains(&{variant})",
            InternalEventStruct.argument_name(self),
            self.dynamic_deadline_event_name(duration),
        )
    }

    fn constant(&self, b: bool) -> String {
        match b {
            true => "true",
            false => "false",
        }
        .into()
    }
}

pub(crate) struct DynamicGuard(Expr, usize);

impl DynamicGuard {
    pub(crate) fn new(expr: Expr, f: &RustFormatter) -> Self {
        let mut expr_counter = f.expr_counter.lock().unwrap();
        let params = expr.contains_parameter_access();
        if let Some(c) = expr_counter.get(&(expr.clone(), params)) {
            Self(expr, *c)
        } else {
            let mut num_exprs = f.num_exprs.lock().unwrap();
            let c = *num_exprs;
            expr_counter.insert((expr.clone(), params), c);
            *num_exprs += 1;
            Self(expr, c)
        }
    }
}

impl FunctionDefinition for DynamicGuard {
    fn name(&self, f: &RustFormatter) -> String {
        f.expr_function_name(self.1)
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        if let Some(sr) = self.0.contains_parameter_access() {
            f.parameter_arguments(sr)
        } else {
            Vec::new()
        }
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::Bool.result())
    }

    fn body(self, f: &RustFormatter) -> String {
        format!("Ok({})", f.expr(self.0))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::ExprFunction(self.1)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
