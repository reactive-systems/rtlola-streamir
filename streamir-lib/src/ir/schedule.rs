use std::{collections::HashMap, time::Duration};

use rtlola_frontend::mir::{self};

use super::{OutputReference, StreamReference};

#[derive(Debug, Clone)]
/// Represents the precomuted schedule for global periodic output streams
pub struct StaticSchedule {
    /// The hyperperiod of the schedule
    pub hyper_period: Duration,
    /// A list of deadlines
    pub deadlines: Vec<Deadline>,
}

#[derive(Debug, Clone)]
/// Represents a deadline of the static schedule. A set of output streams that need to be evaluated at the same time.
pub struct Deadline {
    /// The time duration after the last deadline
    pub pause: Duration,
    /// The set of evaluations due at this deadline
    pub due: Vec<Task>,
}

#[derive(Debug, Clone, Copy)]
/// Represents a task that is due at a deadline of the static schedule
pub enum Task {
    /// A (global-periodic) output stream is spawned
    Spawn(OutputReference),
    /// A (global-periodic) output stream is evaluated
    Eval(OutputReference),
    /// A (global-periodic) output stream is closed
    Close(OutputReference),
}

impl StaticSchedule {
    pub(crate) fn new(
        mir_schedule: mir::Schedule,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Option<Self> {
        let mir::Schedule {
            hyper_period,
            deadlines,
        } = mir_schedule;

        let hyper_period = hyper_period?;
        let deadlines = deadlines
            .into_iter()
            .map(|deadline| Deadline::from_mir_dl(deadline, sr2sr))
            .collect();

        Some(Self {
            hyper_period,
            deadlines,
        })
    }
}

impl Deadline {
    fn from_mir_dl(
        dl: mir::Deadline,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Deadline {
        let mir::Deadline { pause, due } = dl;
        let due = due
            .into_iter()
            .map(|task| Task::from_mir_task(task, sr2sr))
            .collect();
        Self { pause, due }
    }
}

impl Task {
    fn from_mir_task(
        mir_task: mir::Task,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Task {
        match mir_task {
            mir::Task::Evaluate(sr) => Task::Eval(sr2sr[&mir::StreamReference::Out(sr)].out_idx()),
            mir::Task::Spawn(sr) => Task::Spawn(sr2sr[&mir::StreamReference::Out(sr)].out_idx()),
            mir::Task::Close(sr) => Task::Close(sr2sr[&mir::StreamReference::Out(sr)].out_idx()),
        }
    }
}
