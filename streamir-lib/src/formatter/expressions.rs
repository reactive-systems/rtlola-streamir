//! Contains traits for formatting stream expressions in the target language

use itertools::Itertools;

use crate::ir::{
    expressions::{Constant, Expr, ExprKind, Function, Operator},
    StreamReference, Type, WindowReference,
};

/// A trait definining how the parts of the stream expression are represented in the target language.
///
/// The convenience trait [DefaultExprFormatter] can be used if the return type is a string.
pub trait ExprFormatter {
    /// The return type of the formatter.
    type Return;

    /// Formats a constant
    fn constant(&self, c: Constant) -> Self::Return;

    /// Returns the code for the application of an unary operator
    fn unary(&self, op: Operator, operand: Expr) -> Self::Return;

    /// Returns the code for the application of an binary operator
    fn binary(&self, op: Operator, lhs: Expr, rhs: Expr) -> Self::Return;

    /// Returns the code for an if-then-else operator
    fn if_then_else(&self, condition: Expr, consequence: Expr, alternative: Expr) -> Self::Return;

    /// Returns the code, that is used to access the given stream synchronously.
    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> Self::Return;

    /// Returns the code, that is used to access the given stream with an offset.
    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return;

    /// Returns the code, that is used to access the given stream asyncronously.
    fn hold_access(
        &self,
        sr: StreamReference,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return;

    /// Returns the code that is used to access a stream with a get access.
    fn get_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>)
        -> Self::Return;

    /// Returns the code that is used to access a stream with is_fresh.
    fn is_fresh(&self, sr: StreamReference, parameters: Vec<Expr>) -> Self::Return;

    /// Returns the code, that is used to access the given sliding window.
    fn sliding_window_access(&self, wref: usize, default: Option<Expr>) -> Self::Return;

    /// Returns the code, that is used to access the given discrete window.
    fn discrete_window_access(&self, wref: usize, default: Option<Expr>) -> Self::Return;

    /// Returns the code, that is used to access an instance aggregation window.
    fn instance_aggregation(&self, wref: usize, default: Option<Expr>) -> Self::Return;

    /// Returns the code, that is used to access a streams parameter.
    fn parameter_access(&self, sr: StreamReference, p: usize) -> Self::Return;

    /// Returns the code, that is used to access the parameter of a lambda Expr
    fn lambda_parameter_access(&self, wref: WindowReference, idx: usize) -> Self::Return;

    /// Returns the code, that is used to cast an Expr to a given type.
    fn cast(&self, ty: Type, expr: Expr) -> Self::Return;

    /// Returns the code, that constructs a tuple out of its elements.
    fn tuple(&self, inner_exprs: Vec<Expr>) -> Self::Return;

    /// Returns the code, that is used to access a tuple element.
    fn tuple_access(&self, expr: Expr, idx: usize) -> Self::Return;

    /// Returns the code, that is used to call a function.
    fn function_call(&self, function: Function, args: Vec<Expr>) -> Self::Return;

    /// Formats any expression in the target language.
    fn expr(&self, expr: Expr) -> Self::Return {
        match expr.kind {
            ExprKind::Constant(c) => self.constant(c),
            ExprKind::BinaryOperation(op, lhs, rhs) => self.binary(op, *lhs, *rhs),
            ExprKind::UnaryOperation(op, operand) => self.unary(op, *operand),
            ExprKind::Ite(condition, consequence, alternative) => {
                self.if_then_else(*condition, *consequence, *alternative)
            }
            ExprKind::SyncStreamAccess { target, parameters } => {
                self.sync_access(target, parameters)
            }
            ExprKind::OffsetStreamAccess {
                target,
                offset,
                default,
                parameters,
            } => self.offset_access(target, offset, *default, parameters),
            ExprKind::HoldStreamAccess {
                target,
                default,
                parameters,
            } => self.hold_access(target, *default, parameters),
            ExprKind::IsFresh { target, parameters } => self.is_fresh(target, parameters),
            ExprKind::GetAccess {
                target,
                default,
                parameters,
            } => self.get_access(target, *default, parameters),
            ExprKind::WindowAccess {
                target: _,
                window,
                parameters: _,
                default,
            } => match window {
                WindowReference::Sliding(w) => self.sliding_window_access(w, default.map(|b| *b)),
                WindowReference::Discrete(w) => self.discrete_window_access(w, default.map(|b| *b)),
                WindowReference::Instance(w) => self.instance_aggregation(w, default.map(|b| *b)),
            },
            ExprKind::Cast(ty, inner) => self.cast(ty, *inner),
            ExprKind::ParameterAccess(sr, p) => self.parameter_access(sr, p),
            ExprKind::FunctionCall(f, args) => self.function_call(f, args),
            ExprKind::Tuple(inner) => self.tuple(inner),
            ExprKind::TupleAccess(tuple, idx) => self.tuple_access(*tuple, idx),
            ExprKind::LambdaParameterAccess(window_reference, idx) => {
                self.lambda_parameter_access(window_reference, idx)
            }
        }
    }
}

/// A convenience trait for [ExprFormatter]'s that return strings containing default implementations for common operations.
pub trait DefaultExprFormatter
where
    Self: ExprFormatter<Return = String>
        + OperatorFormatter<Return = String>
        + FunctionFormatter<Return = String>
        + ConstantFormatter<Return = String>,
{
    /// Formats a constant
    fn constant(&self, c: Constant) -> String {
        <Self as ConstantFormatter>::constant(self, c)
    }

    /// Returns the code for the application of an unary operator
    fn unary(&self, op: Operator, operand: Expr) -> String {
        format!("({}{})", self.op(op), self.expr(operand))
    }

    /// Returns the code for the application of an binary operator
    fn binary(&self, op: Operator, lhs: Expr, rhs: Expr) -> String {
        format!("({}{}{})", self.expr(lhs), self.op(op), self.expr(rhs))
    }

    /// Returns the code for an if-then-else operator
    fn if_then_else(&self, condition: Expr, consequence: Expr, alternative: Expr) -> String {
        format!(
            "({}?{}:{})",
            self.expr(condition),
            self.expr(consequence),
            self.expr(alternative)
        )
    }

    /// Returns the code, that is used to access the given stream synchronously.
    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> String;

    /// Returns the code, that is used to access the given stream with an offset.
    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> String;

    /// Returns the code, that is used to access the given stream asyncronously.
    fn hold_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String;

    /// Returns the code that is used to access a stream with a get access.
    fn get_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String;

    /// Returns the code that is used to access a stream with is_fresh.
    fn is_fresh(&self, sr: StreamReference, parameters: Vec<Expr>) -> String;

    /// Returns the code, that is used to access the given sliding window.
    fn sliding_window_access(&self, wref: usize, default: Option<Expr>) -> String;

    /// Returns the code, that is used to access the given discrete window.
    fn discrete_window_access(&self, wref: usize, default: Option<Expr>) -> String;

    /// Returns the code, that is used to access an instance aggregation window.
    fn instance_aggregation(&self, wref: usize, default: Option<Expr>) -> String;

    /// Returns the code, that is used to access a streams parameter.
    fn parameter_access(&self, sr: StreamReference, p: usize) -> String;

    /// Returns the code, that is used to access the parameter of a lambda Expr
    fn lambda_parameter_access(&self, wref: WindowReference, idx: usize) -> String;

    /// Returns the code, that is used to cast an Expr to a given type.
    fn cast(&self, ty: Type, expr: Expr) -> String;

    /// Returns the code, that constructs a tuple out of its elements.
    fn tuple(&self, inner: Vec<Expr>) -> String {
        format!("({})", inner.into_iter().map(|e| self.expr(e)).join(", "))
    }

    /// Returns the code, that is used to access a tuple element.
    fn tuple_access(&self, expr: Expr, i: usize) -> String {
        format!("{}.{}", self.expr(expr), i)
    }

    /// Returns the code, that is used to call a function.
    fn function_call(&self, function: Function, args: Vec<Expr>) -> String {
        let args = args.into_iter().map(|e| self.expr(e)).join(", ");
        format!("{}({})", self.function(function), args)
    }
}

impl<X> ExprFormatter for X
where
    X: DefaultExprFormatter,
{
    type Return = String;

    fn constant(&self, c: Constant) -> Self::Return {
        <Self as DefaultExprFormatter>::constant(self, c)
    }

    fn unary(&self, op: Operator, operand: Expr) -> Self::Return {
        <Self as DefaultExprFormatter>::unary(self, op, operand)
    }

    fn binary(&self, op: Operator, lhs: Expr, rhs: Expr) -> Self::Return {
        <Self as DefaultExprFormatter>::binary(self, op, lhs, rhs)
    }

    fn if_then_else(&self, condition: Expr, consequence: Expr, alternative: Expr) -> Self::Return {
        <Self as DefaultExprFormatter>::if_then_else(self, condition, consequence, alternative)
    }

    fn sync_access(&self, stream: StreamReference, parameters: Vec<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::sync_access(self, stream, parameters)
    }

    fn offset_access(
        &self,
        stream: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        <Self as DefaultExprFormatter>::offset_access(self, stream, offset, default, parameters)
    }

    fn hold_access(
        &self,
        stream: StreamReference,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        <Self as DefaultExprFormatter>::hold_access(self, stream, default, parameters)
    }

    fn get_access(
        &self,
        stream: StreamReference,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        <Self as DefaultExprFormatter>::get_access(self, stream, default, parameters)
    }

    fn is_fresh(&self, stream: StreamReference, parameters: Vec<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::is_fresh(self, stream, parameters)
    }

    fn sliding_window_access(&self, window_idx: usize, default: Option<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::sliding_window_access(self, window_idx, default)
    }

    fn discrete_window_access(&self, window_idx: usize, default: Option<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::discrete_window_access(self, window_idx, default)
    }

    fn instance_aggregation(&self, window_idx: usize, default: Option<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::instance_aggregation(self, window_idx, default)
    }

    fn parameter_access(&self, sr: StreamReference, p: usize) -> Self::Return {
        <Self as DefaultExprFormatter>::parameter_access(self, sr, p)
    }

    fn lambda_parameter_access(&self, wref: WindowReference, idx: usize) -> Self::Return {
        <Self as DefaultExprFormatter>::lambda_parameter_access(self, wref, idx)
    }

    fn cast(&self, ty: Type, expr: Expr) -> Self::Return {
        <Self as DefaultExprFormatter>::cast(self, ty, expr)
    }

    fn tuple(&self, inner_exprs: Vec<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::tuple(self, inner_exprs)
    }

    fn tuple_access(&self, expr: Expr, i: usize) -> Self::Return {
        <Self as DefaultExprFormatter>::tuple_access(self, expr, i)
    }

    fn function_call(&self, function: Function, args: Vec<Expr>) -> Self::Return {
        <Self as DefaultExprFormatter>::function_call(self, function, args)
    }
}

/// A formatter defining how operators are represented in the target language.
/// (See also [DefaultOperatorFormatter]).
pub trait OperatorFormatter {
    /// The type returned by the formatter.
    type Return;

    /// Returns the Self::Return for the boolean negation operator
    fn op_not(&self) -> Self::Return;

    /// Returns the Self::Return for the numeric negation operator
    fn op_neg(&self) -> Self::Return;

    /// Returns the Self::Return for the add operator
    fn op_add(&self) -> Self::Return;

    /// Returns the Self::Return for the substraction operator
    fn op_sub(&self) -> Self::Return;

    /// Returns the Self::Return for the multiplication operator
    fn op_mul(&self) -> Self::Return;

    /// Returns the Self::Return for the division operator
    fn op_div(&self) -> Self::Return;

    /// Returns the Self::Return for the modulo operator
    fn op_rem(&self) -> Self::Return;

    /// Returns the Self::Return for the power operator
    fn op_pow(&self) -> Self::Return;

    /// Returns the boolean and operator
    fn op_and(&self) -> Self::Return;

    /// Returns the boolean or operator
    fn op_or(&self) -> Self::Return;

    /// Returns the bitwise xor operator
    fn op_bitxor(&self) -> Self::Return;

    /// Returns the bitwise and operator
    fn op_bitand(&self) -> Self::Return;

    /// Returns the bitwise or operator
    fn op_bitor(&self) -> Self::Return;

    /// Returns the bitwise not operator
    fn op_bitnot(&self) -> Self::Return;

    /// Returns the shift-right operator
    fn op_shr(&self) -> Self::Return;

    /// Returns the shift-left operator
    fn op_shl(&self) -> Self::Return;

    /// Returns the equality operator
    fn op_eq(&self) -> Self::Return;

    /// Returns the less-than operator
    fn op_lt(&self) -> Self::Return;

    /// Returns the less-than or equal operator
    fn op_le(&self) -> Self::Return;

    /// Returns the not equal operator
    fn op_ne(&self) -> Self::Return;

    /// Returns the greater than or equal operator
    fn op_ge(&self) -> Self::Return;

    /// Returns the greater than operator
    fn op_gt(&self) -> Self::Return;

    /// Represents an operator in the target language.
    fn op(&self, op: Operator) -> Self::Return {
        match op {
            Operator::Not => self.op_not(),
            Operator::Neg => self.op_neg(),
            Operator::Add => self.op_add(),
            Operator::Sub => self.op_sub(),
            Operator::Mul => self.op_mul(),
            Operator::Div => self.op_div(),
            Operator::Rem => self.op_rem(),
            Operator::Pow => self.op_pow(),
            Operator::And => self.op_and(),
            Operator::Or => self.op_or(),
            Operator::BitXor => self.op_bitxor(),
            Operator::BitAnd => self.op_bitand(),
            Operator::BitOr => self.op_bitor(),
            Operator::BitNot => self.op_bitnot(),
            Operator::Shl => self.op_shl(),
            Operator::Shr => self.op_shr(),
            Operator::Eq => self.op_eq(),
            Operator::Lt => self.op_lt(),
            Operator::Le => self.op_le(),
            Operator::Ne => self.op_ne(),
            Operator::Ge => self.op_ge(),
            Operator::Gt => self.op_gt(),
        }
    }
}

/// A convenience trait for [OperatorFormatter]'s that return a string with default implementations.
pub trait DefaultOperatorFormatter {
    /// Returns the string for the boolean negation operator
    fn op_not(&self) -> String {
        "!".into()
    }

    /// Returns the string for the numeric negation operator
    fn op_neg(&self) -> String {
        "-".into()
    }

    /// Returns the string for the add operator
    fn op_add(&self) -> String {
        "+".into()
    }

    /// Returns the string for the substraction operator
    fn op_sub(&self) -> String {
        "-".into()
    }

    /// Returns the string for the multiplication operator
    fn op_mul(&self) -> String {
        "*".into()
    }

    /// Returns the string for the division operator
    fn op_div(&self) -> String {
        "/".into()
    }

    /// Returns the string for the modulo operator
    fn op_rem(&self) -> String {
        "%".into()
    }

    /// Returns the string for the power operator
    fn op_pow(&self) -> String {
        "**".into()
    }

    /// Returns the boolean and operator
    fn op_and(&self) -> String {
        "&&".into()
    }

    /// Returns the boolean or operator
    fn op_or(&self) -> String {
        "||".into()
    }

    /// Returns the bitwise xor operator
    fn op_bitxor(&self) -> String {
        "^".into()
    }

    /// Returns the bitwise and operator
    fn op_bitand(&self) -> String {
        "&".into()
    }

    /// Returns the bitwise or operator
    fn op_bitor(&self) -> String {
        "|".into()
    }

    /// Returns the bitwise not operator
    fn op_bitnot(&self) -> String {
        "~".into()
    }

    /// Returns the shift-right operator
    fn op_shr(&self) -> String {
        ">>".into()
    }

    /// Returns the shift-left operator
    fn op_shl(&self) -> String {
        "<<".into()
    }

    /// Returns the equality operator
    fn op_eq(&self) -> String {
        "==".into()
    }

    /// Returns the less-than operator
    fn op_lt(&self) -> String {
        "<".into()
    }

    /// Returns the less-than or equal operator
    fn op_le(&self) -> String {
        "<=".into()
    }

    /// Returns the not equal operator
    fn op_ne(&self) -> String {
        "!=".into()
    }

    /// Returns the greater than or equal operator
    fn op_ge(&self) -> String {
        ">=".into()
    }

    /// Returns the greater than operator
    fn op_gt(&self) -> String {
        ">".into()
    }
}

impl<X> OperatorFormatter for X
where
    X: DefaultOperatorFormatter,
{
    type Return = String;

    fn op_not(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_not(self)
    }

    fn op_neg(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_neg(self)
    }

    fn op_add(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_add(self)
    }

    fn op_sub(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_sub(self)
    }

    fn op_mul(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_mul(self)
    }

    fn op_div(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_div(self)
    }

    fn op_rem(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_rem(self)
    }

    fn op_pow(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_pow(self)
    }

    fn op_and(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_and(self)
    }

    fn op_or(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_or(self)
    }

    fn op_bitxor(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_bitxor(self)
    }

    fn op_bitand(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_bitand(self)
    }

    fn op_bitor(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_bitor(self)
    }

    fn op_bitnot(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_bitnot(self)
    }

    fn op_shr(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_shr(self)
    }

    fn op_shl(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_shl(self)
    }

    fn op_eq(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_eq(self)
    }

    fn op_lt(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_lt(self)
    }

    fn op_le(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_le(self)
    }

    fn op_ne(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_ne(self)
    }

    fn op_ge(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_ge(self)
    }

    fn op_gt(&self) -> Self::Return {
        <Self as DefaultOperatorFormatter>::op_gt(self)
    }
}

/// A trait defining how functions are represented in the target language. (See also [DefaultFunctionFormatter]).
pub trait FunctionFormatter {
    /// The return type of the formatter
    type Return;

    /// Returns the name of the squareroot function in the target language.
    fn function_sqrt(&self) -> Self::Return;

    /// Returns the name of the absolute value function in the target language.
    fn function_abs(&self) -> Self::Return;

    /// Returns the name of the sin function in the target language.
    fn function_sin(&self) -> Self::Return;

    /// Returns the name of the arcsin function in the target language.
    fn function_arcsin(&self) -> Self::Return;

    /// Returns the name of the cos function in the target language.
    fn function_cos(&self) -> Self::Return;

    /// Returns the name of the arccos function in the target language.
    fn function_arccos(&self) -> Self::Return;

    /// Returns the name of the tan function in the target language.
    fn function_tan(&self) -> Self::Return;

    /// Returns the name of the arctan function in the target language.
    fn function_arctan(&self) -> Self::Return;

    /// Returns the name of the min function in the target language.
    fn function_min(&self) -> Self::Return;

    /// Returns the name of the max function in the target language.
    fn function_max(&self) -> Self::Return;

    /// Formats a function in the target language.
    fn function(&self, f: Function) -> Self::Return {
        match f {
            Function::Sqrt => self.function_sqrt(),
            Function::Abs => self.function_abs(),
            Function::Sin => self.function_sin(),
            Function::Arcsin => self.function_arcsin(),
            Function::Cos => self.function_cos(),
            Function::Arccos => self.function_arccos(),
            Function::Tan => self.function_tan(),
            Function::Arctan => self.function_arctan(),
            Function::Min => self.function_min(),
            Function::Max => self.function_max(),
        }
    }
}

/// A convenience trait for [FunctionFormatter] that returns strings with default implementations.
pub trait DefaultFunctionFormatter {
    /// Returns the name of the squareroot function in the target language.
    fn function_sqrt(&self) -> String {
        "sqrt".into()
    }

    /// Returns the name of the absolute value function in the target language.
    fn function_abs(&self) -> String {
        "abs".into()
    }

    /// Returns the name of the sin function in the target language.
    fn function_sin(&self) -> String {
        "sin".into()
    }

    /// Returns the name of the arcsin function in the target language.
    fn function_arcsin(&self) -> String {
        "arcsin".into()
    }

    /// Returns the name of the cos function in the target language.
    fn function_cos(&self) -> String {
        "cos".into()
    }

    /// Returns the name of the arccos function in the target language.
    fn function_arccos(&self) -> String {
        "arccos".into()
    }

    /// Returns the name of the tan function in the target language.
    fn function_tan(&self) -> String {
        "tan".into()
    }

    /// Returns the name of the arctan function in the target language.
    fn function_arctan(&self) -> String {
        "arctan".into()
    }

    /// Returns the name of the min function in the target language.
    fn function_min(&self) -> String {
        "min".into()
    }

    /// Returns the name of the max function in the target language.
    fn function_max(&self) -> String {
        "max".into()
    }
}

impl<X> FunctionFormatter for X
where
    X: DefaultFunctionFormatter,
{
    type Return = String;

    fn function_sqrt(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_sqrt(self)
    }

    fn function_abs(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_abs(self)
    }

    fn function_sin(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_sin(self)
    }

    fn function_arcsin(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_arcsin(self)
    }

    fn function_cos(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_cos(self)
    }

    fn function_arccos(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_arccos(self)
    }

    fn function_tan(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_tan(self)
    }

    fn function_arctan(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_arctan(self)
    }

    fn function_min(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_min(self)
    }

    fn function_max(&self) -> Self::Return {
        <Self as DefaultFunctionFormatter>::function_max(self)
    }
}

/// A trait defining how constants are represented in the target language. (See also [DefaultConstantFormatter])
pub trait ConstantFormatter {
    /// The return type of the formatter.
    type Return;

    /// Returns the Self::Return that the given language uses to format a string.
    fn constant_bool(&self, b: bool) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the string.
    fn constant_string(&self, s: String) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the uint
    fn constant_uint(&self, i: u64, _bits: u16) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the int
    fn constant_int(&self, i: i64, _bits: u16) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the float
    fn constant_float32(&self, f: f64) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the float
    fn constant_float64(&self, f: f64) -> Self::Return;

    /// Returns the Self::Return that the given language uses to format the tuple
    fn constant_tuple(&self, f: Vec<Constant>) -> Self::Return;

    /// Formats a constant in the target language.
    fn constant(&self, c: Constant) -> Self::Return {
        match c {
            Constant::Str(s) => self.constant_string(s),
            Constant::Bool(b) => self.constant_bool(b),
            Constant::UInt(u, b) => self.constant_uint(u, b),
            Constant::Int(u, b) => self.constant_int(u, b),
            Constant::Float32(f) => self.constant_float32(f),
            Constant::Float64(f) => self.constant_float64(f),
            Constant::Tuple(constants) => self.constant_tuple(constants),
        }
    }
}

/// A convenience trait for [ConstantFormatter] that return strings with default implementations.
pub trait DefaultConstantFormatter
where
    Self: ConstantFormatter<Return = String>,
{
    /// Returns the string that the given language uses to format the boolean
    fn constant_bool(&self, b: bool) -> String {
        match b {
            true => "true",
            false => "false",
        }
        .into()
    }

    /// Returns the string that the given language uses to format the string
    fn constant_string(&self, s: String) -> String {
        format!("\"{s}\"")
    }

    /// Returns the string that the given language uses to format the uint
    fn constant_uint(&self, i: u64, _bits: u16) -> String {
        i.to_string()
    }

    /// Returns the string that the given language uses to format the int
    fn constant_int(&self, i: i64, _bits: u16) -> String {
        i.to_string()
    }

    /// Returns the string that the fiven language uses to format the float
    fn constant_float32(&self, f: f64) -> String {
        f.to_string()
    }

    /// Returns the string that the fiven language uses to format the float
    fn constant_float64(&self, f: f64) -> String {
        f.to_string()
    }

    /// Returns the string that the fiven language uses to format the tuple
    fn constant_tuple(&self, f: Vec<Constant>) -> String {
        let fields = f.into_iter().map(|v| self.constant(v)).join(", ");
        format!("({})", fields)
    }
}

impl<X> ConstantFormatter for X
where
    X: DefaultConstantFormatter,
{
    type Return = String;

    fn constant_bool(&self, b: bool) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_bool(self, b)
    }

    fn constant_string(&self, s: String) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_string(self, s)
    }

    fn constant_uint(&self, i: u64, _bits: u16) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_uint(self, i, _bits)
    }

    fn constant_int(&self, i: i64, _bits: u16) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_int(self, i, _bits)
    }

    fn constant_float32(&self, f: f64) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_float32(self, f)
    }

    fn constant_float64(&self, f: f64) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_float64(self, f)
    }

    fn constant_tuple(&self, f: Vec<Constant>) -> Self::Return {
        <Self as DefaultConstantFormatter>::constant_tuple(self, f)
    }
}
