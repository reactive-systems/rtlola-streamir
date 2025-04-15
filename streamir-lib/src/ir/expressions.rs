//! Contains the internal representation of stream expressions inside the StreamIR

use super::{StreamReference, Type, WindowReference};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Represents a stream expression
pub struct Expr {
    /// The type of the expression
    pub ty: Type,
    /// The kind of the expression
    pub kind: ExprKind,
}

/// Represents the different kinds a expression can have
#[derive(Clone, Debug)]
pub enum ExprKind {
    /// A Constant
    Constant(Constant),
    /// An binary operation
    BinaryOperation(Operator, Box<Expr>, Box<Expr>),
    /// An unary operation
    UnaryOperation(Operator, Box<Expr>),
    /// An if-then-else operation
    Ite(Box<Expr>, Box<Expr>, Box<Expr>),
    /// An synchronous access to a stream
    SyncStreamAccess {
        /// with that stream reference
        target: StreamReference,
        /// with these parameters
        parameters: Vec<Expr>,
    },
    /// An synchronous access to a stream with an offset
    OffsetStreamAccess {
        /// with that stream reference
        target: StreamReference,
        /// the offset
        offset: u32,
        /// the default value of the access
        default: Box<Expr>,
        /// and these parameters
        parameters: Vec<Expr>,
    },
    /// An asynchronous access to a stream
    HoldStreamAccess {
        /// with that stream reference
        target: StreamReference,
        /// the default value of the access
        default: Box<Expr>,
        /// and these parameters
        parameters: Vec<Expr>,
    },
    /// An is fresh acess to a stream
    IsFresh {
        /// The target of the access
        target: StreamReference,
        /// and these parameters
        parameters: Vec<Expr>,
    },
    /// An get access to a stream
    GetAccess {
        /// the target of the access
        target: StreamReference,
        /// the default value of the access
        default: Box<Expr>,
        /// and these parameters
        parameters: Vec<Expr>,
    },
    /// The access to a window
    WindowAccess {
        /// the window aggregates of this stream
        target: StreamReference,
        /// the reference of the accessed sliding window
        window: WindowReference,
        /// the parameters
        parameters: Vec<Expr>,
        /// the default value (for e.g. average, min max window operations)
        default: Option<Box<Expr>>,
    },
    /// The cast to another type
    Cast(Type, Box<Expr>),
    /// The access of a parameter of a stream
    ParameterAccess(StreamReference, usize),
    /// A function call (with the given arguments)
    FunctionCall(Function, Vec<Expr>),
    /// The construction of a tuple expression
    Tuple(Vec<Expr>),
    /// The access of an element in a tuple
    TupleAccess(Box<Expr>, usize),
    /// The access to a parameter of a lambda expression
    LambdaParameterAccess(WindowReference, usize),
}

impl std::hash::Hash for ExprKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
/// Represent the different functions usable in expressions
pub enum Function {
    /// The square root function
    Sqrt,
    /// The absolute value function
    Abs,
    /// The sinus function
    Sin,
    /// The arcsin function
    Arcsin,
    /// The cosinus function
    Cos,
    /// The arccos function
    Arccos,
    /// The tan function
    Tan,
    /// The arctan function
    Arctan,
    /// The minimum function
    Min,
    /// The maximum function
    Max,
}

/// Represents a constant of a stream expression
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Constant {
    /// A string
    Str(String),
    /// A boolean
    Bool(bool),
    /// An unsigned integer
    UInt(u64, u16),
    /// A signed integer
    Int(i64, u16),
    /// A 32-bit floating point number
    Float32(f64),
    /// A 64-bit floating point number
    Float64(f64),
    /// A constant tuple
    Tuple(Vec<Constant>),
}

/// Represents a binary or unary operation in a stream expression
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operator {
    /// Binary negation
    Not,
    /// Numeric negation
    Neg,
    /// Addition
    Add,
    /// Subtraction
    Sub,
    /// Multiplication
    Mul,
    /// Division
    Div,
    /// Modulo
    Rem,
    /// Power
    Pow,
    /// Binary and
    And,
    /// Binary or
    Or,
    /// Bitwise xor
    BitXor,
    /// Bitwise and
    BitAnd,
    /// Bitwise or
    BitOr,
    /// Bitwise not
    BitNot,
    /// left-shift
    Shl,
    /// right-shift
    Shr,
    /// equality test
    Eq,
    /// less-than test
    Lt,
    /// less-than or equal test
    Le,
    /// not-equal test
    Ne,
    /// greater or equal test
    Ge,
    /// greater-than test
    Gt,
}

impl PartialEq for ExprKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Constant(l0), Self::Constant(r0)) => l0 == r0,
            (Self::BinaryOperation(l0, l1, l2), Self::BinaryOperation(r0, r1, r2)) => {
                l0 == r0 && l1 == r1 && l2 == r2
            }
            (Self::UnaryOperation(l0, l1), Self::UnaryOperation(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Ite(l0, l1, l2), Self::Ite(r0, r1, r2)) => l0 == r0 && l1 == r1 && l2 == r2,
            (
                Self::SyncStreamAccess {
                    target: l_target,
                    parameters: l_parameters,
                },
                Self::SyncStreamAccess {
                    target: r_target,
                    parameters: r_parameters,
                },
            ) => l_target == r_target && l_parameters == r_parameters,
            (
                Self::OffsetStreamAccess {
                    target: l_target,
                    offset: l_offset,
                    default: l_default,
                    parameters: l_parameters,
                },
                Self::OffsetStreamAccess {
                    target: r_target,
                    offset: r_offset,
                    default: r_default,
                    parameters: r_parameters,
                },
            ) => {
                l_target == r_target
                    && l_offset == r_offset
                    && l_default == r_default
                    && l_parameters == r_parameters
            }
            (
                Self::HoldStreamAccess {
                    target: l_target,
                    default: l_default,
                    parameters: l_parameters,
                },
                Self::HoldStreamAccess {
                    target: r_target,
                    default: r_default,
                    parameters: r_parameters,
                },
            ) => l_target == r_target && l_default == r_default && l_parameters == r_parameters,
            (
                Self::IsFresh {
                    target: l_target,
                    parameters: l_parameters,
                },
                Self::IsFresh {
                    target: r_target,
                    parameters: r_parameters,
                },
            ) => l_target == r_target && l_parameters == r_parameters,
            (
                Self::GetAccess {
                    target: l_target,
                    default: l_default,
                    parameters: l_parameters,
                },
                Self::GetAccess {
                    target: r_target,
                    default: r_default,
                    parameters: r_parameters,
                },
            ) => l_target == r_target && l_default == r_default && l_parameters == r_parameters,
            (
                Self::WindowAccess {
                    target: l_target,
                    window: l_window,
                    parameters: l_parameters,
                    default: l_default,
                },
                Self::WindowAccess {
                    target: r_target,
                    window: r_window,
                    parameters: r_parameters,
                    default: r_default,
                },
            ) => {
                l_target == r_target
                    && l_window == r_window
                    && l_parameters == r_parameters
                    && l_default == r_default
            }
            (Self::Cast(l0, l1), Self::Cast(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::ParameterAccess(_, l1), Self::ParameterAccess(_, r1)) => l1 == r1, // ignore the stream reference
            (Self::FunctionCall(l0, l1), Self::FunctionCall(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Tuple(l0), Self::Tuple(r0)) => l0 == r0,
            (Self::TupleAccess(l0, l1), Self::TupleAccess(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::LambdaParameterAccess(l0, l1), Self::LambdaParameterAccess(r0, r1)) => {
                l0 == r0 && l1 == r1
            }
            _ => false,
        }
    }
}

impl Eq for ExprKind {}

impl Expr {
    /// Returns whether the expression contains an expression to the streams own parameter
    pub fn contains_parameter_access(&self) -> Option<StreamReference> {
        match &self.kind {
            ExprKind::ParameterAccess(s, _) => Some(*s),
            ExprKind::Constant(_) => None,
            ExprKind::BinaryOperation(_, lhs, rhs) => lhs
                .contains_parameter_access()
                .or_else(|| rhs.contains_parameter_access()),
            ExprKind::Cast(_, expr) | ExprKind::UnaryOperation(_, expr) => {
                expr.contains_parameter_access()
            }
            ExprKind::Ite(cond, cons, alt) => cond
                .contains_parameter_access()
                .or_else(|| cons.contains_parameter_access())
                .or_else(|| alt.contains_parameter_access()),
            ExprKind::IsFresh {
                target: _,
                parameters,
            }
            | ExprKind::WindowAccess {
                target: _,
                window: _,
                parameters,
                default: None,
            }
            | ExprKind::SyncStreamAccess {
                target: _,
                parameters,
            } => parameters.iter().find_map(Self::contains_parameter_access),
            ExprKind::HoldStreamAccess {
                target: _,
                default,
                parameters,
            }
            | ExprKind::GetAccess {
                target: _,
                default,
                parameters,
            }
            | ExprKind::WindowAccess {
                target: _,
                window: _,
                parameters,
                default: Some(default),
            }
            | ExprKind::OffsetStreamAccess {
                target: _,
                offset: _,
                default,
                parameters,
            } => parameters
                .iter()
                .find_map(Self::contains_parameter_access)
                .or_else(|| default.contains_parameter_access()),
            ExprKind::FunctionCall(_, exprs) | ExprKind::Tuple(exprs) => {
                exprs.iter().find_map(Self::contains_parameter_access)
            }
            ExprKind::TupleAccess(expr, _) => expr.contains_parameter_access(),
            ExprKind::LambdaParameterAccess(_, _) => None,
        }
    }
}
