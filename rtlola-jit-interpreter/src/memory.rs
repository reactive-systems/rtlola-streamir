use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;
use std::time::Duration;

use itertools::Itertools;
use streamir_lib::ir::memory::{Memory as IrMemory, StreamMemory};
use streamir_lib::ir::{InputReference, OutputReference, StreamReference, WindowReference};

use crate::closuregen::windows::{InstanceWindowTrait, SlidingWindowTrait};
use crate::closuregen::{EvaluationContext, Event};
use crate::value::Value;
use crate::{Closuregen, Time};

pub(crate) type Instance = Vec<Value>;

#[derive(Debug, Clone)]
pub(crate) struct InstanceBuffer {
    buffer: VecDeque<Value>,
    memory_bound: usize,
}

impl InstanceBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            memory_bound: capacity,
        }
    }

    #[inline]
    fn replace_value(&mut self, value: Value) {
        match self.buffer.get_mut(0) {
            Some(item) => *item = value,
            None => self.buffer.push_front(value),
        }
    }

    #[inline]
    fn push_value(&mut self) {
        if self.buffer.len() == self.memory_bound {
            self.buffer.pop_back();
        }
        self.buffer.push_front(Value::None);
    }

    #[inline]
    pub(crate) fn get_value(&self, offset: u32) -> Value {
        self.buffer
            .get(offset as usize)
            .cloned()
            .unwrap_or(Value::None)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum UnparameterizedStreamBuffer {
    SingleValue(Value),
    Static(InstanceBuffer),
    Dynamic {
        buffer: Option<InstanceBuffer>,
        memory_bound: usize,
    },
}

impl From<&IrMemory> for UnparameterizedStreamBuffer {
    fn from(value: &IrMemory) -> Self {
        match &value.buffer {
            StreamMemory::NoMemory
            | StreamMemory::Static(streamir_lib::ir::memory::StreamBuffer::SingleValue) => {
                UnparameterizedStreamBuffer::SingleValue(Value::None)
            }
            StreamMemory::Static(streamir_lib::ir::memory::StreamBuffer::Bounded(b)) => {
                UnparameterizedStreamBuffer::Static(InstanceBuffer::new(*b))
            }
            StreamMemory::Static(streamir_lib::ir::memory::StreamBuffer::UnBounded) => {
                unimplemented!("No implementation for an unbounded streambuffer")
            }
            StreamMemory::Dynamic {
                buffer,
                has_spawn: _,
                has_close: _,
            } => UnparameterizedStreamBuffer::Dynamic {
                buffer: None,
                memory_bound: buffer.bound().unwrap(),
            },
            StreamMemory::Instances { .. } => {
                unreachable!("Streambuffer invalid for Unparameterized Stream")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ParameterizedStreamBuffer {
    instances: HashMap<Rc<Instance>, InstanceBuffer>,
    alive_instances: HashSet<Rc<Instance>>,
    memory_bound: usize,
}

impl From<&IrMemory> for ParameterizedStreamBuffer {
    fn from(value: &IrMemory) -> Self {
        match &value.buffer {
            StreamMemory::NoMemory | StreamMemory::Static(_) | StreamMemory::Dynamic { .. } => {
                unreachable!("Streambuffer invalid for Parameterized Stream")
            }
            StreamMemory::Instances {
                buffer,
                parameter: _,
            } => {
                let memory_bound = buffer.bound().unwrap();
                Self {
                    instances: HashMap::new(),
                    alive_instances: HashSet::new(),
                    memory_bound,
                }
            }
        }
    }
}

impl ParameterizedStreamBuffer {
    #[inline]
    fn replace_instance_value(&mut self, instance: &Rc<Instance>, value: Value) {
        self.instances
            .get_mut(instance)
            .expect("expect intance")
            .replace_value(value)
    }

    #[inline]
    fn push_instance_value(&mut self, instance: &Rc<Instance>) {
        self.instances
            .get_mut(instance)
            .expect("instance must exist")
            .push_value()
    }

    #[inline]
    fn get_instance_value(&self, instance: &Instance, offset: u32) -> Value {
        self.instances
            .get(instance)
            .as_ref()
            .map(|buffer| buffer.get_value(offset))
            .unwrap_or(Value::None)
    }

    #[inline]
    fn spawn_instance(&mut self, instance: Rc<Instance>) -> bool {
        if !self.instances.contains_key(&instance) {
            self.instances
                .insert(instance.clone(), InstanceBuffer::new(self.memory_bound));
            self.alive_instances.insert(instance);
            true
        } else {
            false
        }
    }

    #[inline]
    fn close_instance(&mut self, instance: &Rc<Instance>) {
        debug_assert!(self.instances.contains_key(instance));
        self.instances.remove(instance);
        self.alive_instances.remove(instance);
    }

    #[inline]
    pub(crate) fn take_instances(&mut self) -> HashSet<Rc<Instance>> {
        std::mem::take(&mut self.alive_instances)
    }

    #[inline]
    pub(crate) fn return_instances(&mut self, instances: HashSet<Rc<Instance>>) {
        self.alive_instances = instances
    }

    #[inline]
    fn instance_is_alive(&self, instance: &Vec<Value>) -> bool {
        self.instances.contains_key(instance)
    }

    #[inline]
    fn instances(&self) -> impl Iterator<Item = &Rc<Instance>> + '_ {
        self.alive_instances.iter()
    }
}

impl UnparameterizedStreamBuffer {
    #[inline]
    fn replace_value(&mut self, value: Value) {
        match self {
            UnparameterizedStreamBuffer::SingleValue(buffer) => *buffer = value,
            UnparameterizedStreamBuffer::Static(buffer)
            | UnparameterizedStreamBuffer::Dynamic {
                buffer: Some(buffer),
                memory_bound: _,
            } => buffer.replace_value(value),
            UnparameterizedStreamBuffer::Dynamic { buffer: None, .. } => {
                unreachable!()
            }
        }
    }

    #[inline]
    fn push_value(&mut self) {
        match self {
            UnparameterizedStreamBuffer::SingleValue(buffer) => *buffer = Value::None,
            UnparameterizedStreamBuffer::Static(stream_buffer)
            | UnparameterizedStreamBuffer::Dynamic {
                buffer: Some(stream_buffer),
                ..
            } => stream_buffer.push_value(),
            UnparameterizedStreamBuffer::Dynamic { buffer: None, .. } => {
                unreachable!()
            }
        }
    }

    #[inline]
    fn get_value(&self, offset: u32) -> Value {
        match self {
            UnparameterizedStreamBuffer::SingleValue(value) => value.clone(),
            UnparameterizedStreamBuffer::Static(stream_buffer)
            | UnparameterizedStreamBuffer::Dynamic {
                buffer: Some(stream_buffer),
                ..
            } => stream_buffer.get_value(offset),
            UnparameterizedStreamBuffer::Dynamic { buffer: None, .. } => Value::None,
        }
    }

    #[inline]
    fn spawn_stream(&mut self) -> bool {
        match self {
            UnparameterizedStreamBuffer::Static(_)
            | UnparameterizedStreamBuffer::SingleValue(_) => false,
            UnparameterizedStreamBuffer::Dynamic {
                buffer: buffer @ None,
                memory_bound,
            } => {
                *buffer = Some(InstanceBuffer::new(*memory_bound));
                true
            }
            UnparameterizedStreamBuffer::Dynamic {
                buffer: Some(_), ..
            } => false,
        }
    }

    #[inline]
    fn close_stream(&mut self) {
        match self {
            UnparameterizedStreamBuffer::SingleValue(_)
            | UnparameterizedStreamBuffer::Static(_)
            | UnparameterizedStreamBuffer::Dynamic { buffer: None, .. } => {
                unreachable!()
            }

            UnparameterizedStreamBuffer::Dynamic {
                buffer: buffer @ Some(_),
                ..
            } => *buffer = None,
        }
    }

    #[inline]
    fn stream_is_alive(&self) -> bool {
        match self {
            UnparameterizedStreamBuffer::SingleValue(_)
            | UnparameterizedStreamBuffer::Static(_) => true,
            UnparameterizedStreamBuffer::Dynamic {
                buffer,
                memory_bound: _,
            } => buffer.is_some(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Memory {
    inputs: Vec<UnparameterizedStreamBuffer>,
    parameterized_outputs: Vec<ParameterizedStreamBuffer>,
    unparameterized_outputs: Vec<UnparameterizedStreamBuffer>,
    pub(crate) sliding_windows: Vec<Box<dyn SlidingWindowTrait>>,
    pub(crate) instanced_sliding_windows: Vec<HashMap<Rc<Instance>, Box<dyn SlidingWindowTrait>>>,
    instance_windows: Vec<Box<dyn InstanceWindowTrait>>,
}

impl Memory {
    pub(crate) fn shift_input(&mut self, sr: InputReference) {
        self.inputs[sr].push_value()
    }

    pub(crate) fn eval_input(&mut self, sr: InputReference, value: Value) {
        self.inputs[sr].replace_value(value);
    }

    pub(crate) fn shift_output_value(&mut self, sr: usize) {
        self.unparameterized_outputs[sr].push_value()
    }

    pub(crate) fn eval_output_value(&mut self, sr: usize, value: Value) {
        self.unparameterized_outputs[sr].replace_value(value);
    }

    pub(crate) fn shift_output_instance(&mut self, sr: usize, instance: &Rc<Instance>) {
        self.parameterized_outputs[sr].push_instance_value(instance)
    }

    pub(crate) fn eval_output_instance(
        &mut self,
        sr: usize,
        instance: &Rc<Instance>,
        value: Value,
    ) {
        self.parameterized_outputs[sr].replace_instance_value(instance, value)
    }

    pub(crate) fn get_input_value(&self, sr: InputReference, offset: u32) -> Value {
        self.inputs[sr].get_value(offset)
    }

    pub(crate) fn get_output_value(&self, sr: usize, offset: u32) -> Value {
        self.unparameterized_outputs[sr].get_value(offset)
    }

    pub(crate) fn get_output_instance_value(
        &self,
        sr: usize,
        instance: &Instance,
        offset: u32,
    ) -> Value {
        self.parameterized_outputs[sr].get_instance_value(instance, offset)
    }

    pub(crate) fn spawn_instance(&mut self, sr: usize, instance: Rc<Instance>) -> bool {
        self.parameterized_outputs[sr].spawn_instance(instance)
    }

    pub(crate) fn spawn_stream(&mut self, sr: usize) -> bool {
        self.unparameterized_outputs[sr].spawn_stream()
    }

    pub(crate) fn stream_is_alive(&self, sr: usize) -> bool {
        self.unparameterized_outputs[sr].stream_is_alive()
    }

    pub(crate) fn instance_is_alive(&self, sr: usize, instance: &Vec<Value>) -> bool {
        self.parameterized_outputs[sr].instance_is_alive(instance)
    }

    pub(crate) fn close_stream(&mut self, sr: usize) {
        self.unparameterized_outputs[sr].close_stream();
    }

    pub(crate) fn close_instance(&mut self, sr: usize, instance: &Rc<Instance>) {
        self.parameterized_outputs[sr].close_instance(instance);
    }

    pub(crate) fn sliding_window_accept_value(
        &mut self,
        wref: WindowReference,
        v: Value,
        ts: Time,
    ) {
        self.sliding_windows[wref.idx()].accept_value(v, ts);
    }

    pub(crate) fn instance_sliding_window_accept_value(
        &mut self,
        wref: WindowReference,
        v: Value,
        instance: &Instance,
        ts: Time,
    ) {
        if let Some(window) = self.instanced_sliding_windows[wref.idx()].get_mut(instance) {
            window.accept_value(v, ts);
        }
    }

    pub(crate) fn sliding_window_get_value(&self, idx: usize, ts: Time) -> Value {
        self.sliding_windows[idx].get_value(ts)
    }

    pub(crate) fn instance_sliding_window_get_value(
        &self,
        idx: usize,
        instance: &Instance,
        ts: Time,
    ) -> Value {
        self.instanced_sliding_windows[idx]
            .get(instance)
            .unwrap()
            .get_value(ts)
    }

    pub(crate) fn instance_window_get_value(
        &self,
        idx: usize,
        ctx: &EvaluationContext<'_>,
    ) -> Value {
        self.instance_windows[idx].get_value(ctx)
    }

    pub(crate) fn instances(&self, idx: usize) -> impl Iterator<Item = &Rc<Instance>> + '_ {
        self.parameterized_outputs[idx].instances()
    }

    pub(crate) fn instance_buffer(&mut self, sr: usize) -> &mut ParameterizedStreamBuffer {
        &mut self.parameterized_outputs[sr]
    }
}

impl<E: Event + 'static> Closuregen<E> {
    pub(crate) fn memory(&self) -> Memory {
        let mut sliding_windows = Vec::new();
        let mut instanced_sliding_windows = Vec::new();
        let mut instance_windows = Vec::new();

        self.wref2window
            .iter()
            .sorted_by_key(|(w, _)| *w)
            .for_each(|(wref, w)| match wref {
                WindowReference::Sliding(_) => {
                    sliding_windows.push(self.sliding_window(w, Duration::new(0, 0)));
                    instanced_sliding_windows.push(HashMap::new())
                }
                WindowReference::Discrete(_) => unimplemented!(),
                WindowReference::Instance(_) => {
                    instance_windows.push(self.instance_window(w.clone()))
                }
            });
        let (inputs, outputs): (Vec<_>, Vec<_>) =
            self.sr2memory.iter().partition_map(|(sr, mem)| match sr {
                StreamReference::In(i) => itertools::Either::Left((i, mem)),
                StreamReference::Out(o) => itertools::Either::Right((o, mem)),
            });
        let (unparameterized_outputs, parameterized_outputs): (Vec<_>, Vec<_>) =
            outputs.into_iter().partition_map(|(o, mem)| match o {
                OutputReference::Unparameterized(i) => itertools::Either::Left((i, mem)),
                OutputReference::Parameterized(i) => itertools::Either::Right((i, mem)),
            });
        Memory {
            inputs: inputs
                .into_iter()
                .sorted_by_key(|(i, _mem)| *i)
                .map(|(_i, mem)| UnparameterizedStreamBuffer::from(mem))
                .collect(),
            parameterized_outputs: parameterized_outputs
                .into_iter()
                .sorted_by_key(|(i, _mem)| *i)
                .map(|(_i, mem)| ParameterizedStreamBuffer::from(mem))
                .collect(),
            unparameterized_outputs: unparameterized_outputs
                .into_iter()
                .sorted_by_key(|(i, _mem)| *i)
                .map(|(_i, mem)| UnparameterizedStreamBuffer::from(mem))
                .collect(),
            sliding_windows,
            instanced_sliding_windows,
            instance_windows,
        }
    }
}
