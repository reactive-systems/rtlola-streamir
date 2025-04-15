use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

use bit_set::BitSet;
use streamir_lib::ir::{
    memory::Memory as IrMemory, windows::Window, InputReference, LocalFreq, LocalFreqRef,
    OutputReference, StreamReference, WindowReference,
};

use crate::{
    memory::{Instance, Memory},
    schedule::{DeadlineEvent, Schedule, StaticSchedule},
    value::Value,
    Inputs, Time,
};

mod expressions;
mod guards;
pub(crate) mod statements;
pub(crate) mod windows;

pub(crate) struct Closuregen<E: Event> {
    pub(crate) sr2memory: HashMap<StreamReference, IrMemory>,
    pub(crate) wref2window: HashMap<WindowReference, Window>,
    pub(crate) lr2local_freq: HashMap<LocalFreqRef, LocalFreq>,
    pub(crate) static_schedule: Option<StaticSchedule>,
    phantom: PhantomData<E>,
}

impl<E: Event> Closuregen<E> {
    pub fn new(
        sr2memory: HashMap<StreamReference, IrMemory>,
        wref2window: HashMap<WindowReference, Window>,
        lr2local_freq: HashMap<LocalFreqRef, LocalFreq>,
        static_schedule: Option<StaticSchedule>,
    ) -> Self {
        Self {
            sr2memory,
            wref2window,
            lr2local_freq,
            static_schedule,
            phantom: PhantomData,
        }
    }

    pub fn with_event<O: Event>(self) -> Closuregen<O> {
        let Self {
            sr2memory,
            wref2window,
            lr2local_freq,
            static_schedule,
            ..
        } = self;
        Closuregen {
            sr2memory,
            wref2window,
            lr2local_freq,
            static_schedule,
            phantom: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InstanceCollection {
    pub(crate) spawned: Option<Rc<Instance>>,
    pub(crate) eval: Vec<Rc<Instance>>,
    pub(crate) closed: Vec<Rc<Instance>>,
}

impl InstanceCollection {
    pub(crate) fn new() -> Self {
        Self {
            spawned: None,
            eval: Vec::new(),
            closed: Vec::new(),
        }
    }

    pub(crate) fn spawn(&mut self, instance: Rc<Instance>) {
        debug_assert!(self.spawned.is_none());
        self.spawned = Some(instance);
    }

    pub(crate) fn eval(&mut self, instance: Rc<Instance>) {
        debug_assert!(!self.eval.iter().any(|i| i == &instance));
        self.eval.push(instance);
    }

    pub(crate) fn close(&mut self, instance: Rc<Instance>) {
        debug_assert!(!self.closed.iter().any(|i| i == &instance));
        self.closed.push(instance);
    }

    fn clear(&mut self) {
        self.spawned = None;
        self.eval.clear();
        self.closed.clear();
    }
}

pub(crate) struct EvaluationContext<'e> {
    ts: Time,
    pub(crate) memory: &'e mut Memory,
    pub(crate) fresh_inputs: &'e mut BitSet,
    pub(crate) fresh_outputs: &'e mut BitSet,
    pub(crate) spawned_streams: &'e mut BitSet,
    pub(crate) closing_streams: &'e mut BitSet,
    pub(crate) closing_param_local: &'e mut BitSet,
    pub(crate) closing_unparam_local: &'e mut BitSet,
    pub(crate) instances: &'e mut Vec<InstanceCollection>,
    pub(crate) schedule: &'e mut Schedule,
    parameter: Option<Rc<Instance>>,
    lambda_parameter: RefCell<Option<Rc<Instance>>>,
}

impl<'a> EvaluationContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ts: Time,
        memory: &'a mut Memory,
        fresh_inputs: &'a mut BitSet,
        fresh_outputs: &'a mut BitSet,
        spawned_streams: &'a mut BitSet,
        closing_streams: &'a mut BitSet,
        closing_param_local: &'a mut BitSet,
        closing_unparam_local: &'a mut BitSet,
        instances: &'a mut Vec<InstanceCollection>,
        schedule: &'a mut Schedule,
    ) -> EvaluationContext<'a> {
        EvaluationContext {
            ts,
            memory,
            fresh_inputs,
            fresh_outputs,
            spawned_streams,
            closing_streams,
            closing_param_local,
            closing_unparam_local,
            instances,
            parameter: None,
            lambda_parameter: RefCell::new(None),
            schedule,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.fresh_inputs.clear();
        self.fresh_outputs.clear();
        self.closing_streams.iter().for_each(|o| {
            self.memory.close_stream(o);
        });
        self.closing_unparam_local.iter().for_each(|s| {
            self.schedule
                .dynamic_schedule
                .remove_stream(OutputReference::Unparameterized(s));
        });
        self.closing_streams.clear();
        self.closing_unparam_local.clear();
        self.instances
            .iter_mut()
            .enumerate()
            .for_each(|(o, collection)| {
                collection.closed.iter().for_each(|inst| {
                    if self.closing_param_local.contains(o) {
                        self.schedule
                            .dynamic_schedule
                            .remove_instance(OutputReference::Parameterized(o), inst);
                    }
                    self.memory.close_instance(o, inst);
                });
                collection.clear();
            });
        self.closing_param_local.clear();
        self.schedule.clear();
    }
}

pub(crate) trait Event {
    fn get_input(sr: InputReference, inputs: &Self) -> &Value;

    fn eval_activation_condition(in_ref: usize, inputs: &Self) -> bool;

    fn eval_dynamic_stream(ctx: &EvaluationContext, sr: OutputReference, inputs: &Self) -> bool;

    fn eval_dynamic_instance(ctx: &EvaluationContext, sr: OutputReference, inputs: &Self) -> bool;

    fn eval_static_stream(indices: &BitSet, inputs: &Self) -> bool;
}

impl Event for Inputs {
    fn get_input(sr: InputReference, inputs: &Inputs) -> &Value {
        inputs.0[sr].as_ref().unwrap()
    }

    fn eval_activation_condition(in_ref: usize, inputs: &Self) -> bool {
        inputs.0[in_ref].is_some()
    }

    fn eval_dynamic_stream(_ctx: &EvaluationContext, _sr: OutputReference, _inputs: &Self) -> bool {
        unreachable!()
    }

    fn eval_dynamic_instance(
        _ctx: &EvaluationContext,
        _sr: OutputReference,
        _inputs: &Self,
    ) -> bool {
        unreachable!()
    }

    fn eval_static_stream(_indices: &BitSet, _inputs: &Self) -> bool {
        unreachable!()
    }
}

impl Event for DeadlineEvent {
    fn get_input(_sr: InputReference, _inputs: &DeadlineEvent) -> &Value {
        unreachable!()
    }

    fn eval_activation_condition(_in_ref: usize, _inputs: &Self) -> bool {
        unreachable!()
    }

    fn eval_dynamic_stream(ctx: &EvaluationContext, sr: OutputReference, inputs: &Self) -> bool {
        inputs.is_dynamic
            && ctx
                .schedule
                .get_dynamic()
                .streams
                .contains(sr.unparameterized_idx())
    }

    fn eval_dynamic_instance(ctx: &EvaluationContext, sr: OutputReference, inputs: &Self) -> bool {
        inputs.is_dynamic
            && ctx.schedule.get_dynamic().instances[sr.parameterized_idx()]
                .contains(ctx.parameter.as_ref().unwrap())
    }

    fn eval_static_stream(indices: &BitSet, inputs: &Self) -> bool {
        inputs.is_static.is_some_and(|i| indices.contains(i))
    }
}
