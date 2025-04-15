//! An interpreter based one JIT compilation of the StreamIR

#![forbid(unused_must_use)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

use std::time::Duration;

use bit_set::BitSet;
use closuregen::{statements::CompiledStmt, Closuregen, EvaluationContext, InstanceCollection};
use memory::Memory;
use schedule::{DeadlineEvent, Schedule};
use streamir_lib::{
    formatter::statements::StmtFormatter,
    ir::StreamIr,
    optimize, optimize_all,
    rewrite_rules::{EvaluateGuards, FastGuards},
};
use value::Value;
use verdict::{TotalIncremental, Verdict, VerdictFactory};

mod closuregen;
pub mod csv;
mod memory;
mod schedule;
#[cfg(test)]
mod tests;
mod value;
pub mod verdict;

type Time = Duration;

#[derive(Debug, Clone)]
/// Represents the input values of a single input event to the monitor.
///
/// Consists of a list of optional values, where None indicates that the input stream does not receive a new
/// value at that time.
/// The list is sorted according to the stream reference index.
pub struct Inputs(pub Vec<Option<Value>>);

/// Represents the main monitor struct holding the state of the monitor between evaluation cycles
pub struct Monitor {
    /// Holds current and old values of streams and windows
    memory: Memory,
    /// The StreamIR statement evaluating the event-based fragment of the specification
    event_prog: CompiledStmt<Inputs>,
    /// The StreamIR statement evaluating the time-based fragment of the specification
    timed_prog: CompiledStmt<DeadlineEvent>,
    /// Marks inputs that received a new value during the current evaluation cycle
    fresh_inputs: BitSet,
    /// Marks (unparameterized) outputs which received a new value during the evaluation cycle
    fresh_outputs: BitSet,
    /// Marks (unparameterized) outputs which where spawned during the evaluation cycle
    spawned_streams: BitSet,
    /// Marks (unparameterized) outputs which where closed during the evaluation cycle
    closing_streams: BitSet,
    /// Marks (unparameterized) local-periodic outputs which where closed during the evaluation cycle
    closing_unparam_local: BitSet,
    /// Marks (parameterized) local-periodic outputs which where closed during the evaluation cycle
    closing_param_local: BitSet,
    /// Holds information about parameterized outputs (fresh, spawned, eval and close during current cycle)
    instances: Vec<InstanceCollection>,
    /// Holds the schedule for periodic stream evaluations
    schedule: Schedule,
    /// Constructs a verdict after each cycle
    verdict_factory: VerdictFactory,
}

impl std::fmt::Debug for Monitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Monitor")
            .field("memory", &self.memory)
            .finish()
    }
}

impl Monitor {
    /// Construct a new monitor for the given StreamIR
    ///
    /// Performs partial evaluation to split the IR into event-based and time-based fragment,
    /// and performs JIT compilation of both parts.
    pub fn build(ir: StreamIr, optimized: bool) -> Self {
        let schedule = Schedule::new(&ir);
        let verdict_factory = VerdictFactory::new(&ir);
        let num_inputs = ir.num_inputs();
        let num_outputs = ir.num_outputs();

        // Partially evaluate the StreamIR for the event-based fragment
        let event_based = optimize(
            ir.clone(),
            vec![Box::new(EvaluateGuards::only_event_based())],
        )
        .unwrap();

        // optimize event-based fragment based on argument
        let event_based = if optimized {
            optimize_all(event_based).unwrap()
        } else {
            // To be able to compare to the old interpreter, we always want fast guards
            optimize(event_based, vec![Box::new(FastGuards)]).unwrap()
        };

        let StreamIr {
            stmt,
            sr2memory,
            wref2window,
            lref2lfreq,
            ..
        } = event_based;

        // compile the event-based fragment using JIT compilation
        let event_clouregen = Closuregen::<Inputs>::new(
            sr2memory,
            wref2window,
            lref2lfreq,
            schedule.static_schedule.clone(),
        );
        let event_prog = event_clouregen.stmt(stmt);

        //  Partially evaluate the (original) StreamIR for the time-based fragment
        let time_based = optimize(ir, vec![Box::new(EvaluateGuards::only_time_based())]).unwrap();
        let time_based = if optimized {
            optimize_all(time_based).unwrap()
        } else {
            // To be able to compare to the old interpreter, we always want fast guards
            optimize(time_based, vec![Box::new(FastGuards)]).unwrap()
        };

        let StreamIr {
            stmt,
            sr2memory: timed_sr2memory,
            ..
        } = time_based;

        // The optimizations should not change the memory differently for timed- and event-based parts
        assert_eq!(event_clouregen.sr2memory, timed_sr2memory);
        // So we just reuse the old StreamIR information for compilating the time-based fragment
        let timed_closuregen = event_clouregen.with_event::<DeadlineEvent>();
        let timed_prog = timed_closuregen.stmt(stmt);
        // And construct the memory (for the time-based fragment, but the memory information is identical to the event-based fragment).
        let memory = timed_closuregen.memory();

        let fresh_inputs = BitSet::with_capacity(num_inputs);
        let fresh_outputs = BitSet::with_capacity(num_outputs);
        let spawned_streams = BitSet::with_capacity(num_outputs);
        let closing_streams = BitSet::with_capacity(num_outputs);
        let closing_unparam_local = BitSet::with_capacity(num_outputs);
        let closing_param_local = BitSet::with_capacity(num_outputs);
        let instances = (0..num_outputs)
            .map(|_| InstanceCollection::new())
            .collect();

        Self {
            memory,
            event_prog,
            timed_prog,
            fresh_inputs,
            fresh_outputs,
            spawned_streams,
            closing_streams,
            closing_unparam_local,
            closing_param_local,
            instances,
            schedule,
            verdict_factory,
        }
    }

    /// Update the monitor with a new event providing new values to input streams and returns the [Verdict].
    ///
    /// The given `ts` is used to indicate the timestamp of the inputs.
    /// The function updates all periodic streams that are due since the last update up until (but not including) that timestamp.
    pub fn accept_event(&mut self, ev: Inputs, ts: Time) -> Verdict {
        let timed_verdicts = self.accept_time::<false>(ts);
        let mut ctx = EvaluationContext::new(
            ts,
            &mut self.memory,
            &mut self.fresh_inputs,
            &mut self.fresh_outputs,
            &mut self.spawned_streams,
            &mut self.closing_streams,
            &mut self.closing_param_local,
            &mut self.closing_unparam_local,
            &mut self.instances,
            &mut self.schedule,
        );
        for (i, input) in ev.0.iter().enumerate() {
            if input.is_some() {
                ctx.fresh_inputs.insert(i);
            }
        }
        self.event_prog.execute(&ev, &mut ctx);
        let event_verdict = TotalIncremental::create(&ctx, &self.verdict_factory);
        let verdict = Verdict {
            timed: timed_verdicts,
            ts,
            event: event_verdict,
        };
        ctx.clear();
        verdict
    }

    /// Updates all periodic streams that are due since the last update up until that timestamp and returns a list of [TotalIncremental] verdicts with
    /// the corresponding timestamps of evaluation.
    ///
    /// With `INCLUSIVE` being true, that timestamp is included, otherwise not.
    pub fn accept_time<const INCLUSIVE: bool>(
        &mut self,
        ts: Time,
    ) -> Vec<(Time, TotalIncremental)> {
        let mut verdicts = Vec::new();

        while let Some(deadline) = self.schedule.next::<INCLUSIVE>(ts) {
            let mut ctx = EvaluationContext::new(
                deadline.ts,
                &mut self.memory,
                &mut self.fresh_inputs,
                &mut self.fresh_outputs,
                &mut self.spawned_streams,
                &mut self.closing_streams,
                &mut self.closing_param_local,
                &mut self.closing_unparam_local,
                &mut self.instances,
                &mut self.schedule,
            );
            self.timed_prog.execute(&deadline, &mut ctx);
            verdicts.push((
                deadline.ts,
                TotalIncremental::create(&ctx, &self.verdict_factory),
            ));
            ctx.clear();
        }
        verdicts
    }

    /// Update all periodic streams that are due until the end time of the monitor.
    pub fn finish(&mut self, ts: Time) -> Vec<(Time, TotalIncremental)> {
        self.accept_time::<true>(ts)
    }
}
