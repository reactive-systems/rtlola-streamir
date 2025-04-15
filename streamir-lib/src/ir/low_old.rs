mod expressions;
pub(crate) mod livetime_equivalences;

use livetime_equivalences::LivetimeEquivalences;
use std::{collections::HashMap, iter, time::Duration};
use thiserror::Error;
use uom::num_traits::Inv;
use uom::si::{
    rational64::{Frequency as UOM_Frequency, Time},
    time::nanosecond,
};

use expressions::convert_stream_expression;
use itertools::Itertools;
use rtlola_frontend::mir::{self, Expression, ExpressionKind, PacingType, Stream};

use super::schedule::StaticSchedule;
use super::windows::{InstanceSelection, Window, WindowKind, WindowOperation};
use super::OutputReference;
use super::{
    expressions::Expr,
    memory::{Memory, Parameter, StreamBuffer, StreamMemory},
    Guard, IfStmt, Stmt, StreamIr, StreamReference, Type, WindowReference,
};
use super::{LocalFreq, LocalFreqRef};

impl TryFrom<rtlola_frontend::RtLolaMir> for StreamIr {
    type Error = LoweringError;

    fn try_from(value: rtlola_frontend::RtLolaMir) -> Result<Self, Self::Error> {
        let schedule = value
            .compute_schedule()
            .map_err(LoweringError::ComputeSchedule)?;
        let rtlola_frontend::RtLolaMir {
            inputs,
            outputs,
            time_driven: _,
            event_driven: _,
            discrete_windows,
            sliding_windows,
            instance_aggregations,
            triggers,
            global_tags: _,
        } = value;

        let mut cur_unparameterized = 0;
        let mut cur_parameterized = 0;
        let sr2sr: HashMap<_, _> = inputs
            .iter()
            .map(|i| (i.reference, StreamReference::In(i.reference.in_ix())))
            .chain(outputs.iter().map(|o| {
                (
                    o.reference,
                    StreamReference::Out(if o.is_parameterized() {
                        let i = cur_parameterized;
                        cur_parameterized += 1;
                        OutputReference::Parameterized(i)
                    } else {
                        let i = cur_unparameterized;
                        cur_unparameterized += 1;
                        OutputReference::Unparameterized(i)
                    }),
                )
            }))
            .collect();
        let mut lref2lfreq: HashMap<LocalFreqRef, LocalFreq> = HashMap::new();

        let livetime_equivalences = LivetimeEquivalences::new(&outputs, &sr2sr);
        let static_schedule = StaticSchedule::new(schedule, &sr2sr);

        let (sr2memory_inputs, input_stmts): (HashMap<_, _>, Vec<_>) = inputs
            .into_iter()
            .map(|i| {
                let sr = sr2sr[&i.reference];
                let (mem, stmts) = StreamIr::lower_input(i, &sr2sr);
                ((sr, mem), stmts)
            })
            .unzip();
        let layer_0 = Stmt::parallel(input_stmts);
        let (sr2memory_outputs, stmts): (HashMap<StreamReference, Memory>, Vec<_>) = outputs
            .into_iter()
            .map(|o| {
                let sr = sr2sr[&o.reference];
                let (mem, stmts) = StreamIr::lower_output(o, &sr2sr, &mut lref2lfreq)?;
                Ok(((sr, mem), stmts))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .unzip();

        let stmts = stmts
            .into_iter()
            .flatten()
            .sorted_by_key(|(layer, _stmt)| *layer);
        let layers = Stmt::seq(
            iter::once(layer_0).chain(stmts.chunk_by(|(layer, _stmt)| *layer).into_iter().map(
                |(_layer, stmts)| Stmt::parallel(stmts.into_iter().map(|(_layer, stmt)| stmt)),
            )),
        );

        let sr2memory = sr2memory_inputs
            .into_iter()
            .chain(sr2memory_outputs)
            .collect();

        let wref2window = sliding_windows
            .into_iter()
            .map(|swin| {
                Ok((
                    swin.reference.into(),
                    StreamIr::lower_sliding_window(swin, &sr2sr, &mut lref2lfreq)?,
                ))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .chain(
                discrete_windows
                    .into_iter()
                    .map(|dwin| {
                        Ok((
                            dwin.reference.into(),
                            StreamIr::lower_discrete_window(dwin, &sr2sr, &mut lref2lfreq)?,
                        ))
                    })
                    .collect::<Vec<_>>(),
            )
            .chain(
                instance_aggregations
                    .into_iter()
                    .map(|iwin| {
                        Ok((
                            iwin.reference.into(),
                            StreamIr::lower_instance_aggregation(iwin, &sr2sr, &mut lref2lfreq)?,
                        ))
                    })
                    .collect::<Vec<_>>(),
            )
            .collect::<Result<_, _>>()?;

        let triggers = triggers
            .into_iter()
            .map(|t| sr2sr[&t.output_reference].out_idx())
            .collect();

        Ok(StreamIr {
            stmt: layers,
            sr2memory,
            wref2window,
            lref2lfreq,
            livetime_equivalences,
            static_schedule,
            triggers,
        })
    }
}

impl From<mir::MemorizationBound> for StreamBuffer {
    fn from(value: mir::MemorizationBound) -> Self {
        match value {
            mir::MemorizationBound::Unbounded => StreamBuffer::UnBounded,
            mir::MemorizationBound::Bounded(b) => StreamBuffer::Bounded(b as usize),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MyLayer {
    Layer(mir::Layer),
    Close,
}

impl From<mir::Layer> for MyLayer {
    fn from(value: mir::Layer) -> Self {
        MyLayer::Layer(value)
    }
}

impl From<mir::Parameter> for Parameter {
    fn from(value: mir::Parameter) -> Self {
        let mir::Parameter { name, ty, idx: _ } = value;
        Parameter {
            name,
            ty: ty.into(),
        }
    }
}

impl StreamIr {
    fn lower_input(
        input: mir::InputStream,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> (Memory, Stmt) {
        let sr = sr2sr[&input.reference];
        let memory = Memory {
            buffer: StreamMemory::Static(input.memory_bound.into()),
            ty: input.ty.into(),
            name: input.name,
        };
        let stmt = Stmt::seq([Stmt::Shift(sr), Stmt::Input(sr.in_idx())]).filter(Guard::Stream(sr));
        (memory, stmt)
    }

    fn lower_output(
        output: mir::OutputStream,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<(Memory, [(MyLayer, Stmt); 4]), LoweringError> {
        let mir::OutputStream {
            name,
            kind: _,
            ty,
            spawn,
            eval,
            close,
            accesses: _,
            accessed_by: _,
            aggregated_by: _,
            aggregates,
            memory_bound,
            layer,
            reference,
            params,
            tags: _,
        } = output;
        let sr = sr2sr[&reference];
        let parameter = params.into_iter().map(|p| p.into()).collect::<Vec<_>>();
        let has_spawn = spawn.condition.is_some(); //???
        let has_close = close.condition.is_some(); //???
        let spawned_and_closed_windows = aggregates
            .iter()
            .filter_map(|(_target, origin, wref)| match origin {
                mir::Origin::Spawn => None,
                mir::Origin::Filter(_) | mir::Origin::Eval(_) | mir::Origin::Close => {
                    Some((*wref).into())
                }
            })
            .collect::<Vec<_>>();
        let (shift, eval, mut eval_freq) = StreamIr::lower_eval(sr, eval, sr2sr, lref2lfreq)?;
        let shift = shift.iterate(sr, &parameter);
        let eval = eval.iterate(sr, &parameter);
        let (close, close_freq) = StreamIr::lower_close(
            sr,
            close,
            sr2sr,
            lref2lfreq,
            eval_freq.clone(),
            spawned_and_closed_windows.clone(),
        )?;
        let close = close.iterate(sr, &parameter);
        eval_freq.extend(close_freq);
        let spawn = StreamIr::lower_spawn(
            sr,
            spawn,
            sr2sr,
            lref2lfreq,
            eval_freq,
            spawned_and_closed_windows,
        )?;
        let mem = Memory {
            buffer: parameter
                .is_empty()
                .then(|| StreamMemory::Dynamic {
                    buffer: memory_bound.into(),
                    has_spawn,
                    has_close,
                })
                .unwrap_or_else(|| StreamMemory::Instances {
                    buffer: memory_bound.into(),
                    parameter,
                }),
            ty: ty.into(),
            name,
        };
        Ok((
            mem,
            [
                (layer.spawn_layer().into(), spawn),
                (layer.shift_layer().into(), shift),
                (layer.evaluation_layer().into(), eval),
                (MyLayer::Close, close),
            ],
        ))
    }

    fn lower_spawn(
        sr: StreamReference,
        spawn: mir::Spawn,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
        local_freqs: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Result<Stmt, LoweringError> {
        let mir::Spawn {
            expression,
            pacing,
            condition,
        } = spawn;
        let (guard, lref) = Self::lower_guard(pacing, condition, sr, sr2sr, lref2lfreq)?;
        if lref.is_some() {
            Err(LoweringError::LocalFreq)
        } else {
            Ok(Stmt::Spawn {
                sr: sr.out_idx(),
                with: expression
                    .map(|Expression { ty, kind }| match kind {
                        ExpressionKind::Tuple(inner) => inner
                            .into_iter()
                            .map(|expr| convert_stream_expression(expr, None, sr2sr))
                            .collect(),
                        other => Ok(vec![convert_stream_expression(
                            Expression { ty, kind: other },
                            None,
                            sr2sr,
                        )?]),
                    })
                    .transpose()?,
                local_frequencies: local_freqs,
                windows,
            }
            .filter(guard))
        }
    }

    fn lower_eval(
        sr: StreamReference,
        eval: mir::Eval,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<(Stmt, Stmt, Vec<LocalFreqRef>), LoweringError> {
        let mir::Eval {
            clauses,
            eval_pacing: _,
        } = eval;
        let shift = |_expr: Expression| Ok(Stmt::Shift(sr));
        let eval = |expr: Expression| {
            Ok::<Stmt, LoweringError>(Stmt::Eval {
                sr: sr.out_idx(),
                with: convert_stream_expression(expr, None, sr2sr)?,
            })
        };
        let mut local_freqs = Vec::new();
        let mut construct = |f: Box<dyn Fn(Expression) -> Result<Stmt, LoweringError>>| {
            let mut clauses = clauses.clone();
            let last = clauses
                .pop()
                .map(
                    |mir::EvalClause {
                         condition,
                         expression,
                         pacing,
                     }| {
                        let (guard, lref) =
                            Self::lower_guard(pacing, condition, sr, sr2sr, lref2lfreq)?;
                        local_freqs.extend(lref);
                        Ok(f(expression)?.filter(guard))
                    },
                )
                .unwrap();
            clauses.into_iter().rfold(
                last,
                |alt,
                 mir::EvalClause {
                     condition,
                     expression,
                     pacing,
                 }| {
                    let (guard, lref) =
                        Self::lower_guard(pacing, condition, sr, sr2sr, lref2lfreq)?;
                    local_freqs.extend(lref);
                    Ok(f(expression)?.filter_else(guard, alt?))
                },
            )
        };
        Ok((
            construct(Box::new(shift))?,
            construct(Box::new(eval))?,
            local_freqs,
        ))
    }

    fn lower_close(
        sr: StreamReference,
        close: mir::Close,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
        mut eval_local_freqs: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Result<(Stmt, Option<LocalFreqRef>), LoweringError> {
        let mir::Close {
            condition, pacing, ..
        } = close;
        let (condition, lfreq) = condition
            .map(|g| Self::lower_guard(pacing, Some(g), sr, sr2sr, lref2lfreq))
            .unwrap_or_else(|| Ok((Guard::Constant(false), None)))?;
        eval_local_freqs.extend(lfreq);
        Ok((
            Stmt::Close {
                sr: sr.out_idx(),
                local_frequencies: eval_local_freqs,
                windows,
            }
            .filter(condition),
            lfreq,
        ))
    }

    fn lower_guard(
        pacing: PacingType,
        condition: Option<Expression>,
        source: StreamReference,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref1lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<(Guard, Option<LocalFreqRef>), LoweringError> {
        let (pacing, lref) = Guard::from_pt(pacing, source, sr2sr, lref1lfreq);
        if let Some(condition) = condition {
            Ok((
                pacing.and(convert_stream_expression(condition, None, sr2sr)?.into()),
                lref,
            ))
        } else {
            Ok((pacing, lref))
        }
    }

    fn lower_sliding_window(
        sliding_window: mir::SlidingWindow,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<Window, LoweringError> {
        let mir::SlidingWindow {
            target,
            caller,
            duration,
            num_buckets,
            bucket_size,
            wait,
            op,
            reference,
            ty,
            origin,
            pacing,
        } = sliding_window;
        Ok(Window {
            wref: reference.into(),
            op: op.into(),
            target: sr2sr[&target],
            caller: sr2sr[&caller],
            ty: ty.into(),
            kind: WindowKind::Sliding {
                duration,
                bucket_count: num_buckets.unwrap() as usize,
                wait,
                bucket_duration: bucket_size,
            },
            origin_pacing: Guard::from_pt(pacing, sr2sr[&caller], sr2sr, lref2lfreq).0,
            origin: origin.into(),
        })
    }

    fn lower_discrete_window(
        discrete_window: mir::DiscreteWindow,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<Window, LoweringError> {
        let mir::DiscreteWindow {
            target,
            caller,
            duration,
            wait,
            op,
            reference,
            ty,
            origin,
            pacing,
        } = discrete_window;
        Ok(Window {
            wref: reference.into(),
            op: op.into(),
            target: sr2sr[&target],
            caller: sr2sr[&caller],
            ty: ty.into(),
            kind: WindowKind::Discrete {
                num_values: duration,
                wait,
            },
            origin_pacing: Guard::from_pt(pacing, sr2sr[&caller], sr2sr, lref2lfreq).0,
            origin: origin.into(),
        })
    }

    fn lower_instance_aggregation(
        instance_aggregation: mir::InstanceAggregation,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> Result<Window, LoweringError> {
        let mir::InstanceAggregation {
            target,
            caller,
            selection,
            aggr,
            reference,
            ty,
            origin,
            pacing,
        } = instance_aggregation;
        Ok(Window {
            wref: reference.into(),
            op: aggr.into(),
            target: sr2sr[&target],
            caller: sr2sr[&caller],
            ty: ty.into(),
            kind: WindowKind::Instances {
                selection: InstanceSelection::from_mir_selection(selection, sr2sr)?,
            },
            origin_pacing: Guard::from_pt(pacing, sr2sr[&caller], sr2sr, lref2lfreq).0,
            origin: origin.into(),
        })
    }
}

impl Stmt {
    fn filter(self, guard: Guard) -> Self {
        Stmt::If(IfStmt {
            guard,
            cons: Box::new(self),
            alt: Box::new(Stmt::Skip),
        })
    }

    fn filter_else(self, guard: Guard, alt: Stmt) -> Self {
        Stmt::If(IfStmt {
            guard,
            cons: Box::new(self),
            alt: Box::new(alt),
        })
    }

    pub(crate) fn seq(iter: impl IntoIterator<Item = Stmt>) -> Self {
        let stmts = iter.into_iter().collect::<Vec<_>>();
        match stmts.len() {
            0 => Stmt::Skip,
            1 => stmts.into_iter().next().unwrap(),
            2.. => Stmt::Seq(stmts),
        }
    }

    pub(crate) fn parallel(iter: impl IntoIterator<Item = Stmt>) -> Self {
        let stmts = iter.into_iter().collect::<Vec<_>>();
        match stmts.len() {
            0 => Stmt::Skip,
            1 => stmts.into_iter().next().unwrap(),
            2.. => Stmt::Parallel(stmts),
        }
    }

    fn iterate(self, sr: StreamReference, parameter: &[Parameter]) -> Self {
        if parameter.is_empty() {
            self.filter(Guard::Alive(sr))
        } else {
            Stmt::Iterate {
                sr: vec![sr.out_idx()],
                stmt: Box::new(self),
            }
        }
    }
}

impl Guard {
    fn and(self, rhs: Self) -> Self {
        Guard::And {
            lhs: Box::new(self),
            rhs: Box::new(rhs),
        }
    }
}

impl From<Expr> for Guard {
    fn from(value: Expr) -> Self {
        Guard::Dynamic(value)
    }
}

impl Guard {
    fn from_pt(
        pt: PacingType,
        sr: StreamReference,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
        lref2lfreq: &mut HashMap<LocalFreqRef, LocalFreq>,
    ) -> (Self, Option<LocalFreqRef>) {
        match pt {
            PacingType::GlobalPeriodic(freq) => {
                (Guard::GlobalFreq(frequency_to_duration(freq)), None)
            }
            PacingType::LocalPeriodic(freq) => {
                let new_ref = lref2lfreq.len();
                let freq = LocalFreq {
                    dur: frequency_to_duration(freq),
                    sr: sr.out_idx(),
                    reference: new_ref,
                };
                if let Some(r) = lref2lfreq
                    .iter()
                    .find_map(|(k, v)| (*v == freq).then_some(*k))
                {
                    (Guard::LocalFreq(r), Some(r))
                } else {
                    lref2lfreq.insert(new_ref, freq);
                    (Guard::LocalFreq(new_ref), Some(new_ref))
                }
            }
            PacingType::Event(activation_condition) => {
                (Guard::from_ac(activation_condition, sr2sr), None)
            }
            PacingType::Constant => (Guard::Constant(true), None),
        }
    }
}

fn frequency_to_duration(frequency: UOM_Frequency) -> Duration {
    let period =
        Time::new::<uom::si::time::second>(frequency.get::<uom::si::frequency::hertz>().inv());
    Duration::from_nanos(
        period
            .get::<nanosecond>()
            .to_integer()
            .try_into()
            .expect("Period [ns] too large for u64!"),
    )
}

impl Guard {
    fn from_ac(
        value: mir::ActivationCondition,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Self {
        match value {
            mir::ActivationCondition::Conjunction(activation_conditions) => activation_conditions
                .into_iter()
                .map(|g| Guard::from_ac(g, sr2sr))
                .reduce(|lhs, rhs| Guard::And {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                })
                .unwrap(),
            mir::ActivationCondition::Disjunction(activation_conditions) => activation_conditions
                .into_iter()
                .map(|g| Guard::from_ac(g, sr2sr))
                .reduce(|lhs, rhs| Guard::Or {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                })
                .unwrap(),
            mir::ActivationCondition::Stream(stream_reference) => {
                Guard::Stream(sr2sr[&stream_reference])
            }
            mir::ActivationCondition::True => Guard::Constant(true),
        }
    }
}

impl From<mir::WindowReference> for WindowReference {
    fn from(value: mir::WindowReference) -> Self {
        match value {
            mir::WindowReference::Sliding(i) => WindowReference::Sliding(i),
            mir::WindowReference::Discrete(i) => WindowReference::Discrete(i),
            mir::WindowReference::Instance(i) => WindowReference::Instance(i),
        }
    }
}

impl From<mir::Type> for Type {
    fn from(value: mir::Type) -> Self {
        match value {
            mir::Type::Bool => Type::Bool,
            mir::Type::Int(mir::IntTy::Int8) => Type::Int(8),
            mir::Type::Int(mir::IntTy::Int16) => Type::Int(16),
            mir::Type::Int(mir::IntTy::Int32) => Type::Int(32),
            mir::Type::Int(mir::IntTy::Int64) => Type::Int(64),
            mir::Type::UInt(mir::UIntTy::UInt8) => Type::UInt(8),
            mir::Type::UInt(mir::UIntTy::UInt16) => Type::UInt(16),
            mir::Type::UInt(mir::UIntTy::UInt32) => Type::UInt(32),
            mir::Type::UInt(mir::UIntTy::UInt64) => Type::UInt(64),
            mir::Type::Float(mir::FloatTy::Float32) => Type::Float32,
            mir::Type::Float(mir::FloatTy::Float64) => Type::Float64,
            mir::Type::String => Type::String,
            mir::Type::Option(inner_ty) => Type::Option(Box::new(Type::from(*inner_ty))),
            mir::Type::Tuple(inner_tys) => {
                Type::Tuple(inner_tys.into_iter().map(Type::from).collect())
            }
            mir::Type::Fixed(mir::FixedTy::Fixed16_8) => Type::Fixed(16),
            mir::Type::Fixed(mir::FixedTy::Fixed32_16) => Type::Fixed(32),
            mir::Type::Fixed(mir::FixedTy::Fixed64_32) => Type::Fixed(64),
            mir::Type::UFixed(mir::FixedTy::Fixed16_8) => Type::UFixed(16),
            mir::Type::UFixed(mir::FixedTy::Fixed32_16) => Type::UFixed(32),
            mir::Type::UFixed(mir::FixedTy::Fixed64_32) => Type::UFixed(64),
            mir::Type::Bytes => Type::Bytes,
            mir::Type::Function { .. } => unimplemented!(),
        }
    }
}

impl From<mir::WindowOperation> for WindowOperation {
    fn from(value: mir::WindowOperation) -> Self {
        match value {
            mir::WindowOperation::Count => WindowOperation::Count,
            mir::WindowOperation::Min => WindowOperation::Min,
            mir::WindowOperation::Max => WindowOperation::Max,
            mir::WindowOperation::Sum => WindowOperation::Sum,
            mir::WindowOperation::Product => WindowOperation::Product,
            mir::WindowOperation::Average => WindowOperation::Average,
            mir::WindowOperation::Integral => WindowOperation::Integral,
            mir::WindowOperation::Conjunction => WindowOperation::Conjunction,
            mir::WindowOperation::Disjunction => WindowOperation::Disjunction,
            mir::WindowOperation::Last => WindowOperation::Last,
            mir::WindowOperation::Variance => WindowOperation::Variance,
            mir::WindowOperation::Covariance => WindowOperation::Covariance,
            mir::WindowOperation::StandardDeviation => WindowOperation::StandardDeviation,
            mir::WindowOperation::NthPercentile(n) => WindowOperation::NthPercentile(n),
        }
    }
}

impl From<mir::InstanceOperation> for WindowOperation {
    fn from(value: mir::InstanceOperation) -> Self {
        match value {
            mir::InstanceOperation::Count => WindowOperation::Count,
            mir::InstanceOperation::Min => WindowOperation::Min,
            mir::InstanceOperation::Max => WindowOperation::Max,
            mir::InstanceOperation::Sum => WindowOperation::Sum,
            mir::InstanceOperation::Product => WindowOperation::Product,
            mir::InstanceOperation::Average => WindowOperation::Average,
            mir::InstanceOperation::Conjunction => WindowOperation::Conjunction,
            mir::InstanceOperation::Disjunction => WindowOperation::Disjunction,
            mir::InstanceOperation::Variance => WindowOperation::Variance,
            mir::InstanceOperation::Covariance => WindowOperation::Covariance,
            mir::InstanceOperation::StandardDeviation => WindowOperation::StandardDeviation,
            mir::InstanceOperation::NthPercentile(n) => WindowOperation::NthPercentile(n),
        }
    }
}

impl InstanceSelection {
    fn from_mir_selection(
        value: mir::InstanceSelection,
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Result<Self, LoweringError> {
        match value {
            mir::InstanceSelection::Fresh => Ok(InstanceSelection::Fresh),
            mir::InstanceSelection::All => Ok(InstanceSelection::All),
            mir::InstanceSelection::FilteredFresh { parameters, cond } => {
                Ok(InstanceSelection::FilteredFresh {
                    parameters: parameters.into_iter().map(|p| p.into()).collect(),
                    cond: convert_stream_expression(*cond, None, sr2sr)?,
                })
            }
            mir::InstanceSelection::FilteredAll { parameters, cond } => {
                Ok(InstanceSelection::FilteredAll {
                    parameters: parameters.into_iter().map(|p| p.into()).collect(),
                    cond: convert_stream_expression(*cond, None, sr2sr)?,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Error)]
/// An error that can happen during the lowering of the RtLolaMIR into the StreamIR
pub enum LoweringError {
    #[error("specification contains optional expression that is not immediately followed by a default value")]
    /// The specification contains an optional expression that is not immediately followed by a default value
    DefaultRequired,
    #[error("specification contains a future access")]
    /// The specification contains a future access
    FutureAccess,
    #[error("specification contains the unsupported function {0}")]
    /// The specification contains an unsupported function
    UnsupportedFunction(String),
    #[error("Local frequency in an invalid position")]
    /// The specification contains a local frequency in an invalid position
    LocalFreq,
    #[error("Error computing static schedule: {0}")]
    /// An error happened when computing the static schedule from the RtLolaMir
    ComputeSchedule(String),
}
