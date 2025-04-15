use std::time::Duration;

use bit_set::BitSet;
use streamir_lib::{
    formatter::{expressions::ExprFormatter, guards::GuardFormatter},
    ir::{expressions::Expr, Guard, LocalFreq, OutputReference, StreamReference},
};

use super::{Closuregen, EvaluationContext, Event};

type CompiledGuardFn<Event> = Box<dyn Fn(&Event, &mut EvaluationContext) -> bool>;
pub(crate) struct CompiledGuard<Event>(CompiledGuardFn<Event>);

impl<Event> CompiledGuard<Event> {
    fn new(f: impl Fn(&Event, &mut EvaluationContext) -> bool + 'static) -> Self {
        Self(Box::new(f))
    }

    pub(crate) fn execute(&self, e: &Event, ctx: &mut EvaluationContext) -> bool {
        (self.0)(e, ctx)
    }
}

impl<E: Event + 'static> GuardFormatter for Closuregen<E> {
    type Return = CompiledGuard<E>;

    fn stream(&self, sr: StreamReference) -> Self::Return {
        match sr {
            StreamReference::In(i) => {
                CompiledGuard::new(move |e, _ctx| E::eval_activation_condition(i, e))
            }
            StreamReference::Out(_output_reference) => {
                unimplemented!("Guard are only implement over input streams")
            }
        }
    }

    fn alive(&self, sr: StreamReference) -> Self::Return {
        match sr {
            StreamReference::In(_) => CompiledGuard::new(move |_, _| true),
            StreamReference::Out(OutputReference::Unparameterized(idx)) => {
                CompiledGuard::new(move |_, ctx| ctx.memory.stream_is_alive(idx))
            }
            StreamReference::Out(OutputReference::Parameterized(idx)) => {
                CompiledGuard::new(move |_, ctx| {
                    ctx.memory
                        .instance_is_alive(idx, ctx.parameter.as_ref().unwrap())
                })
            }
        }
    }

    fn dynamic(&self, expr: Expr) -> Self::Return {
        let expr = self.expr(expr);
        CompiledGuard::new(move |_ev, ctx| expr.execute(ctx).as_bool())
    }

    fn global_freq(&self, duration: Duration) -> Self::Return {
        let duration = duration.as_nanos();
        let (_, indices) = self
            .static_schedule
            .as_ref()
            .unwrap()
            .deadlines
            .iter()
            .enumerate()
            .fold((0, BitSet::new()), |(acc_time, mut bs), (idx, dl_time)| {
                let acc_time = acc_time + dl_time.as_nanos();
                if acc_time % duration == 0 {
                    bs.insert(idx);
                }
                (acc_time, bs)
            });
        CompiledGuard::new(move |event, _| E::eval_static_stream(&indices, event))
    }

    fn local_freq(&self, id: usize) -> Self::Return {
        let LocalFreq {
            dur: _,
            sr,
            reference: _,
        } = self.lr2local_freq[&id];
        match sr {
            OutputReference::Unparameterized(_) => {
                CompiledGuard::new(move |event, ctx| Event::eval_dynamic_stream(ctx, sr, event))
            }
            OutputReference::Parameterized(_) => {
                CompiledGuard::new(move |event, ctx| Event::eval_dynamic_instance(ctx, sr, event))
            }
        }
    }

    fn and(&self, lhs: Guard, rhs: Guard) -> Self::Return {
        let lhs = self.guard(lhs);
        let rhs = self.guard(rhs);
        CompiledGuard::new(move |e, ctx| lhs.execute(e, ctx) && rhs.execute(e, ctx))
    }

    fn or(&self, lhs: Guard, rhs: Guard) -> Self::Return {
        let lhs = self.guard(lhs);
        let rhs = self.guard(rhs);
        CompiledGuard::new(move |e, ctx| lhs.execute(e, ctx) || rhs.execute(e, ctx))
    }

    fn constant(&self, b: bool) -> Self::Return {
        CompiledGuard::new(move |_, _| b)
    }

    fn fast_and(&self, inner: Vec<StreamReference>) -> Self::Return {
        let ac_set = inner.iter().map(|sr| sr.in_idx()).collect::<BitSet>();
        CompiledGuard::new(move |_, ctx| ac_set.is_subset(ctx.fresh_inputs))
    }
}
