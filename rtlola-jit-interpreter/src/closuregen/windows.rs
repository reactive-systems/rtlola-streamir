use std::{fmt::Debug, time::Duration};

use aggregations::*;
use dyn_clone::DynClone;
use instances::{ConditionalInstanceWindow, InstanceWindow};
use sliding::SlidingWindow;
use streamir_lib::{
    formatter::expressions::ExprFormatter,
    ir::{
        windows::{InstanceSelection, Window, WindowKind, WindowOperation},
        Guard, OutputReference, StreamReference, Type, WindowReference,
    },
};

use crate::{value::Value, Time};

use super::{statements::CompiledStmt, Closuregen, EvaluationContext, Event};

mod aggregations;
mod instances;
mod sliding;

pub(crate) trait SlidingWindowTrait: DynClone + Debug {
    fn get_value(&self, ts: Time) -> Value;
    fn accept_value(&mut self, v: Value, ts: Time);
    fn activate(&mut self, ts: Time);
}
dyn_clone::clone_trait_object!(SlidingWindowTrait);

macro_rules! construct_sliding_window1 {
    ($op:expr, $ty:expr, $wait:expr, $bucket_count:expr, $ts:expr, $bucket_duration:expr) => {
        match $op {
            WindowOperation::Sum => {
                construct_sliding_window2!(Sum, $ty, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Count => {
                construct_sliding_window3!(Count, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Average => {
                construct_sliding_window2!(Avg, $ty, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Conjunction => {
                construct_sliding_window3!(Conjunction, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Disjunction => {
                construct_sliding_window3!(Disjunction, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Min => {
                construct_sliding_window2!(Min, $ty, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Max => {
                construct_sliding_window2!(Max, $ty, $wait, $bucket_count, $ts, $bucket_duration)
            }
            WindowOperation::Integral => {
                construct_sliding_window2!(
                    Integral,
                    $ty,
                    $wait,
                    $bucket_count,
                    $ts,
                    $bucket_duration
                )
            }
            op => unimplemented!("window operation not supported: {:?}", op),
        }
    };
}

macro_rules! construct_sliding_window2 {
    ($op:ident, $ty:expr, $wait:expr, $bucket_count:expr, $ts:expr, $bucket_duration:expr) => {
        match $ty.inner_ty() {
            Type::UInt(_) => {
                construct_sliding_window3!(
                    $op<Unsigned>,
                    $wait,
                    $bucket_count,
                    $ts,
                    $bucket_duration
                )
            }
            Type::Int(_) => {
                construct_sliding_window3!($op<Signed>, $wait, $bucket_count, $ts, $bucket_duration)
            }
            Type::Float32 | Type::Float64 => {
                construct_sliding_window3!(
                    aggregations::$op<Float>,
                    $wait,
                    $bucket_count,
                    $ts,
                    $bucket_duration
                )
            }
            ty => unimplemented!("window operation over unsupported type: {:?}", ty),
        }
    };
}

macro_rules! construct_sliding_window3 {
    ($op:ty, $wait:expr, $bucket_count:expr, $ts:expr, $bucket_duration:expr) => {
        match $wait {
            true => Box::new(SlidingWindow::<true, $op>::new(
                $bucket_count,
                $ts,
                $bucket_duration,
            )),
            false => Box::new(SlidingWindow::<false, $op>::new(
                $bucket_count,
                $ts,
                $bucket_duration,
            )),
        }
    };
}

impl<E: Event> Closuregen<E> {
    pub(crate) fn sliding_window(&self, window: &Window, ts: Time) -> Box<dyn SlidingWindowTrait> {
        let WindowKind::Sliding {
            duration,
            bucket_count,
            bucket_duration,
            wait,
        } = window.kind
        else {
            unreachable!()
        };
        debug_assert_eq!(duration, bucket_duration * (bucket_count as u32));
        construct_sliding_window1!(
            window.op,
            window.ty.clone(),
            wait,
            bucket_count,
            ts,
            bucket_duration
        )
    }
}

macro_rules! construct_instance_aggregation1 {
    ($op:expr, $ty:expr, $selection:expr, $idx:expr, $s:expr) => {
        match $op {
            WindowOperation::Sum => {
                construct_instance_aggregation2!(Sum, $ty, $selection, $idx, $s)
            }
            WindowOperation::Count => {
                construct_instance_aggregation3!(Count, $selection, $idx, $s)
            }
            WindowOperation::Average => {
                construct_instance_aggregation2!(Avg, $ty, $selection, $idx, $s)
            }
            WindowOperation::Conjunction => {
                construct_instance_aggregation3!(Conjunction, $selection, $idx, $s)
            }
            WindowOperation::Disjunction => {
                construct_instance_aggregation3!(Disjunction, $selection, $idx, $s)
            }
            WindowOperation::Min => {
                construct_instance_aggregation2!(Min, $ty, $selection, $idx, $s)
            }
            WindowOperation::Max => {
                construct_instance_aggregation2!(Max, $ty, $selection, $idx, $s)
            }
            other => unimplemented!("unsupported instance aggregation function: {other:?}"),
        }
    };
}

macro_rules! construct_instance_aggregation2 {
    ($op:ident, $ty:expr, $selection:expr, $idx:expr, $s:expr) => {
        match $ty {
            Type::UInt(_) => construct_instance_aggregation3!($op<Unsigned>, $selection, $idx, $s),
            Type::Int(_) => construct_instance_aggregation3!($op<Signed>, $selection, $idx, $s),
            Type::Float32 | Type::Float64 => {
                construct_instance_aggregation3!($op<Float>, $selection, $idx, $s)
            }
            other => unimplemented!("unsupported type for instance aggregation: {other:?}"),
        }
    };
}

macro_rules! construct_instance_aggregation3 {
    ($agg:ty, $selection:expr, $idx:expr, $s:expr) => {
        match $selection {
            InstanceSelection::All => Box::new(InstanceWindow::<$agg, false>::new($idx)),
            InstanceSelection::Fresh => Box::new(InstanceWindow::<$agg, true>::new($idx)),
            InstanceSelection::FilteredAll { cond, .. } => {
                Box::new(ConditionalInstanceWindow::<$agg, false>::new(
                    $s.expr(cond),
                    $idx,
                ))
            }
            InstanceSelection::FilteredFresh { cond, .. } => {
                Box::new(ConditionalInstanceWindow::<$agg, true>::new(
                    $s.expr(cond),
                    $idx,
                ))
            }
        }
    };
}

pub(crate) trait InstanceWindowTrait: Debug {
    fn get_value(&self, ctx: &EvaluationContext<'_>) -> Value;
}

impl<E: Event + 'static> Closuregen<E> {
    pub(crate) fn instance_window(&self, window: Window) -> Box<dyn InstanceWindowTrait> {
        let Window {
            wref: _,
            op: _,
            target,
            caller: _,
            kind,
            ty,
            origin: _,
            origin_pacing: _,
        } = window;
        let WindowKind::Instances { selection } = kind else {
            unreachable!()
        };

        construct_instance_aggregation1!(
            window.op,
            ty,
            selection,
            target.out_idx().parameterized_idx(),
            self
        )
        // match (window.op, ty, selection) {
        //     (WindowOperation::Sum, Type::UInt(_), InstanceSelection::All) => {
        //         Box::new(InstanceWindow::<aggregations::Sum<Unsigned>, false>::new(
        //             target.out_idx().parameterized_idx(),
        //         ))
        //     }
        //     (WindowOperation::Sum, Type::UInt(_), InstanceSelection::Fresh) => {
        //         Box::new(InstanceWindow::<aggregations::Sum<Unsigned>, true>::new(
        //             target.out_idx().parameterized_idx(),
        //         ))
        //     }
        //     (WindowOperation::Sum, Type::UInt(_), InstanceSelection::FilteredAll { cond, .. }) => {
        //         Box::new(ConditionalInstanceWindow::<
        //             aggregations::Sum<Unsigned>,
        //             false,
        //         >::new(
        //             self.expr(cond), target.out_idx().parameterized_idx()
        //         ))
        //     }
        //     (
        //         WindowOperation::Sum,
        //         Type::UInt(_),
        //         InstanceSelection::FilteredFresh { cond, .. },
        //     ) => Box::new(
        //         ConditionalInstanceWindow::<aggregations::Sum<Unsigned>, true>::new(
        //             self.expr(cond),
        //             target.out_idx().parameterized_idx(),
        //         ),
        //     ),
        //     (WindowOperation::Count, _, InstanceSelection::All) => {
        //         Box::new(InstanceWindow::<aggregations::Count, false>::new(
        //             target.out_idx().parameterized_idx(),
        //         ))
        //     }
        //     (WindowOperation::Count, _, InstanceSelection::Fresh) => {
        //         Box::new(InstanceWindow::<aggregations::Count, true>::new(
        //             target.out_idx().parameterized_idx(),
        //         ))
        //     }
        //     (WindowOperation::Count, _, InstanceSelection::FilteredAll { cond, .. }) => Box::new(
        //         ConditionalInstanceWindow::<aggregations::Count, false>::new(
        //             self.expr(cond),
        //             target.out_idx().parameterized_idx(),
        //         ),
        //     ),
        //     (WindowOperation::Count, _, InstanceSelection::FilteredFresh { cond, .. }) => {
        //         Box::new(ConditionalInstanceWindow::<aggregations::Count, true>::new(
        //             self.expr(cond),
        //             target.out_idx().parameterized_idx(),
        //         ))
        //     }
        //     op => unimplemented!("{:?}", op),
        // }
    }
}

type CompiledAcceptValuesFn = Box<dyn Fn(&Value, &mut EvaluationContext)>;
pub(crate) struct CompiledAcceptValues(CompiledAcceptValuesFn);

impl CompiledAcceptValues {
    pub(crate) fn new(f: impl Fn(&Value, &mut EvaluationContext) + 'static) -> Self {
        Self(Box::new(f))
    }

    pub(crate) fn execute(&self, inputs: &Value, memory: &mut EvaluationContext) {
        (self.0)(inputs, memory)
    }
}

impl<E: Event> Closuregen<E> {
    pub(crate) fn accept_values(&self, sr: StreamReference) -> CompiledAcceptValues {
        self.wref2window
            .iter()
            .filter(|(_, w)| w.target == sr)
            .filter(|(_, w)| matches!(w.wref, WindowReference::Sliding(_)))
            .map(|(_, w)| self.accept_value(w))
            .fold(CompiledAcceptValues::new(move |_, _| {}), |init, cur| {
                CompiledAcceptValues::new(move |v, ctx| {
                    init.execute(v, ctx);
                    cur.execute(v, ctx);
                })
            })
    }

    pub(crate) fn accept_value(&self, window: &Window) -> CompiledAcceptValues {
        match window.wref {
            WindowReference::Sliding(_)
                if matches!(
                    window.caller,
                    StreamReference::Out(OutputReference::Parameterized(_))
                ) =>
            {
                let wref = window.wref;
                CompiledAcceptValues::new(move |v, ctx| {
                    ctx.memory.instance_sliding_window_accept_value(
                        wref,
                        v.clone(),
                        ctx.parameter.as_ref().unwrap(),
                        ctx.ts,
                    )
                })
            }
            WindowReference::Sliding(_)
                if matches!(
                    window.caller,
                    StreamReference::Out(OutputReference::Unparameterized(_))
                ) =>
            {
                let wref = window.wref;
                CompiledAcceptValues::new(move |v, ctx| {
                    ctx.memory
                        .sliding_window_accept_value(wref, v.clone(), ctx.ts)
                })
            }
            WindowReference::Discrete(_)
            | WindowReference::Sliding(_)
            | WindowReference::Instance(_) => unreachable!(),
        }
    }
}

impl<E: Event + 'static> Closuregen<E> {
    pub(crate) fn compile_instance_spawn(&self, wref: WindowReference) -> CompiledStmt<E> {
        match wref {
            WindowReference::Sliding(idx) => {
                let Window {
                    wref: _,
                    op: _,
                    target,
                    caller: _,
                    origin: _,
                    origin_pacing,
                    kind: _,
                    ty: _,
                } = &self.wref2window[&wref];
                let target_idx = target.out_idx().parameterized_idx();
                let spawn_window_instance = move |ctx: &mut EvaluationContext, ts| {
                    let mut base = ctx.memory.sliding_windows[idx].clone();
                    base.activate(ts);
                    ctx.memory.instanced_sliding_windows[idx]
                        .insert(ctx.parameter.clone().unwrap(), base);
                    if ctx.instances[target_idx]
                        .eval
                        .iter()
                        .any(|i| **i == ctx.parameter.as_ref().unwrap().as_slice())
                    {
                        let v = ctx.memory.get_output_instance_value(
                            target_idx,
                            ctx.parameter.as_ref().unwrap(),
                            0,
                        );
                        ctx.memory.sliding_windows[idx].accept_value(v, ctx.ts);
                    }
                };
                match origin_pacing {
                    Guard::GlobalFreq(_duration) => CompiledStmt::new(move |_, ctx| {
                        spawn_window_instance(ctx, Duration::new(0, 0))
                    }),
                    Guard::LocalFreq(_) => {
                        CompiledStmt::new(move |_, ctx| spawn_window_instance(ctx, ctx.ts))
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn compile_dynamic_spawn(&self, wref: WindowReference) -> CompiledStmt<E> {
        match wref {
            WindowReference::Sliding(idx) => {
                let Window {
                    origin_pacing,
                    target,
                    ..
                } = &self.wref2window[&wref];

                let add_fresh_value = match *target {
                    StreamReference::In(i) => CompiledStmt::new(move |_ev, ctx| {
                        if ctx.fresh_inputs.contains(i) {
                            let v = ctx.memory.get_input_value(i, 0);
                            ctx.memory.sliding_windows[idx].accept_value(v, ctx.ts);
                        }
                    }),
                    StreamReference::Out(OutputReference::Unparameterized(o)) => {
                        CompiledStmt::new(move |_ev, ctx| {
                            if ctx.fresh_outputs.contains(o) {
                                let v = ctx.memory.get_output_value(o, 0);
                                ctx.memory.sliding_windows[idx].accept_value(v, ctx.ts);
                            }
                        })
                    }
                    StreamReference::Out(OutputReference::Parameterized(_)) => {
                        unreachable!()
                    }
                };

                match origin_pacing {
                    Guard::GlobalFreq(_) => CompiledStmt::new(move |_ev, ctx| {
                        ctx.memory.sliding_windows[idx].activate(Duration::new(0, 0));
                        add_fresh_value.execute(_ev, ctx);
                    }),
                    Guard::LocalFreq(_) => CompiledStmt::new(move |_ev, ctx| {
                        ctx.memory.sliding_windows[idx].activate(ctx.ts);
                        add_fresh_value.execute(_ev, ctx);
                    }),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }
}
