use std::rc::Rc;

use bit_set::BitSet;
use itertools::Itertools;
use streamir_lib::{
    formatter::{expressions::ExprFormatter, guards::GuardFormatter, statements::StmtFormatter},
    ir::{
        expressions::Expr, Guard, InputReference, LocalFreq, LocalFreqRef, OutputReference, Stmt,
        StreamReference, WindowReference,
    },
};

use super::{Closuregen, EvaluationContext, Event};

type CompiledStmtFn<Event> = Box<dyn Fn(&Event, &mut EvaluationContext)>;
pub(crate) struct CompiledStmt<Event>(CompiledStmtFn<Event>);

impl<Event> CompiledStmt<Event> {
    pub(crate) fn new(f: impl Fn(&Event, &mut EvaluationContext) + 'static) -> Self {
        Self(Box::new(f))
    }

    pub(crate) fn execute(&self, inputs: &Event, memory: &mut EvaluationContext) {
        (self.0)(inputs, memory)
    }
}

impl<E: 'static> FromIterator<CompiledStmt<E>> for CompiledStmt<E> {
    fn from_iter<T: IntoIterator<Item = CompiledStmt<E>>>(iter: T) -> Self {
        iter.into_iter()
            .fold(CompiledStmt::new(move |_, _| {}), |init, cur| {
                CompiledStmt::new(move |v, ctx| {
                    init.execute(v, ctx);
                    cur.execute(v, ctx);
                })
            })
    }
}

impl<E: Event + 'static> StmtFormatter for Closuregen<E> {
    type Return = CompiledStmt<E>;

    fn skip(&self) -> Self::Return {
        CompiledStmt::new(move |_, _| {})
    }

    fn seq(&self, inner: Vec<Stmt>) -> Self::Return {
        let inner: Vec<_> = inner.into_iter().map(|stmt| self.stmt(stmt)).collect();
        CompiledStmt::new(move |inputs, ctx| inner.iter().for_each(|m| m.execute(inputs, ctx)))
    }

    fn parallel(&self, inner: Vec<Stmt>) -> Self::Return {
        self.seq(inner)
    }

    fn shift(&self, sr: StreamReference) -> Self::Return {
        match sr {
            StreamReference::In(idx) => {
                CompiledStmt::new(move |_ev, ctx| ctx.memory.shift_input(idx))
            }
            StreamReference::Out(idx) => match idx {
                OutputReference::Unparameterized(i) => {
                    CompiledStmt::new(move |_ev, ctx| ctx.memory.shift_output_value(i))
                }
                OutputReference::Parameterized(i) => CompiledStmt::new(move |_ev, ctx| {
                    ctx.memory
                        .shift_output_instance(i, ctx.parameter.as_ref().unwrap())
                }),
            },
        }
    }

    fn input(&self, sr: InputReference) -> Self::Return {
        let windows = self.accept_values(StreamReference::In(sr));
        CompiledStmt::new(move |inputs, ctx| {
            let value = E::get_input(sr, inputs);
            windows.execute(value, ctx);
            ctx.memory.eval_input(sr, value.clone());
        })
    }

    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Self::Return {
        match sr {
            OutputReference::Unparameterized(idx) => {
                let dynamic_queue_update: Vec<_> = local_frequencies
                    .into_iter()
                    .map(|r| self.lr2local_freq[&r])
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
                    [] => CompiledStmt::new(|_, _| {}),
                    [LocalFreq {
                        dur: period,
                        sr: _,
                        reference: _,
                    }] => CompiledStmt::new(move |_ev, ctx| {
                        ctx.schedule
                            .dynamic_schedule
                            .add_stream(ctx.ts + period, period, sr);
                    }),
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
                        // CompiledStmt::new(move |_ev, ctx| {
                        //     ctx.schedule.dynamic_schedule.add_stream(
                        //         ctx.ts + period_1,
                        //         period_1,
                        //         sr,
                        //     );
                        //     ctx.schedule.dynamic_schedule.add_stream(
                        //         ctx.ts + period_2,
                        //         period_2,
                        //         sr,
                        //     );
                        // })
                    }
                    _ => unreachable!(
                        "Found more than two dynamic deadline {:?} in spawn of {sr:?}",
                        &dynamic_queue_update[..]
                    ),
                };
                let windows_activate = windows
                    .into_iter()
                    .filter(|w| matches!(w, WindowReference::Sliding(_)))
                    .map(|w| self.compile_dynamic_spawn(w))
                    .collect::<CompiledStmt<E>>();

                CompiledStmt::new(move |ev, ctx| {
                    if ctx.memory.spawn_stream(idx) {
                        spawn_deadlines.execute(ev, ctx);
                        windows_activate.execute(ev, ctx);
                        ctx.spawned_streams.insert(idx);
                    }
                })
            }
            OutputReference::Parameterized(idx) => {
                let compiled_with =
                    with.map(|with| with.into_iter().map(|p| self.expr(p)).collect::<Vec<_>>());

                let dynamic_queue_update: Vec<_> = local_frequencies
                    .into_iter()
                    .map(|r| self.lr2local_freq[&r])
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
                    [] => CompiledStmt::new(|_, _| {}),
                    [LocalFreq {
                        dur: period,
                        sr: _,
                        reference: _,
                    }] => CompiledStmt::new(move |_ev, ctx| {
                        ctx.schedule.dynamic_schedule.add_instance(
                            ctx.ts + period,
                            period,
                            sr,
                            ctx.parameter.as_ref().unwrap().clone(),
                        );
                    }),
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
                        //     CompiledStmt::new(move |_ev, ctx| {
                        //     ctx.schedule.dynamic_schedule.add_instance(
                        //         ctx.ts + period_1,
                        //         period_1,
                        //         sr,
                        //         ctx.parameter.as_ref().unwrap().clone(),
                        //     );
                        //     ctx.schedule.dynamic_schedule.add_instance(
                        //         ctx.ts + period_2,
                        //         period_2,
                        //         sr,
                        //         ctx.parameter.as_ref().unwrap().clone(),
                        //     );
                        // })
                    }
                    _ => unreachable!(),
                };
                let windows_activate = windows
                    .into_iter()
                    .filter(|w| matches!(w, WindowReference::Sliding(_)))
                    .map(|w| self.compile_instance_spawn(w))
                    .collect::<CompiledStmt<E>>();

                CompiledStmt::new(move |ev, ctx| {
                    let instance: Vec<_> = compiled_with
                        .as_ref()
                        .map(|with| with.iter().map(|p| p.execute(ctx)).collect())
                        .unwrap_or_default();
                    let rc = Rc::new(instance);
                    if ctx.memory.spawn_instance(idx, rc.clone()) {
                        ctx.parameter = Some(rc);
                        spawn_deadlines.execute(ev, ctx);
                        windows_activate.execute(ev, ctx);
                        ctx.instances[idx].spawn(ctx.parameter.as_ref().unwrap().clone());
                    }
                })
            }
        }
    }

    fn eval(&self, sr: OutputReference, with: Expr, _idx: usize) -> Self::Return {
        let compiled_expr = self.expr(with);
        let windows = self.accept_values(StreamReference::Out(sr));
        match sr {
            OutputReference::Unparameterized(i) => CompiledStmt::new(move |_ev, ctx| {
                let new_value = compiled_expr.execute(ctx);
                ctx.fresh_outputs.insert(i);
                windows.execute(&new_value, ctx);
                ctx.memory.eval_output_value(i, new_value);
            }),
            OutputReference::Parameterized(i) => CompiledStmt::new(move |_, ctx| {
                let new_value = compiled_expr.execute(ctx);
                windows.execute(&new_value, ctx);
                ctx.instances[i].eval(ctx.parameter.as_ref().unwrap().clone());
                ctx.memory
                    .eval_output_instance(i, ctx.parameter.as_ref().unwrap(), new_value);
            }),
        }
    }

    fn close(
        &self,
        sr: OutputReference,
        local_frequencies: Vec<LocalFreqRef>,
        _windows: Vec<WindowReference>,
    ) -> Self::Return {
        let local_frequencies: BitSet = local_frequencies.into_iter().collect();
        let (closing_unparam_local, closing_param_local) = local_frequencies
            .into_iter()
            .map(|freq| self.lr2local_freq[&freq].sr)
            .partition_map(|o| match o {
                OutputReference::Unparameterized(i) => itertools::Either::Left(i),
                OutputReference::Parameterized(i) => itertools::Either::Right(i),
            });
        match sr {
            OutputReference::Unparameterized(i) => CompiledStmt::new(move |_, ctx| {
                ctx.closing_streams.insert(i);
                ctx.closing_unparam_local.union_with(&closing_unparam_local);
            }),
            OutputReference::Parameterized(i) => CompiledStmt::new(move |_, ctx| {
                ctx.instances[i].close(ctx.parameter.as_ref().unwrap().clone());
                // TODO parameter
                ctx.closing_param_local.union_with(&closing_param_local);
            }),
        }
    }

    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> Self::Return {
        let guard = self.guard(guard);
        let cons = self.stmt(cons);
        if let Some(alt) = alt {
            let alt = self.stmt(alt);
            CompiledStmt::new(move |ev, ctx| {
                if guard.execute(ev, ctx) {
                    cons.execute(ev, ctx);
                } else {
                    alt.execute(ev, ctx);
                }
            })
        } else {
            CompiledStmt::new(move |ev, ctx| {
                if guard.execute(ev, ctx) {
                    cons.execute(ev, ctx);
                }
            })
        }
    }

    fn iterate(&self, sr: Vec<OutputReference>, inner: Stmt) -> Self::Return {
        let compiled_stmt = self.stmt(inner);
        let sr = sr[0].parameterized_idx();
        CompiledStmt::new(move |ev, ctx| {
            // we replace the buffer with a default value during iteration,
            // but need to ensure it is returned afterwards.
            // This is only possible because for every iterate parameters block,
            // it can never happen an instance is spawned or removed from the memory inside the block.
            let instances = ctx.memory.instance_buffer(sr).take_instances();
            for p in &instances {
                ctx.parameter = Some(p.clone());
                compiled_stmt.execute(ev, ctx);
            }
            ctx.memory.instance_buffer(sr).return_instances(instances);
        })
    }

    fn assign(
        &self,
        sr: Vec<OutputReference>,
        parameter_expr: Vec<Expr>,
        inner: Stmt,
    ) -> Self::Return {
        let inner = self.stmt(inner);
        let parameter_expr = parameter_expr
            .into_iter()
            .map(|p| self.expr(p))
            .collect::<Vec<_>>();
        let sr = sr[0].parameterized_idx();
        CompiledStmt::new(move |ev, ctx| {
            let instance = parameter_expr
                .iter()
                .map(|e| e.execute(ctx))
                .collect::<Vec<_>>();
            if ctx.memory.instance_is_alive(sr, &instance) {
                ctx.parameter = Some(Rc::new(instance));
                inner.execute(ev, ctx);
            }
        })
    }
}
