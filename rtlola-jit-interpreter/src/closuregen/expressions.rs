use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

use ordered_float::NotNan;
use streamir_lib::{
    formatter::expressions::ExprFormatter,
    ir::{
        expressions::{Constant, Expr, Function, Operator},
        OutputReference, StreamReference, Type, WindowReference,
    },
};

use crate::value::Value;

use super::{Closuregen, EvaluationContext, Event};

pub(crate) struct CompiledExpr(Box<dyn Fn(&EvaluationContext) -> Value>);

impl CompiledExpr {
    fn new(f: impl Fn(&EvaluationContext) -> Value + 'static) -> Self {
        Self(Box::new(f))
    }

    pub(crate) fn execute(&self, memory: &EvaluationContext) -> Value {
        (self.0)(memory)
    }
}

fn translate_constant(c: Constant) -> Value {
    match c {
        Constant::Bool(b) => Value::Bool(b),
        Constant::UInt(u, _) => Value::Unsigned(u),
        Constant::Int(i, _) => Value::Signed(i),
        Constant::Float64(f) | Constant::Float32(f) => Value::Float(NotNan::new(f).unwrap()),
        Constant::Str(s) => Value::Str(s),
        Constant::Tuple(vec) => Value::Tuple(vec.into_iter().map(translate_constant).collect()),
    }
}

impl<E: Event + 'static> ExprFormatter for Closuregen<E> {
    type Return = CompiledExpr;

    fn constant(&self, c: Constant) -> Self::Return {
        let c = translate_constant(c);
        CompiledExpr::new(move |_| c.clone())
    }

    fn unary(&self, op: Operator, operand: Expr) -> Self::Return {
        let operand = self.expr(operand);

        macro_rules! create_unop {
            ($fn:ident) => {
                CompiledExpr::new(move |ctx| {
                    let lhs = operand.execute(ctx);
                    lhs.$fn()
                })
            };
        }

        match op {
            Operator::Not => create_unop!(not),
            Operator::Neg => create_unop!(neg),
            Operator::BitNot => create_unop!(not),

            _ => unreachable!(),
        }
    }

    fn binary(&self, op: Operator, lhs: Expr, rhs: Expr) -> Self::Return {
        let lhs = self.expr(lhs);
        let rhs = self.expr(rhs);

        macro_rules! create_binop {
            ($fn:ident) => {
                CompiledExpr::new(move |ctx| {
                    let lhs = lhs.execute(ctx);
                    let rhs = rhs.execute(ctx);
                    lhs.$fn(rhs)
                })
            };
        }

        macro_rules! create_cmp {
            ($fn:ident) => {
                CompiledExpr::new(move |ctx| {
                    let lhs = lhs.execute(ctx);
                    let rhs = rhs.execute(ctx);
                    Value::Bool(lhs.$fn(&rhs))
                })
            };
        }

        macro_rules! create_lazy_binop {
            ($b:expr) => {{
                let b = Value::Bool($b);
                CompiledExpr::new(move |ctx| {
                    let lhs = lhs.execute(ctx);
                    if lhs == b {
                        b.clone()
                    } else {
                        rhs.execute(ctx)
                    }
                })
            }};
        }

        match op {
            Operator::Add => create_binop!(add),
            Operator::Sub => create_binop!(sub),
            Operator::Mul => create_binop!(mul),
            Operator::Div => create_binop!(div),
            Operator::Rem => create_binop!(rem),
            Operator::Pow => create_binop!(pow),
            Operator::BitXor => create_binop!(bitxor),
            Operator::BitAnd => create_binop!(bitand),
            Operator::BitOr => create_binop!(bitor),
            Operator::Shl => create_binop!(shl),
            Operator::Shr => create_binop!(shr),
            Operator::Eq => create_cmp!(eq),
            Operator::Lt => create_cmp!(lt),
            Operator::Le => create_cmp!(le),
            Operator::Ne => create_cmp!(ne),
            Operator::Ge => create_cmp!(ge),
            Operator::Gt => create_cmp!(gt),

            Operator::And => create_lazy_binop!(false),
            Operator::Or => create_lazy_binop!(true),

            _ => unreachable!(),
        }
    }

    fn if_then_else(&self, condition: Expr, consequence: Expr, alternative: Expr) -> Self::Return {
        let condition = self.expr(condition);
        let consequence = self.expr(consequence);
        let alternative = self.expr(alternative);

        CompiledExpr::new(move |ctx| {
            let cond = condition.execute(ctx);
            if cond.as_bool() {
                consequence.execute(ctx)
            } else {
                alternative.execute(ctx)
            }
        })
    }

    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> Self::Return {
        match sr {
            StreamReference::In(sr) => {
                CompiledExpr::new(move |ctx| ctx.memory.get_input_value(sr, 0))
            }
            StreamReference::Out(sr) => match sr {
                OutputReference::Unparameterized(i) => {
                    CompiledExpr::new(move |ctx| ctx.memory.get_output_value(i, 0))
                }
                OutputReference::Parameterized(i) => {
                    let parameters: Vec<_> = parameters
                        .into_iter()
                        .map(|param| self.expr(param))
                        .collect();
                    CompiledExpr::new(move |ctx| {
                        let parameters: Vec<_> =
                            parameters.iter().map(|param| param.execute(ctx)).collect();
                        ctx.memory.get_output_instance_value(i, &parameters, 0)
                    })
                }
            },
        }
    }

    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        let default = self.expr(default);
        match sr {
            StreamReference::In(sr) => CompiledExpr::new(move |ctx| {
                ctx.memory
                    .get_input_value(sr, offset)
                    .unwrap_or_else(|| default.execute(ctx))
            }),
            StreamReference::Out(sr) => match sr {
                OutputReference::Unparameterized(sr) => CompiledExpr::new(move |ctx| {
                    ctx.memory
                        .get_output_value(sr, offset)
                        .unwrap_or_else(|| default.execute(ctx))
                }),
                OutputReference::Parameterized(sr) => {
                    let parameters: Vec<_> = parameters
                        .into_iter()
                        .map(|param| self.expr(param))
                        .collect();
                    CompiledExpr::new(move |ctx| {
                        let parameters: Vec<_> =
                            parameters.iter().map(|param| param.execute(ctx)).collect();
                        ctx.memory
                            .get_output_instance_value(sr, &parameters, offset)
                            .unwrap_or_else(|| default.execute(ctx))
                    })
                }
            },
        }
    }

    fn hold_access(
        &self,
        sr: StreamReference,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        let default = self.expr(default);
        match sr {
            StreamReference::In(sr) => CompiledExpr::new(move |ctx| {
                ctx.memory
                    .get_input_value(sr, 0)
                    .unwrap_or_else(|| default.execute(ctx))
            }),
            StreamReference::Out(sr) => match sr {
                OutputReference::Unparameterized(sr) => CompiledExpr::new(move |ctx| {
                    ctx.memory
                        .get_output_value(sr, 0)
                        .unwrap_or_else(|| default.execute(ctx))
                }),
                OutputReference::Parameterized(sr) => {
                    let parameters: Vec<_> = parameters
                        .into_iter()
                        .map(|param| self.expr(param))
                        .collect();
                    CompiledExpr::new(move |ctx| {
                        let parameters: Vec<_> =
                            parameters.iter().map(|param| param.execute(ctx)).collect();
                        ctx.memory
                            .get_output_instance_value(sr, &parameters, 0)
                            .unwrap_or_else(|| default.execute(ctx))
                    })
                }
            },
        }
    }

    fn get_access(
        &self,
        sr: StreamReference,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> Self::Return {
        let default = self.expr(default);
        match sr {
            StreamReference::In(sr) => CompiledExpr::new(move |ctx| {
                if ctx.fresh_inputs.contains(sr) {
                    ctx.memory.get_input_value(sr, 0)
                } else {
                    default.execute(ctx)
                }
            }),
            StreamReference::Out(sr) => match sr {
                OutputReference::Unparameterized(sr) => CompiledExpr::new(move |ctx| {
                    if ctx.fresh_outputs.contains(sr) {
                        ctx.memory.get_output_value(sr, 0)
                    } else {
                        default.execute(ctx)
                    }
                }),
                OutputReference::Parameterized(sr) => {
                    let parameters: Vec<_> = parameters
                        .into_iter()
                        .map(|param| self.expr(param))
                        .collect();
                    CompiledExpr::new(move |ctx| {
                        let parameters: Vec<_> =
                            parameters.iter().map(|param| param.execute(ctx)).collect();
                        if let Some(instance) = ctx.instances.get(sr) {
                            if instance
                                .eval
                                .iter()
                                .any(|instance| (***instance) == parameters)
                            {
                                ctx.memory.get_output_instance_value(sr, &parameters, 0)
                            } else {
                                default.execute(ctx)
                            }
                        } else {
                            default.execute(ctx)
                        }
                    })
                }
            },
        }
    }

    fn is_fresh(&self, sr: StreamReference, parameters: Vec<Expr>) -> Self::Return {
        match sr {
            StreamReference::In(sr) => {
                CompiledExpr::new(move |ctx| Value::Bool(ctx.fresh_inputs.contains(sr)))
            }
            StreamReference::Out(sr) => match sr {
                OutputReference::Unparameterized(sr) => {
                    CompiledExpr::new(move |ctx| Value::Bool(ctx.fresh_outputs.contains(sr)))
                }
                OutputReference::Parameterized(sr) => {
                    let parameters: Vec<_> = parameters
                        .into_iter()
                        .map(|param| self.expr(param))
                        .collect();
                    CompiledExpr::new(move |ctx| {
                        let parameters: Vec<_> =
                            parameters.iter().map(|param| param.execute(ctx)).collect();
                        if let Some(instance) = ctx.instances.get(sr) {
                            Value::Bool(
                                instance
                                    .eval
                                    .iter()
                                    .any(|instance| (***instance) == parameters),
                            )
                        } else {
                            Value::Bool(false)
                        }
                    })
                }
            },
        }
    }

    fn sliding_window_access(&self, idx: usize, default: Option<Expr>) -> Self::Return {
        let default = default.map(|d| self.expr(d));
        let instanced = matches!(
            self.wref2window[&WindowReference::Sliding(idx)].caller,
            StreamReference::Out(OutputReference::Parameterized(_))
        );
        match (default, instanced) {
            (Some(d), false) => CompiledExpr::new(move |ctx| {
                ctx.memory
                    .sliding_window_get_value(idx, ctx.ts)
                    .unwrap_or_else(|| d.execute(ctx))
            }),
            (Some(d), true) => CompiledExpr::new(move |ctx| {
                ctx.memory
                    .instance_sliding_window_get_value(idx, ctx.parameter.as_ref().unwrap(), ctx.ts)
                    .unwrap_or_else(|| d.execute(ctx))
            }),
            (None, false) => {
                CompiledExpr::new(move |ctx| ctx.memory.sliding_window_get_value(idx, ctx.ts))
            }
            (None, true) => CompiledExpr::new(move |ctx| {
                ctx.memory.instance_sliding_window_get_value(
                    idx,
                    ctx.parameter.as_ref().unwrap(),
                    ctx.ts,
                )
            }),
        }
    }

    fn discrete_window_access(&self, _window_idx: usize, _default: Option<Expr>) -> Self::Return {
        unimplemented!("Discrete Windows are not yet implemented")
    }

    fn instance_aggregation(&self, idx: usize, default: Option<Expr>) -> Self::Return {
        match default.map(|d| self.expr(d)) {
            Some(d) => CompiledExpr::new(move |ctx| {
                ctx.memory
                    .instance_window_get_value(idx, ctx)
                    .unwrap_or_else(|| d.execute(ctx))
            }),
            None => CompiledExpr::new(move |ctx| ctx.memory.instance_window_get_value(idx, ctx)),
        }
    }

    fn parameter_access(&self, _sr: StreamReference, p: usize) -> Self::Return {
        CompiledExpr::new(move |ctx| ctx.parameter.as_ref().unwrap()[p].clone())
    }

    fn lambda_parameter_access(&self, _wref: WindowReference, idx: usize) -> Self::Return {
        CompiledExpr::new(move |ctx| ctx.lambda_parameter.borrow().as_ref().unwrap()[idx].clone())
    }

    fn cast(&self, to_ty: Type, expr: Expr) -> Self::Return {
        let from_ty = expr.ty.clone();
        let f_expr = self.expr(expr);
        macro_rules! create_convert {
            (Float, $to:ident, $ty:ty) => {
                CompiledExpr::new(move |ctx| {
                    let v = f_expr.execute(ctx);
                    match v {
                        Value::Float(f) => Value::$to(f.into_inner() as $ty),
                        v => {
                            unreachable!(
                                "Value type of {:?} does not match convert from type {:?}",
                                v,
                                Value::try_from(0.0).unwrap()
                            )
                        }
                    }
                })
            };
            ($from:ident, Float, $ty:ty) => {
                CompiledExpr::new(move |ctx| {
                    let v = f_expr.execute(ctx);
                    match v {
                        Value::$from(v) => Value::try_from(v as $ty).unwrap(),
                        v => {
                            unreachable!(
                                "Value type of {:?} does not match convert from type {:?}",
                                v,
                                Value::$from(0)
                            )
                        }
                    }
                })
            };
            ($from:ident, $to:ident, $ty:ty) => {
                CompiledExpr::new(move |ctx| {
                    let v = f_expr.execute(ctx);
                    match v {
                        Value::$from(v) => Value::$to(v as $ty),
                        v => {
                            unreachable!(
                                "Value type of {:?} does not match convert from type {:?}",
                                v,
                                Value::$from(0)
                            )
                        }
                    }
                })
            };
            ($from:ident, $to:ident, $fn:expr) => {
                CompiledExpr::new(move |ctx| {
                    let v = f_expr.execute(ctx);
                    match v {
                        Value::$from(v) => Value::$to($fn(v)),
                        v => {
                            unreachable!(
                                "Value type of {:?} does not match convert from type {:?}",
                                v,
                                stringify!($from)
                            )
                        }
                    }
                })
            };
        }
        use Type::*;
        match (from_ty, to_ty) {
            (UInt(_), UInt(_)) => f_expr,
            (UInt(_), Int(_)) => create_convert!(Unsigned, Signed, i64),
            (UInt(_), Float32 | Float64) => create_convert!(Unsigned, Float, f64),
            (Int(_), UInt(_)) => create_convert!(Signed, Unsigned, u64),
            (Int(_), Int(_)) => f_expr,
            (Int(_), Float32 | Float64) => create_convert!(Signed, Float, f64),
            (Float32 | Float64, UInt(_)) => create_convert!(Float, Unsigned, u64),
            (Float32 | Float64, Int(_)) => create_convert!(Float, Signed, i64),
            (Float32 | Float64, Float32 | Float64) => f_expr,
            (from, to) => unreachable!("from: {:?}, to: {:?}", from, to),
        }
    }

    fn tuple(&self, inner_exprs: Vec<Expr>) -> Self::Return {
        let exprs: Vec<_> = inner_exprs
            .into_iter()
            .map(|expr| self.expr(expr))
            .collect();
        CompiledExpr::new(move |ctx| {
            Value::Tuple(exprs.iter().map(|exp| exp.execute(ctx)).collect())
        })
    }

    fn tuple_access(&self, expr: Expr, idx: usize) -> Self::Return {
        let expression = self.expr(expr);
        CompiledExpr::new(move |ctx| {
            let tuple = expression.execute(ctx);
            let Value::Tuple(inner) = tuple else {
                unreachable!()
            };
            inner.into_iter().nth(idx).unwrap()
        })
    }

    fn function_call(&self, function: Function, args: Vec<Expr>) -> Self::Return {
        macro_rules! create_floatfn {
            ($fn:ident) => {{
                let arg = self.expr(args[0].clone());
                CompiledExpr::new(move |ctx| {
                    let arg = arg.execute(ctx);
                    match arg {
                        Value::Float(f) => Value::try_from(f.$fn()).unwrap(),
                        _ => unreachable!(),
                    }
                })
            }};
        }

        macro_rules! create_binary_arith {
            ($fn:ident) => {{
                let (lhs, rhs) = match &args[..] {
                    [lhs, rhs] => (lhs, rhs),
                    _ => unreachable!("wrong number of arguments for function $fn"),
                };
                let lhs = self.expr(lhs.clone());
                let rhs = self.expr(rhs.clone());
                CompiledExpr::new(move |ctx| {
                    let lhs = lhs.execute(ctx);
                    let rhs = rhs.execute(ctx);
                    match (lhs, rhs) {
                        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1.$fn(f2)),
                        (Value::Signed(s1), Value::Signed(s2)) => Value::Signed(s1.$fn(s2)),
                        (Value::Unsigned(u1), Value::Unsigned(u2)) => Value::Unsigned(u1.$fn(u2)),
                        (v1, v2) => {
                            unreachable!("wrong Value types of {:?}, {:?} for function $fn", v1, v2)
                        }
                    }
                })
            }};
        }

        match function {
            Function::Sqrt => create_floatfn!(sqrt),
            Function::Abs => {
                let arg = self.expr(args[0].clone());
                CompiledExpr::new(move |ctx| {
                    let arg = arg.execute(ctx);
                    match arg {
                        Value::Float(f) => Value::try_from(f.abs()).unwrap(),
                        Value::Signed(i) => Value::Signed(i.abs()),
                        v => unreachable!("wrong Value type of {:?}, for function abs", v),
                    }
                })
            }
            Function::Sin => create_floatfn!(sin),
            Function::Arcsin => create_floatfn!(asin),
            Function::Cos => create_floatfn!(cos),
            Function::Arccos => create_floatfn!(acos),
            Function::Tan => create_floatfn!(tan),
            Function::Arctan => create_floatfn!(atan),
            Function::Min => create_binary_arith!(min),
            Function::Max => create_binary_arith!(max),
        }
    }
}
