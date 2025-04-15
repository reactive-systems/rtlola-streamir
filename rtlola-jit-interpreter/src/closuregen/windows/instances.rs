use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::closuregen::expressions::CompiledExpr;
use crate::closuregen::EvaluationContext;
use crate::memory::Instance;
use crate::value::Value;
use crate::Time;

use super::aggregations::Aggregation;
use super::InstanceWindowTrait;

#[derive(Debug)]
pub(crate) struct InstanceWindow<OP: Aggregation, const FRESH: bool> {
    sr: usize,
    phantom: PhantomData<OP>,
}

impl<OP: Aggregation, const FRESH: bool> InstanceWindow<OP, FRESH> {
    pub(crate) fn new(sr: usize) -> Self {
        Self {
            sr,
            phantom: PhantomData,
        }
    }

    fn reduce(iter: impl Iterator<Item = Value>, ts: Time) -> Value {
        iter.map(|v| OP::from_value(v, ts))
            .reduce(|lhs, rhs| lhs + rhs)
            .unwrap_or(OP::default(ts))
            .lower()
    }
}

impl<OP: Aggregation> InstanceWindowTrait for InstanceWindow<OP, false> {
    fn get_value(&self, ctx: &EvaluationContext<'_>) -> Value {
        let iter = ctx
            .memory
            .instances(self.sr)
            .map(|instance| ctx.memory.get_output_instance_value(self.sr, instance, 0));
        Self::reduce(iter, ctx.ts)
    }
}

impl<OP: Aggregation> InstanceWindowTrait for InstanceWindow<OP, true> {
    fn get_value(&self, ctx: &EvaluationContext<'_>) -> Value {
        let iter = ctx.instances[self.sr]
            .eval
            .iter()
            .map(|instance| ctx.memory.get_output_instance_value(self.sr, instance, 0));
        Self::reduce(iter, ctx.ts)
    }
}

pub(crate) struct ConditionalInstanceWindow<OP: Aggregation, const FRESH: bool> {
    condition: CompiledExpr,
    sr: usize,
    phantom: PhantomData<OP>,
}

impl<OP: Aggregation, const FRESH: bool> Debug for ConditionalInstanceWindow<OP, FRESH> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionalInstanceWindow")
            .field("sr", &self.sr)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<OP: Aggregation, const FRESH: bool> ConditionalInstanceWindow<OP, FRESH> {
    pub(crate) fn new(condition: CompiledExpr, sr: usize) -> Self {
        Self {
            condition,
            sr,
            phantom: PhantomData,
        }
    }

    fn reduce<'a>(
        &self,
        iter: impl Iterator<Item = &'a Rc<Instance>>,
        ts: Time,
        sr: usize,
        ctx: &EvaluationContext<'a>,
    ) -> Value {
        iter.filter_map(|instance| {
            *ctx.lambda_parameter.borrow_mut() = Some(instance.clone());
            self.condition
                .execute(ctx)
                .as_bool()
                .then(|| ctx.memory.get_output_instance_value(sr, instance, 0))
        })
        .filter(|v| !matches!(v, Value::None))
        .map(|v| OP::from_value(v, ts))
        .reduce(|lhs, rhs| lhs + rhs)
        .unwrap_or(OP::default(ts))
        .lower()
    }
}

impl<OP: Aggregation> InstanceWindowTrait for ConditionalInstanceWindow<OP, false> {
    fn get_value(&self, ctx: &EvaluationContext<'_>) -> Value {
        let iter = ctx.memory.instances(self.sr);
        self.reduce(iter, ctx.ts, self.sr, ctx)
    }
}

impl<OP: Aggregation> InstanceWindowTrait for ConditionalInstanceWindow<OP, true> {
    fn get_value(&self, ctx: &EvaluationContext<'_>) -> Value {
        let iter = ctx.instances[self.sr].eval.iter();
        self.reduce(iter, ctx.ts, self.sr, ctx)
    }
}
