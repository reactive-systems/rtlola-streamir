use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use bit_set::BitSet;
use streamir_lib::ir::{self, OutputReference, StreamIr};

use crate::{memory::Instance, Time};

#[derive(Debug, Clone, Eq, PartialEq)]
struct State {
    time: Time,
    deadline: Deadline,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct Deadline {
    sr: OutputReference,
    instance: Option<Rc<Instance>>,
}

#[derive(Debug, Clone)]
pub(crate) struct DynamicDeadlines {
    pub(crate) streams: BitSet,
    pub(crate) instances: Vec<HashSet<Rc<Instance>>>,
    pub time: Time,
}

impl DynamicDeadlines {
    fn new(num_outputs: usize) -> Self {
        Self {
            streams: BitSet::with_capacity(num_outputs),
            instances: vec![HashSet::new(); num_outputs],
            time: Duration::new(0, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DynamicSchedule {
    queue: BinaryHeap<State>,
    sr2period: HashMap<Deadline, Time>,
    current: DynamicDeadlines,
}

impl DynamicSchedule {
    pub(crate) fn new(ir: &StreamIr) -> Self {
        Self {
            queue: BinaryHeap::new(),
            sr2period: HashMap::new(),
            current: DynamicDeadlines::new(ir.num_outputs()),
        }
    }

    pub(crate) fn next<const INCLUSIVE: bool>(&mut self, until: Time) -> bool {
        let mut has_event = false;
        while let Some(state) = self.queue.peek() {
            if !self.sr2period.contains_key(&state.deadline) {
                self.queue.pop();
                continue;
            }
            if (!INCLUSIVE && state.time >= until) || state.time > until {
                return has_event;
            }

            if has_event && state.time > self.current.time {
                return has_event;
            }

            let mut state = self.queue.pop().unwrap();

            self.current.time = state.time;
            state.time += self.sr2period[&state.deadline];
            if let Some(inst) = &state.deadline.instance {
                self.current.instances[state.deadline.sr.parameterized_idx()].insert(inst.clone());
            } else {
                self.current
                    .streams
                    .insert(state.deadline.sr.unparameterized_idx());
            }

            has_event = true;

            self.queue.push(state)
        }
        has_event
    }

    fn peek_time(&self) -> Option<Time> {
        self.queue.peek().map(|state| state.time)
    }

    pub(crate) fn clear_dynamic_deadlines(&mut self) {
        self.current.streams.clear();
        self.current
            .instances
            .iter_mut()
            .for_each(|inst| inst.clear());
    }

    pub(crate) fn add_stream(&mut self, time: Time, period: Time, sr: OutputReference) {
        let dl = Deadline { sr, instance: None };
        self.sr2period.insert(dl.clone(), period);
        self.queue.push(State { time, deadline: dl });
    }

    pub(crate) fn add_instance(
        &mut self,
        time: Time,
        period: Time,
        sr: OutputReference,
        instance: Rc<Instance>,
    ) {
        let dl = Deadline {
            sr,
            instance: Some(instance),
        };
        self.sr2period.insert(dl.clone(), period);
        self.queue.push(State { time, deadline: dl });
    }

    pub(crate) fn remove_stream(&mut self, sr: OutputReference) {
        self.sr2period.remove(&Deadline { sr, instance: None });
    }

    pub(crate) fn remove_instance(&mut self, sr: OutputReference, instance: &Rc<Instance>) {
        self.sr2period.remove(&Deadline {
            sr,
            instance: Some(instance.clone()),
        });
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StaticSchedule {
    pub(crate) deadlines: Vec<Time>,
    timestamp: Time,
    current_static: usize,
}

impl StaticSchedule {
    pub fn new(ir: &StreamIr) -> Option<Self> {
        let ir::StaticSchedule {
            hyper_period: _,
            deadlines,
        } = ir.static_schedule.as_ref()?;

        let deadlines: Vec<_> = deadlines.iter().map(|deadline| deadline.pause).collect();
        let timestamp = deadlines[0];

        Some(Self {
            deadlines,
            timestamp,
            current_static: 0,
        })
    }

    fn peek_time(&self) -> Time {
        self.timestamp
    }

    fn next<const INCLUSIVE: bool>(&mut self, until: Time) -> Option<usize> {
        if (INCLUSIVE && self.timestamp <= until) || self.timestamp < until {
            let r = self.current_static;
            self.current_static = (self.current_static + 1) % self.deadlines.len();
            self.timestamp += self.deadlines[self.current_static];
            Some(r)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Schedule {
    pub(crate) dynamic_schedule: DynamicSchedule,
    pub(crate) static_schedule: Option<StaticSchedule>,
}

pub(crate) struct DeadlineEvent {
    pub(crate) ts: Time,
    pub(crate) is_dynamic: bool,
    pub(crate) is_static: Option<usize>,
}

impl DeadlineEvent {
    fn new_static(ts: Time, i: usize) -> Self {
        Self {
            ts,
            is_dynamic: false,
            is_static: Some(i),
        }
    }

    fn new_dynamic(ts: Time) -> Self {
        Self {
            ts,
            is_dynamic: true,
            is_static: None,
        }
    }

    fn new_both(ts: Time, i: usize) -> Self {
        Self {
            ts,
            is_dynamic: true,
            is_static: Some(i),
        }
    }
}

impl Schedule {
    pub(crate) fn get_dynamic(&self) -> &DynamicDeadlines {
        &self.dynamic_schedule.current
    }

    pub(crate) fn clear(&mut self) {
        self.dynamic_schedule.clear_dynamic_deadlines();
    }

    pub(crate) fn new(ir: &StreamIr) -> Self {
        Self {
            dynamic_schedule: DynamicSchedule::new(ir),
            static_schedule: StaticSchedule::new(ir),
        }
    }

    pub(crate) fn next<const INCLUSIVE: bool>(&mut self, until: Time) -> Option<DeadlineEvent> {
        let dynamic_deadline = self.dynamic_schedule.peek_time();
        let static_deadline = self.static_schedule.as_ref().map(|s| s.peek_time());
        match (dynamic_deadline, static_deadline) {
            (None, None) => None,
            (Some(ts), None) => self
                .dynamic_schedule
                .next::<INCLUSIVE>(until)
                .then(|| DeadlineEvent::new_dynamic(ts)),
            (None, Some(ts)) => self
                .static_schedule
                .as_mut()
                .unwrap()
                .next::<INCLUSIVE>(until)
                .map(|r| DeadlineEvent::new_static(ts, r)),
            (Some(dynamic_deadline), Some(static_deadline)) => {
                match dynamic_deadline.cmp(&static_deadline) {
                    Ordering::Less => self
                        .dynamic_schedule
                        .next::<INCLUSIVE>(until)
                        .then(|| DeadlineEvent::new_dynamic(dynamic_deadline)),
                    Ordering::Equal => self
                        .static_schedule
                        .as_mut()
                        .unwrap()
                        .next::<INCLUSIVE>(until)
                        .map(|r| {
                            let b = self.dynamic_schedule.next::<INCLUSIVE>(until);
                            debug_assert!(b);
                            DeadlineEvent::new_both(dynamic_deadline, r)
                        }),
                    Ordering::Greater => self
                        .static_schedule
                        .as_mut()
                        .unwrap()
                        .next::<INCLUSIVE>(until)
                        .map(|r| DeadlineEvent::new_static(static_deadline, r)),
                }
            }
        }
    }
}
