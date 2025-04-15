//! The internal representation of the StreamIR.

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    time::Duration,
};

use expressions::Expr;
pub use lowering::livetime_equivalences::LivetimeEquivalences;
use memory::{Memory, StreamMemory};
use rtlola_frontend::mir::{self};
use windows::Window;
mod display;
pub mod expressions;
mod lowering;
pub mod memory;
#[cfg(test)]
pub(crate) mod parse;
mod print;
mod schedule;
pub mod windows;
pub use lowering::LoweringError;
pub use print::DebugFormatter;
pub use schedule::{Deadline, StaticSchedule, Task};

#[derive(Debug, Clone)]
/// The internal representation of the StreamIR.
pub struct StreamIr {
    /// The outermost statement of the StreamIR program.
    pub stmt: Stmt,
    /// The memory representation of each stream.
    pub sr2memory: HashMap<StreamReference, Memory>,
    /// Information on windows in the specification.
    pub wref2window: HashMap<WindowReference, Window>,
    /// A mapping from references to information of local frequencies.
    pub lref2lfreq: HashMap<LocalFreqRef, LocalFreq>,
    /// Equivalence classes for the livetime information of streams.
    pub livetime_equivalences: LivetimeEquivalences,
    /// The precomputed schedule for global periodic output streams
    pub static_schedule: Option<StaticSchedule>,
    /// The mapping of all output references that represent triggers to the corresponding trigger reference
    pub triggers: HashMap<OutputReference, usize>,
    /// The collection of streams this stream accesses non-transitively.  Includes this stream's spawn, evaluation condition, and close expressions.
    pub accesses: HashMap<StreamReference, Accesses>,
    /// The collection of streams that access the current stream non-transitively
    pub accessed_by: HashMap<StreamReference, Accesses>,
}

impl StreamIr {
    /// Returns the memory representation for the given stream.
    pub fn stream_memory(&self, sr: StreamReference) -> &Memory {
        &self.sr2memory[&sr]
    }

    /// Returns the name of the given stream.
    pub fn name(&self, sr: StreamReference) -> &str {
        &self.stream_memory(sr).name
    }

    /// Returns the stream reference of the stream with the given name
    pub fn stream_by_name(&self, name: &str) -> Option<StreamReference> {
        self.sr2memory
            .iter()
            .find(|(_, m)| m.name == name)
            .map(|(sr, _)| sr)
            .copied()
    }

    /// Returns an iterator over all streams in the specification UNSORTED
    pub fn streams(&self) -> impl Iterator<Item = StreamReference> + '_ {
        self.sr2memory.keys().copied()
    }

    /// Returns an iterator over all inputs in the specification UNSORTED
    pub fn inputs(&self) -> impl Iterator<Item = InputReference> + '_ {
        self.sr2memory
            .keys()
            .filter(|sr| matches!(sr, StreamReference::In(_)))
            .map(|sr| sr.in_idx())
    }

    /// Returns the number of input streams in the specification
    pub fn num_inputs(&self) -> usize {
        self.inputs().count()
    }

    /// Returns an iterator over all output streams in the specification UNSORTED
    pub fn outputs(&self) -> impl Iterator<Item = OutputReference> + '_ {
        self.sr2memory
            .keys()
            .filter(|sr| matches!(sr, StreamReference::Out(_)))
            .map(|sr| sr.out_idx())
    }

    /// Returns an iterator over all the output streams that represent triggers UNSORTED
    pub fn triggers(&self) -> impl Iterator<Item = OutputReference> + '_ {
        self.outputs().filter(|o| self.triggers.contains_key(o))
    }

    /// Returns the total number of output streams in the specification
    pub fn num_outputs(&self) -> usize {
        self.outputs().count()
    }

    /// Returns an iterator over all static output streams in the specification
    pub fn static_outputs(&self) -> impl Iterator<Item = OutputReference> + '_ {
        self.outputs().filter(|sr| {
            matches!(
                self.stream_memory(sr.sr()).buffer,
                StreamMemory::Static(_) | StreamMemory::NoMemory
            )
        })
    }

    /// Returns an iterator over of dynamic output streams in the specification
    pub fn dynamic_outputs(&self) -> impl Iterator<Item = OutputReference> + '_ {
        self.outputs().filter(|sr| {
            matches!(
                self.stream_memory(sr.sr()).buffer,
                StreamMemory::Dynamic { .. }
            )
        })
    }

    /// Returns the number of parameterized output streams in the specification
    pub fn parameterized_outputs(&self) -> impl Iterator<Item = OutputReference> + '_ {
        self.outputs().filter(|sr| {
            matches!(
                self.stream_memory(sr.sr()).buffer,
                StreamMemory::Instances { .. }
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A statement of the StreamIR.
pub enum Stmt {
    /// A no-op operation
    Skip,
    /// A sequence of statements
    Seq(Vec<Stmt>),
    /// A set of statements that can be executed in parallel
    Parallel(Vec<Stmt>),
    /// A statement shifting a stream
    Shift(StreamReference),
    /// A statement updating the memory of an input stream with a new value
    Input(InputReference),
    /// A statement spawning a new instance of an output stream
    Spawn {
        /// The reference of the output stream
        sr: OutputReference,
        /// The condition to calculate the new value (if None, the stream is not parameterized)
        with: Option<Vec<Expr>>,
        /// A list of (local) clocks that need to start with the spawn of the instance
        local_frequencies: Vec<LocalFreqRef>,
        /// A list of windows that need to start with the spawn of the instance
        windows: Vec<WindowReference>,
    },
    /// A statement computing the new value of an output stream and writing the value to memory
    Eval {
        /// The reference of the output stream
        sr: OutputReference,
        /// The stream expression of the eval-with clause
        with: Expr,
        /// The index of the eval clause this statment originated from
        idx: usize,
    },
    /// A statement closing an instance of an output stream
    /// To close a specific instance, the statement has to be inside an iterate/assign statement
    Close {
        /// The reference of the output stream
        sr: OutputReference,
        /// A list of (local) clocks that need to stop with the close of the instance
        local_frequencies: Vec<LocalFreqRef>,
        /// A list of windows that need to be closed
        windows: Vec<WindowReference>,
    },
    /// A conditional statement
    If(IfStmt),
    /// A statement iterating over all currently alive instances of an output stream
    Iterate {
        /// The references of the output streams
        sr: Vec<OutputReference>,
        /// A statement that is executed for every instance
        stmt: Box<Stmt>,
    },
    /// A statement assigning the value for the current instance by a stream expression
    Assign {
        /// A list of expressions calculating the parameters
        parameter_expr: Vec<Expr>,
        /// The streams this assign statement originated from
        sr: Vec<OutputReference>,
        /// The inner statement that is executed once with the calculated instance
        stmt: Box<Stmt>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// An conditional statement in the StreamIR.
pub struct IfStmt {
    /// The condition
    pub(crate) guard: Guard,
    /// The consequence statement
    pub(crate) cons: Box<Stmt>,
    /// The alternative statement (is often Stmt::Skip)
    pub(crate) alt: Box<Stmt>,
}

impl IfStmt {
    /// Return the guard of the conditional
    pub fn guard(&self) -> &Guard {
        &self.guard
    }

    /// Return the consequence of the conditional
    pub fn cons(&self) -> &Stmt {
        &self.cons
    }

    /// Return the alternative of the conditional (or None if the alternative is Stmt::Skip)
    pub fn alt(&self) -> Option<&Stmt> {
        (!matches!(*self.alt, Stmt::Skip)).then_some(self.alt.as_ref())
    }

    /// Destructure the conditional into the guard, the consequence and (if not Skip) the alternative.
    pub fn destruct(self) -> (Guard, Stmt, Option<Stmt>) {
        let IfStmt { guard, cons, alt } = self;
        (guard, *cons, (!matches!(*alt, Stmt::Skip)).then_some(*alt))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The condition of an conditional statement
pub enum Guard {
    /// Is true when the given stream received a new value in the current evaluation cycle
    Stream(StreamReference),
    /// Is true when the given stream is currently alive
    Alive(StreamReference),
    /// Is true when the given stream expression evaluates to true under the current monitoring state
    Dynamic(Expr),
    /// Is true if the given global frequency is currently due
    GlobalFreq(Duration),
    /// Is true if the given local frequency is currently due
    LocalFreq(LocalFreqRef),
    /// The conjunction of guards
    And {
        /// the left hand side of the conjunction
        lhs: Box<Guard>,
        /// the right hand side of the conjunction
        rhs: Box<Guard>,
    },
    /// The disjunction of guards
    Or {
        /// the left hand side of the disjunction
        lhs: Box<Guard>,
        /// the right hand side of the disjunction
        rhs: Box<Guard>,
    },
    /// A guard constantly evaluating to true/false
    Constant(bool),
    /// Shortcuts for conjunction of Guard::Stream's
    FastAnd(Vec<StreamReference>),
    /// Shortcuts for disjunction of Guard::Stream's
    FastOr(Vec<StreamReference>),
}

impl Guard {
    pub(crate) fn eq_liveness(
        &self,
        other: &Self,
        livetime_equivalences: &LivetimeEquivalences,
    ) -> bool {
        match (self, other) {
            (Self::Stream(l0), Self::Stream(r0)) => l0 == r0,
            (Self::Alive(l0), Self::Alive(r0)) => livetime_equivalences.is_equivalent(*l0, *r0),
            (Self::Dynamic(l0), Self::Dynamic(r0)) => l0 == r0,
            (Self::GlobalFreq(l0), Self::GlobalFreq(r0)) => l0 == r0,
            (Self::LocalFreq(l0), Self::LocalFreq(r0)) => l0 == r0,
            (
                Self::And {
                    lhs: l_lhs,
                    rhs: l_rhs,
                },
                Self::And {
                    lhs: r_lhs,
                    rhs: r_rhs,
                },
            ) => {
                l_lhs.eq_liveness(r_lhs, livetime_equivalences)
                    && l_rhs.eq_liveness(r_rhs, livetime_equivalences)
            }
            (
                Self::Or {
                    lhs: l_lhs,
                    rhs: l_rhs,
                },
                Self::Or {
                    lhs: r_lhs,
                    rhs: r_rhs,
                },
            ) => {
                l_lhs.eq_liveness(r_lhs, livetime_equivalences)
                    && l_rhs.eq_liveness(r_rhs, livetime_equivalences)
            }
            (Self::Constant(l0), Self::Constant(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Hash for Guard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Guard::Stream(sr) => {
                state.write_u8(1);
                sr.hash(state);
            }
            Guard::Alive(sr) => {
                state.write_u8(2);
                sr.hash(state);
            }
            Guard::Dynamic(_) => {
                state.write_u8(3);
            }
            Guard::GlobalFreq(duration) => {
                state.write_u8(4);
                duration.hash(state);
            }
            Guard::LocalFreq(f) => {
                state.write_u8(5);
                f.hash(state);
            }
            Guard::And { lhs, rhs } => {
                state.write_u8(6);
                lhs.hash(state);
                rhs.hash(state);
            }
            Guard::Or { lhs, rhs } => {
                state.write_u8(7);
                lhs.hash(state);
                rhs.hash(state);
            }
            Guard::Constant(c) => {
                state.write_u8(8);
                c.hash(state);
            }
            Guard::FastAnd(sr) => {
                state.write_u8(9);
                sr.hash(state);
            }
            Guard::FastOr(sr) => {
                state.write_u8(10);
                sr.hash(state);
            }
        }
    }
}

/// A reference to a local frequency
pub type LocalFreqRef = usize;

#[derive(Debug, Clone, Copy)]
/// The information of a local frequency
pub struct LocalFreq {
    /// The frequency
    pub dur: Duration,
    /// The stream this frequency belongs to
    pub sr: OutputReference,
    /// The reference of the frequency
    pub reference: LocalFreqRef,
}

impl PartialEq for LocalFreq {
    fn eq(&self, other: &Self) -> bool {
        self.dur == other.dur && self.sr == other.sr
    }
}

impl Eq for LocalFreq {}

/// Allows for referencing an input stream within the specification.
pub type InputReference = usize;

/// Allows for referencing an output stream within the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum OutputReference {
    /// Un unparameterized stream
    Unparameterized(usize),
    /// A parameterized stream
    Parameterized(usize),
}

impl OutputReference {
    /// The index of an unparameterized stream (panics if is is parameterized)
    pub fn unparameterized_idx(self) -> usize {
        match self {
            OutputReference::Parameterized(_) => unreachable!(),
            OutputReference::Unparameterized(i) => i,
        }
    }

    /// The index of an parameterized stream (panics if is is unparameterized)
    pub fn parameterized_idx(self) -> usize {
        match self {
            OutputReference::Unparameterized(_) => unreachable!(),
            OutputReference::Parameterized(i) => i,
        }
    }

    /// Returns the stream reference to that output reference
    pub fn sr(self) -> StreamReference {
        StreamReference::Out(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
/// A reference of an input or output stream
pub enum StreamReference {
    /// The reference is for an input stream
    In(InputReference),
    /// The reference is for an output stream
    Out(OutputReference),
}

impl StreamReference {
    /// Returns the [InputReference] from a Streamreference
    pub fn in_idx(self) -> InputReference {
        match self {
            StreamReference::In(i) => i,
            StreamReference::Out(_) => unreachable!("Called in_idx on an Outputstream"),
        }
    }

    /// Returns the [OutputReference] from a Streamreference
    pub fn out_idx(self) -> OutputReference {
        match self {
            StreamReference::In(_) => unreachable!("Called out_idx on an Inputstream"),
            StreamReference::Out(o) => o,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// The origin of a stream expression (the clauses of an output stream)
pub enum Origin {
    /// The expression was in the spawn clause
    Spawn,
    /// The expression was in the eval when clause
    EvalWhen(usize),
    /// The expression was in the eval with clause
    EvalWith(usize),
    /// The expression was in the close condition
    Close,
}

impl From<mir::Origin> for Origin {
    fn from(value: mir::Origin) -> Self {
        match value {
            mir::Origin::Spawn => Origin::Spawn,
            mir::Origin::Filter(clause) => Origin::EvalWhen(clause),
            mir::Origin::Eval(clause) => Origin::EvalWith(clause),
            mir::Origin::Close => Origin::Close,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// A reference of a window in the specification
pub enum WindowReference {
    /// The window is a sliding window
    Sliding(usize),
    /// The window is a discrete window
    Discrete(usize),
    /// The window is a instance aggregation
    Instance(usize),
}

impl WindowReference {
    /// The index of the window reference
    pub fn idx(self) -> usize {
        match self {
            WindowReference::Sliding(i)
            | WindowReference::Discrete(i)
            | WindowReference::Instance(i) => i,
        }
    }
}

/// Represents the type of a stream
#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum Type {
    /// An signed integer with a specific number of bits
    Int(u16),
    /// An unsigned integer with a specific number of bits
    UInt(u16),
    /// A boolean
    Bool,
    /// A string
    String,
    /// A 32 bit floating point number
    Float32,
    /// A 64 bit floating point number
    Float64,
    /// A signed fixed point number
    Fixed(u16),
    /// An unsigned fixed point number
    UFixed(u16),
    /// An optional type
    Option(Box<Type>),
    /// A tuple type
    Tuple(Vec<Type>),
    /// A bytestring
    Bytes,
}

impl Type {
    /// Returns the type inside an Option, or the type itself, if not an option
    pub fn inner_ty(&self) -> &Type {
        if let Type::Option(inner) = self {
            inner
        } else {
            self
        }
    }
}

impl StreamIr {
    /// Returns all periodic pacings that are used in guards in the StreamIR
    ///
    /// Returns a tuple, the first element holds the durations of global frequencies,
    /// the second element the reference of local frequencies
    pub fn all_periodic_pacings(&self) -> (Vec<Duration>, Vec<&LocalFreq>) {
        let global_freqs = self.stmt.all_global_freqs().into_iter().collect();
        let local_freqs = self.lref2lfreq.values().collect();
        (global_freqs, local_freqs)
    }
}

impl Stmt {
    fn all_global_freqs(&self) -> HashSet<Duration> {
        match self {
            Stmt::Skip
            | Stmt::Shift(_)
            | Stmt::Input(_)
            | Stmt::Spawn { .. }
            | Stmt::Eval { .. }
            | Stmt::Close { .. } => HashSet::new(),
            Stmt::Parallel(stmts) | Stmt::Seq(stmts) => {
                stmts.iter().flat_map(|s| s.all_global_freqs()).collect()
            }
            Stmt::Iterate { stmt, .. } | Stmt::Assign { stmt, .. } => stmt.all_global_freqs(),
            Stmt::If(IfStmt { guard, cons, alt }) => cons
                .all_global_freqs()
                .into_iter()
                .chain(alt.all_global_freqs())
                .chain(guard.all_global_freqs())
                .collect(),
        }
    }
}

impl Guard {
    fn all_global_freqs(&self) -> HashSet<Duration> {
        match self {
            Guard::GlobalFreq(duration) => vec![*duration].into_iter().collect(),
            Guard::And { lhs, rhs } | Guard::Or { lhs, rhs } => lhs
                .all_global_freqs()
                .into_iter()
                .chain(rhs.all_global_freqs())
                .collect(),
            Guard::Constant(_)
            | Guard::Stream(_)
            | Guard::LocalFreq(_)
            | Guard::Alive(_)
            | Guard::Dynamic(_)
            | Guard::FastAnd(_)
            | Guard::FastOr(_) => HashSet::new(),
        }
    }
}

/// Represent the accesses to other streams
pub type Accesses = Vec<(StreamReference, Vec<(Origin, StreamAccessKind)>)>;

/// Representation of the different stream accesses
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum StreamAccessKind {
    /// Represents the synchronous access
    Sync,
    /// Represents the access to a discrete window
    ///
    /// The argument contains the reference to the discrete window whose value is used in the [Expr].
    DiscreteWindow(WindowReference),
    /// Represents the access to a sliding window
    ///
    /// The argument contains the reference to the sliding window whose value is used in the [Expr].
    SlidingWindow(WindowReference),
    /// Represents the access to a instance aggregation
    ///
    /// The argument contains the reference to the instance aggregation whose value is used in the [Expr].
    InstanceAggregation(WindowReference),
    /// Representation of sample and hold accesses
    Hold,
    /// Representation of offset accesses
    ///
    /// The argument contains the [Offset] of the stream access.
    Offset(Offset),
    /// Represents the optional `get` access.
    Get,
    /// Represents the update check of a stream, if the target received a new value at this timestamp.
    Fresh,
}

/// Offset used in the lookup expression
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Offset {
    /// A strictly positive discrete offset, e.g., `4`, or `42`
    Future(u32),
    /// A non-negative discrete offset, e.g., `0`, `-4`, or `-42`
    Past(u32),
}

impl PartialOrd for Offset {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Offset {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        use Offset::*;
        match (self, other) {
            (Past(_), Future(_)) => Ordering::Less,
            (Future(_), Past(_)) => Ordering::Greater,
            (Future(a), Future(b)) => a.cmp(b),
            (Past(a), Past(b)) => b.cmp(a),
        }
    }
}

impl Stmt {
    /// Returns whether the statment contains iteration over the given stream reference
    pub fn contains_interate(&self, sr: OutputReference) -> bool {
        match self {
            Stmt::Skip
            | Stmt::Shift(_)
            | Stmt::Input(_)
            | Stmt::Spawn { .. }
            | Stmt::Close { .. }
            | Stmt::Eval { .. } => false,
            Stmt::Parallel(stmts) | Stmt::Seq(stmts) => {
                stmts.iter().any(|s| s.contains_interate(sr))
            }
            Stmt::Iterate { sr: srs, stmt } => srs.contains(&sr) || stmt.contains_interate(sr),
            Stmt::If(IfStmt {
                guard: _,
                cons,
                alt,
            }) => cons.contains_interate(sr) || alt.contains_interate(sr),
            Stmt::Assign { stmt, .. } => stmt.contains_interate(sr),
        }
    }
}
