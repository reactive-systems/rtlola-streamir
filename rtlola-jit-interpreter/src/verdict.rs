//! The module contains the functionality to output the current monitoring state after each evaluation cycle

use std::fmt::{Display, Formatter};

use itertools::Itertools;
use streamir_lib::ir::{InputReference, OutputReference, StreamIr};

use crate::closuregen::{EvaluationContext, InstanceCollection};
use crate::value::Value;
use crate::Time;

/// The parameter of an parameterized stream's instance
pub type Parameters = Option<Vec<Value>>;

/// Represent the change that can happen to a stream's instance during an evaluation cycle
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Change {
    /// Indicates that a new instance of a stream was created with the given values as parameters.
    Spawn(Parameters),
    /// Indicates that an instance got a new value. The instance is identified through the given [Parameters].
    Value(Parameters, Value),
    /// Indicates that an instance was closed. The given values are the parameters of the closed instance.
    Close(Parameters),
}

impl Display for Change {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Spawn(para) => match para {
                Some(para) => write!(f, "Spawn<{}>", para.iter().join(", ")),
                None => write!(f, "Spawn"),
            },
            Change::Close(para) => match para {
                Some(para) => write!(f, "Close<{}>", para.iter().join(", ")),
                None => write!(f, "Close"),
            },
            Change::Value(para, value) => match para {
                Some(para) => write!(f, "Instance<{}> = {}", para.iter().join(", "), value),
                None => write!(f, "Value = {}", value),
            },
        }
    }
}

#[derive(Debug, Clone)]
/// Constructs new verdicts based on the current monitoring state
pub struct VerdictFactory {
    num_outputs: usize,
    static_outputs: Vec<usize>,
    dynamic_outputs: Vec<usize>,
    parameterized_outputs: Vec<usize>,
}

impl VerdictFactory {
    pub(crate) fn new(ir: &StreamIr) -> Self {
        Self {
            num_outputs: ir.num_outputs(),
            static_outputs: ir
                .static_outputs()
                .map(|sr| sr.unparameterized_idx())
                .collect(),
            dynamic_outputs: ir
                .dynamic_outputs()
                .map(|sr| sr.unparameterized_idx())
                .collect(),
            parameterized_outputs: ir
                .parameterized_outputs()
                .map(|sr| sr.parameterized_idx())
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
/// The verdict represents the changes to the streams during one evaluation cycle
pub struct TotalIncremental {
    /// The set of changed inputs with the new value
    pub inputs: Vec<(InputReference, Value)>,
    /// The set of changed outputs
    pub outputs: Vec<(OutputReference, Vec<Change>)>,
}

impl PartialEq for TotalIncremental {
    fn eq(&self, other: &Self) -> bool {
        self.sorted_inputs() == other.sorted_inputs()
            && self.sorted_outputs() == other.sorted_outputs()
    }
}

impl Eq for TotalIncremental {}

impl TotalIncremental {
    pub(crate) fn create(data: &EvaluationContext<'_>, factory: &VerdictFactory) -> Self {
        let inputs = data
            .fresh_inputs
            .iter()
            .map(|i| (i, data.memory.get_input_value(i, 0)))
            .collect();

        let mut outputs = Vec::with_capacity(factory.num_outputs);

        for &output_ref in &factory.static_outputs {
            if data.fresh_outputs.contains(output_ref) {
                outputs.push((
                    OutputReference::Unparameterized(output_ref),
                    vec![Change::Value(
                        None,
                        data.memory.get_output_value(output_ref, 0),
                    )],
                ));
            }
        }

        for &output_ref in &factory.dynamic_outputs {
            let mut res = Vec::with_capacity(data.fresh_outputs.len());
            if data.fresh_outputs.contains(output_ref) {
                res.push(Change::Value(
                    None,
                    data.memory.get_output_value(output_ref, 0),
                ));
            }
            if data.spawned_streams.contains(output_ref) {
                res.push(Change::Spawn(None));
            }
            if data.closing_streams.contains(output_ref) {
                res.push(Change::Close(None));
            }
            if !res.is_empty() {
                outputs.push((OutputReference::Unparameterized(output_ref), res))
            }
        }
        for &output_ref in &factory.parameterized_outputs {
            let InstanceCollection {
                spawned,
                eval,
                closed,
            } = &data.instances[output_ref];

            let mut res = Vec::with_capacity(eval.len() + 2);

            if let Some(spawned) = spawned {
                let spawned = (**spawned).clone();
                res.push(Change::Spawn(Some(spawned)))
            }
            for eval in eval {
                let v = data.memory.get_output_instance_value(output_ref, eval, 0);
                let instance = (**eval).clone();
                res.push(Change::Value(Some(instance), v));
            }
            for close in closed {
                let close = (**close).clone();
                res.push(Change::Close(Some(close)));
            }
            if !res.is_empty() {
                outputs.push((OutputReference::Parameterized(output_ref), res))
            }
        }
        TotalIncremental { inputs, outputs }
    }

    /// Returns the input vector with sorted inputreferences
    pub fn sorted_inputs(&self) -> Vec<(InputReference, &Value)> {
        self.inputs
            .iter()
            .sorted_by(|lhs, rhs| lhs.0.cmp(&rhs.0))
            .map(|(i, v)| (*i, v))
            .collect::<Vec<_>>()
    }

    /// Returns the output vector with sorted outputreferences
    pub fn sorted_outputs(&self) -> Vec<(OutputReference, Vec<&Change>)> {
        self.outputs
            .iter()
            .sorted_by(|lhs, rhs| lhs.0.cmp(&rhs.0))
            .map(|(i, v)| (*i, v.iter().sorted().collect::<Vec<_>>()))
            .collect::<Vec<_>>()
    }

    #[cfg(test)]
    fn only_value(self) -> Option<Self> {
        let TotalIncremental { inputs, outputs } = self;
        let res = TotalIncremental {
            inputs,
            outputs: outputs
                .into_iter()
                .flat_map(|(oref, v)| {
                    let v = v
                        .into_iter()
                        .filter(|v| matches!(v, Change::Value(_, _)))
                        .collect::<Vec<_>>();
                    if v.is_empty() {
                        None
                    } else {
                        Some((oref, v))
                    }
                })
                .collect(),
        };
        if res.inputs.is_empty() && res.outputs.is_empty() {
            None
        } else {
            Some(res)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents the result after handing an event to the monitor.
///
/// Contains a set of [TotalIncremental] verdicts for each deadline of periodic streams since the last event,
/// as well as the verdict of the current event.
pub struct Verdict {
    /// All the periodic deadlines with the corresponding verdicts
    pub timed: Vec<(Time, TotalIncremental)>,
    /// The timestamp of the new event
    pub ts: Time,
    /// The verdict of the new event
    pub event: TotalIncremental,
}

impl Verdict {
    #[cfg(test)]
    pub(crate) fn only_value(self) -> Self {
        let Verdict { timed, ts, event } = self;
        Verdict {
            timed: timed
                .into_iter()
                .flat_map(|(ts, v)| v.only_value().map(|v| (ts, v)))
                .collect(),
            ts,
            event: event.only_value().unwrap_or_else(|| TotalIncremental {
                inputs: Vec::new(),
                outputs: Vec::new(),
            }),
        }
    }
}
