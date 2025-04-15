use eval::EvalStatement;
use input::InputStatement;
use itertools::Itertools;
use streamir_lib::{
    formatter::{
        expressions::ExprFormatter,
        guards::GuardFormatter,
        names::GetStreamName,
        statements::{DefaultStmtFormatter, StmtFormatter},
    },
    ir::{
        expressions::Expr, Guard, InputReference, LocalFreq, LocalFreqRef, OutputReference, Stmt,
        StreamReference, WindowReference,
    },
};

use crate::{
    constructs::{FunctionDefinition, RequirementKey, RustType, StructDefinition},
    expressions::{is_fresh::IsFresh, sync::SyncAccess},
    io::{InternalEventStruct, VerdictConstructor, VerdictStruct},
    schedule::ScheduleClearFunction,
    windows::sliding::SlidingWindowBuffer,
    DeadlineEnum, MonitorStruct, RustFormatter, StreamMemoryStruct, WindowMemory,
};

mod eval;
mod input;

impl DefaultStmtFormatter for RustFormatter {
    fn shift(&self, sr: StreamReference) -> String {
        let instance = self
            .stream_parameter(sr)
            .iter()
            .enumerate()
            .map(|(p, _)| self.cycle_parameter_name(p))
            .collect::<Vec<_>>();
        let instance = instance.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        let buffer = self.get_stream_buffer_mut(sr, instance.as_slice());
        format!("{buffer}.shift();")
    }

    fn input(&self, sr: InputReference) -> String {
        let input = InputStatement(sr);
        let name = self.stream_name(StreamReference::In(sr));
        let new_value = format!(
            "{}.{name}.expect(\"Checked the existence with the guard\")",
            InternalEventStruct.argument_name(self)
        );
        format!("{}?;", self.call_self_function(input, &[new_value]))
    }

    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> String {
        let dynamic_queue_update: Vec<_> = local_frequencies
            .into_iter()
            .map(|r| self.lfreq2lfreq[&r])
            .unique_by(
                |LocalFreq {
                     dur,
                     sr: lsr,
                     reference: _,
                 }| {
                    debug_assert_eq!(*lsr, sr);
                    *dur
                },
            )
            .collect();
        let spawn_deadlines = match dynamic_queue_update[..] {
            [] => "".into(),
            [LocalFreq {
                dur: period,
                sr: _,
                reference: _,
            }] => format!(
                "\nself.spawned.push({});",
                DeadlineEnum::dynamic_variant(
                    period,
                    &if with.is_some() {
                        format!(
                            "vec![StreamReference::{}({})]",
                            self.stream_reference_variant(sr.sr()),
                            self.cycle_parameters_variable(sr.sr())
                        )
                    } else {
                        format!(
                            "vec![StreamReference::{}]",
                            self.stream_reference_variant(sr.sr())
                        )
                    },
                    self
                )
            ),
            [LocalFreq {
                dur: _period_1,
                sr: _,
                reference: _,
            }, LocalFreq {
                dur: _period_2,
                sr: _,
                reference: _,
            }] => {
                unimplemented!("Eval and close must have the same local frequency");
            }
            _ => unreachable!(
                "Found more than two dynamic deadline {:?} in spawn of {sr:?}",
                &dynamic_queue_update[..]
            ),
        };
        if let Some(with) = with {
            let spawn_windows = windows
                .into_iter()
                .filter(|w| matches!(w, WindowReference::Sliding(_)))
                .map(|w| {
                    let target = self.wref2window[&w].target;
                    let parameter = self
                        .stream_parameter(target)
                        .iter()
                        .enumerate()
                        .map(|(i, _)| self.cycle_parameter_name(i))
                        .collect::<Vec<_>>();
                    let time = match self.wref2window[&w].origin_pacing {
                        Guard::LocalFreq(_) => "self.time",
                        Guard::GlobalFreq(_) => "Duration::new(0,0)",
                        _ => unreachable!(),
                    };
                    format!(
                        "self.{}.{}.spawn_window({}, {time});
                        if {fresh_check}? {{
                            let new_value = {new_value}?;
                            {window_update}
                        }}",
                        WindowMemory.argument_name(self),
                        self.window_name(w),
                        self.cycle_parameters_variable(sr.sr()),
                        new_value = self.call_self_function(SyncAccess(target), &parameter),
                        fresh_check = self.call_self_function(IsFresh(target), &parameter),
                        window_update = self.update_window(w, true)
                    )
                })
                .join("\n");

            let parameter_assignment = with
                .into_iter()
                .enumerate()
                .map(|(p, expr)| {
                    format!(
                        "let {} = {};",
                        self.cycle_parameter_name(p),
                        self.expr(expr)
                    )
                })
                .join("\n");
            format!(
                "{parameter_assignment}
                if !self.{argname}.{sname}.is_alive(&{params}) {{
                    self.{argname}.{sname}.spawn({params})?;{spawn_deadlines}{spawn_windows}
                }}",
                argname = StreamMemoryStruct.argument_name(self),
                sname = self.output_name(sr),
                params = self.cycle_parameters_variable(sr.sr())
            )
        } else {
            let spawn_windows = windows
                .into_iter()
                .filter(|w| matches!(w, WindowReference::Sliding(_)))
                .map(|w| {
                    let target = self.wref2window[&w].target;
                    let time = match self.wref2window[&w].origin_pacing {
                        Guard::LocalFreq(_) => "self.time",
                        Guard::GlobalFreq(_) => "Duration::new(0,0)",
                        _ => unreachable!(),
                    };
                    format!(
                        "self.{}.{} = {};
                        if {fresh_check}? {{
                            let new_value = {new_value}?;
                            {window_update}
                        }}",
                        WindowMemory.argument_name(self),
                        self.window_name(w),
                        SlidingWindowBuffer::constructor(w.idx(), self, time),
                        new_value = self.call_self_function::<_, String>(SyncAccess(target), &[]),
                        fresh_check = self.call_self_function::<_, String>(IsFresh(target), &[]),
                        window_update = self.update_window(w, true)
                    )
                })
                .join("\n");
            format!(
                "if !self.{argname}.{sname}.is_alive() {{
                    self.{argname}.{sname}.spawn()?;{spawn_deadlines}{spawn_windows}
                }}",
                argname = StreamMemoryStruct.argument_name(self),
                sname = self.output_name(sr)
            )
        }
    }

    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> String {
        let eval = EvalStatement {
            sr,
            expr: with,
            i: idx,
        };
        let instance = self
            .stream_parameter(sr.sr())
            .iter()
            .enumerate()
            .map(|(p, _)| self.cycle_parameter_name(p))
            .collect::<Vec<_>>();
        format!("{}?;", self.call_self_function(eval, &instance))
    }

    fn close(
        &self,
        sr: OutputReference,
        _local_frequencies: Vec<LocalFreqRef>,
        _windows: Vec<WindowReference>,
    ) -> String {
        match sr {
            OutputReference::Unparameterized(_) => {
                format!(
                    "self.{closed_field}.push(StreamReference::{sref});",
                    closed_field = self.closed_argument_name(),
                    sref = self.stream_reference_variant(sr.sr())
                )
            }
            OutputReference::Parameterized(_) => {
                format!(
                    "self.{closed_field}.push(StreamReference::{sref}({params}));",
                    closed_field = self.closed_argument_name(),
                    sref = self.stream_reference_variant(sr.sr()),
                    params = self.cycle_parameters_variable(sr.sr())
                )
            }
        }
    }

    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> String {
        let cond = self.guard(guard);
        let cons = self.stmt(cons);
        if let Some(alt) = alt {
            let alt = self.stmt(alt);
            format!("if {cond} {{\n{cons}\n}} else {{\n{alt}\n}}")
        } else {
            format!("if {cond} {{\n{cons}\n}}")
        }
    }

    fn iterate(&self, sr: Vec<OutputReference>, inner: Stmt) -> String {
        let sr = sr[0].sr();
        let stream_name = self.stream_name(sr);
        let parameter_pattern = self.cycle_parameters_variable(sr);
        format!(
            "for {parameter_pattern} in self.{}.{stream_name}.alive_parameters() {{\n{}\n}}",
            StreamMemoryStruct.argument_name(self),
            self.stmt(inner)
        )
    }

    fn assign(&self, _sr: Vec<OutputReference>, parameter_expr: Vec<Expr>, inner: Stmt) -> String {
        let s = parameter_expr
            .into_iter()
            .enumerate()
            .map(|(i, expr)| {
                format!(
                    "let {} = {};",
                    self.cycle_parameter_name(i),
                    self.expr(expr)
                )
            })
            .join("\n");
        format!("{s}\n{}", self.stmt(inner))
    }
}

impl RustFormatter {
    pub(crate) fn cycle_parameters_variable(&self, sr: StreamReference) -> String {
        let parameter_names = self
            .stream_parameter(sr)
            .iter()
            .enumerate()
            // we purposely don't use the actual name of the parameter because it is hard to keep track of the stream in inner statements
            .map(|(p, _)| self.cycle_parameter_name(p).to_string())
            .join(",");
        if self.stream_parameter(sr).len() == 1 {
            parameter_names
        } else {
            format!("({parameter_names})")
        }
    }

    pub(crate) fn method_parameters_variable(&self, sr: StreamReference) -> String {
        let parameter_names = self.stream_parameter(sr).iter().map(|p| &p.name).join(",");
        if self.stream_parameter(sr).len() == 1 {
            parameter_names
        } else {
            format!("({parameter_names})")
        }
    }
}

pub(crate) struct CycleFunction(pub Stmt);

impl FunctionDefinition for CycleFunction {
    fn name(&self, f: &RustFormatter) -> String {
        f.cycle_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        [
            &format!(
                "self.{} = {}.{};",
                f.time_argument_name(),
                InternalEventStruct.argument_name(f),
                f.time_argument_name()
            ),
            &f.stmt(self.0),
            &format!(
                "let verdict = {}?;",
                f.call_self_function::<_, String>(VerdictConstructor, &[])
            ),
            &format!(
                "{};",
                f.call_self_function::<_, String>(ScheduleClearFunction, &[])
            ),
            &format!(
                "{};",
                f.call_self_function::<_, String>(ClearActivationsFunction, &[])
            ),
            "Ok(verdict)",
        ]
        .join("\n")
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.require_struct(InternalEventStruct);
        vec![InternalEventStruct.as_argument(f)]
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Cycle
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(f.monitor_struct_name())
    }

    fn returns(&self, f: &RustFormatter) -> Option<RustType> {
        f.require_struct(VerdictStruct);
        Some(VerdictStruct.as_ty(f).result())
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

pub(crate) struct ClearActivationsFunction;

impl FunctionDefinition for ClearActivationsFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.clear_activations_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        f.streams()
            .map(|s| {
                format!(
                    "self.{}.{}.{}();",
                    StreamMemoryStruct.argument_name(f),
                    f.stream_name(s),
                    f.clear_activation_function_name()
                )
            })
            .join("\n")
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::ClearActivations
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

impl RustFormatter {
    fn update_windows(&self, target: StreamReference) -> String {
        self.sliding_windows()
            .map(WindowReference::Sliding)
            .filter(|w| self.wref2window[w].target == target)
            .map(|wref| self.update_window(wref, false))
            .join("\n")
    }

    pub(crate) fn update_window(&self, wref: WindowReference, in_cycle: bool) -> String {
        let target = self.wref2window[&wref].target;
        if self
            .stream_parameter(self.wref2window[&wref].target)
            .is_empty()
        {
            format!(
                "self.{}.{}.accept_value(self.{}, new_value);",
                WindowMemory.argument_name(self),
                self.window_name(wref),
                self.time_argument_name()
            )
        } else {
            let params = if in_cycle {
                self.cycle_parameters_variable(target)
            } else {
                self.method_parameters_variable(target)
            };
            format!(
                "self.{}.{}.get_window(&{}).map(|w| w.accept_value(self.{}, new_value));",
                WindowMemory.argument_name(self),
                self.window_name(wref),
                params,
                self.time_argument_name()
            )
        }
    }
}
