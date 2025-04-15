//! The internal representation of windows in the specification.

use std::time::Duration;

use super::{
    expressions::Expr, memory::Parameter, Guard, Origin, StreamReference, Type, WindowReference,
};

#[derive(Debug, Clone)]
/// The representation of any window
pub struct Window {
    /// The reference of the window
    pub wref: WindowReference,
    /// The operation of the window
    pub op: WindowOperation,
    /// The target stream of the window
    pub target: StreamReference,
    /// The stream where the window expressions occurs in
    pub caller: StreamReference,
    /// The origin of the window expression
    pub origin: Origin,
    /// The pacing of the origin of the window expression
    pub origin_pacing: Guard,
    /// The kind of window (sliding, discrete, instance)
    pub kind: WindowKind,
    /// The return type of the window aggregation
    pub ty: Type,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
/// The window operation
pub enum WindowOperation {
    /// A sum aggregation
    Sum,
    /// A average aggregation
    Average,
    /// A conjunction
    Conjunction,
    /// A disjunction
    Disjunction,
    /// A minimum aggregation
    Min,
    /// A maximum aggregation
    Max,
    /// An integral aggregation
    Integral,
    /// A count aggregation
    Count,
    /// A product aggregation
    Product,
    /// A last aggregation
    Last,
    /// A variance aggregation
    Variance,
    /// A covariance aggregation
    Covariance,
    /// A standard deviation aggregation
    StandardDeviation,
    /// An nth-percentile aggregation
    NthPercentile(u8),
}

impl WindowOperation {
    /// Returns whether the window operation returns a None if no values were aggregated
    pub fn returns_option(&self) -> bool {
        match self {
            WindowOperation::Sum
            | WindowOperation::Conjunction
            | WindowOperation::Disjunction
            | WindowOperation::Integral
            | WindowOperation::Count
            | WindowOperation::Product => false,
            WindowOperation::Average
            | WindowOperation::Min
            | WindowOperation::Max
            | WindowOperation::Last
            | WindowOperation::Variance
            | WindowOperation::Covariance
            | WindowOperation::StandardDeviation
            | WindowOperation::NthPercentile(_) => true,
        }
    }
}

#[derive(Debug, Clone)]
/// The kind of a window
pub enum WindowKind {
    /// The window is a sliding window
    Sliding {
        /// The duration the window aggregates over
        duration: Duration,
        /// The number of buckets required by the window
        bucket_count: usize,
        /// The duration of a single bucket
        bucket_duration: Duration,
        /// Whether to wait for a full duration until producing values
        wait: bool,
    },
    /// The window is a discrete window
    Discrete {
        /// The number of values the window aggregates over
        num_values: usize,
        /// Whether to wait for a full duration until producing values
        wait: bool,
    },
    /// The window is a instance aggregation
    Instances {
        /// The instance selection
        selection: InstanceSelection,
    },
}

#[derive(Debug, Clone)]
/// The instance selection of a instance aggregation
pub enum InstanceSelection {
    /// All values are aggregated
    All,
    /// Only fresh values are aggregated
    Fresh,
    /// All values that satisfy a condition are aggregated
    FilteredAll {
        /// The parameters of the lambda expression
        parameters: Vec<Parameter>,
        /// The expression inside the lambda
        cond: Expr,
    },
    /// All fresh values that satisfy a condition are aggregated
    FilteredFresh {
        /// The parameters of the lambda expression
        parameters: Vec<Parameter>,
        /// The expression inside the lambda
        cond: Expr,
    },
}
