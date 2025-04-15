use std::collections::HashMap;

use rtlola_frontend::mir::{self, StreamAccessKind};

use crate::ir::{
    expressions::{Constant, Expr, ExprKind, Function, Operator},
    StreamReference, Type,
};

use super::LoweringError;

pub(super) fn convert_stream_expression(
    expr: mir::Expression,
    default: Option<Expr>,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<Expr, LoweringError> {
    let mir::Expression { ty, kind } = expr;

    let lir_ty = match ty {
        mir::Type::Option(inner) => *inner,
        other => other,
    }
    .into();

    let lir_kind = match kind {
        mir::ExpressionKind::LoadConstant(c) => {
            Ok(ExprKind::Constant(translate_constant(c, &lir_ty)))
        }

        mir::ExpressionKind::ArithLog(op, exprs) => translate_operator(op, exprs, sr2sr),

        mir::ExpressionKind::Ite {
            condition,
            consequence,
            alternative,
        } => translate_ite(*condition, *consequence, *alternative, sr2sr),

        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind: StreamAccessKind::Sync,
        } => translate_sync_access(target, parameters, sr2sr),

        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind: StreamAccessKind::Offset(offset),
        } => translate_offset_access(
            target,
            parameters,
            offset,
            default.ok_or(LoweringError::DefaultRequired)?,
            sr2sr,
        ),
        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind: StreamAccessKind::Hold,
        } => translate_hold_access(
            target,
            parameters,
            default.ok_or(LoweringError::DefaultRequired)?,
            sr2sr,
        ),

        mir::ExpressionKind::Default { expr, default } => {
            let default_expr = convert_stream_expression(*default, None, sr2sr)?;
            return convert_stream_expression(*expr, Some(default_expr), sr2sr);
        }
        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind:
                StreamAccessKind::SlidingWindow(window)
                | StreamAccessKind::DiscreteWindow(window)
                | StreamAccessKind::InstanceAggregation(window),
        } => translate_window(target, parameters, window, default, sr2sr),

        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind: StreamAccessKind::Fresh,
        } => translate_is_fresh(target, parameters, sr2sr),

        mir::ExpressionKind::StreamAccess {
            target,
            parameters,
            access_kind: StreamAccessKind::Get,
        } => translate_get(
            target,
            parameters,
            default.ok_or(LoweringError::DefaultRequired)?,
            sr2sr,
        ),

        mir::ExpressionKind::Convert { expr: inner_exp } => {
            translate_convert(*inner_exp, &lir_ty, sr2sr)
        }

        mir::ExpressionKind::Function(func, inner_exp) if func == "cast" => {
            let inner_exp = inner_exp.into_iter().next().unwrap();
            translate_convert(inner_exp, &lir_ty, sr2sr)
        }

        mir::ExpressionKind::ParameterAccess(target, parameter) => {
            translate_parameter_access(target, parameter, sr2sr)
        }

        mir::ExpressionKind::Function(func, inner_exps) => {
            let f = match func.as_str() {
                "sqrt" => Function::Sqrt,
                "sin" => Function::Sin,
                "arcsin" => Function::Arcsin,
                "cos" => Function::Cos,
                "arccos" => Function::Arccos,
                "tan" => Function::Tan,
                "arctan" => Function::Arctan,
                "abs" => Function::Abs,
                "min" => Function::Min,
                "max" => Function::Max,
                name => return Err(LoweringError::UnsupportedFunction(name.into())),
            };
            let args = inner_exps
                .into_iter()
                .map(|expr| convert_stream_expression(expr, None, sr2sr))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok(ExprKind::FunctionCall(f, args))
        }

        mir::ExpressionKind::Tuple(inner_exps) => {
            let inner_exps = inner_exps
                .into_iter()
                .map(|expr| convert_stream_expression(expr, None, sr2sr))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok(ExprKind::Tuple(inner_exps))
        }
        mir::ExpressionKind::TupleAccess(tuple_expr, i) => {
            let tuple_expr = convert_stream_expression(*tuple_expr, None, sr2sr)?;
            Ok(ExprKind::TupleAccess(Box::new(tuple_expr), i))
        }
        mir::ExpressionKind::LambdaParameterAccess { wref, pref } => {
            Ok(ExprKind::LambdaParameterAccess(wref.into(), pref))
        }
    }?;

    Ok(Expr {
        ty: lir_ty,
        kind: lir_kind,
    })
}

fn translate_constant(c: mir::Constant, ty: &Type) -> Constant {
    match (c, ty) {
        (mir::Constant::Str(s), _) => Constant::Str(s),
        (mir::Constant::Bool(b), _) => Constant::Bool(b),
        (mir::Constant::UInt(u), Type::UInt(b)) => Constant::UInt(u, *b),
        (mir::Constant::Int(i), Type::Int(b)) => Constant::Int(i, *b),
        (mir::Constant::Float(i), Type::Float32) => Constant::Float32(i),
        (mir::Constant::Float(i), Type::Float64) => Constant::Float64(i),
        (mir::Constant::Tuple(inner), Type::Tuple(tys)) => {
            let translated_inner = inner
                .into_iter()
                .zip(tys)
                .map(|(c, ty)| translate_constant(c, ty))
                .collect();
            Constant::Tuple(translated_inner)
        }
        (c, t) => unreachable!("ensured by type checker: {c}: {t:?}"),
    }
}

fn translate_operator(
    op: mir::ArithLogOp,
    operands: Vec<mir::Expression>,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    let operator = match op {
        mir::ArithLogOp::Not => Operator::Not,
        mir::ArithLogOp::Neg => Operator::Neg,
        mir::ArithLogOp::Add => Operator::Add,
        mir::ArithLogOp::Sub => Operator::Sub,
        mir::ArithLogOp::Mul => Operator::Mul,
        mir::ArithLogOp::Div => Operator::Div,
        mir::ArithLogOp::Rem => Operator::Rem,
        mir::ArithLogOp::Pow => Operator::Pow,
        mir::ArithLogOp::And => Operator::And,
        mir::ArithLogOp::Or => Operator::Or,
        mir::ArithLogOp::BitXor => Operator::BitXor,
        mir::ArithLogOp::BitAnd => Operator::BitAnd,
        mir::ArithLogOp::BitOr => Operator::BitOr,
        mir::ArithLogOp::BitNot => Operator::BitNot,
        mir::ArithLogOp::Shl => Operator::Shl,
        mir::ArithLogOp::Shr => Operator::Shr,
        mir::ArithLogOp::Eq => Operator::Eq,
        mir::ArithLogOp::Lt => Operator::Lt,
        mir::ArithLogOp::Le => Operator::Le,
        mir::ArithLogOp::Ne => Operator::Ne,
        mir::ArithLogOp::Ge => Operator::Ge,
        mir::ArithLogOp::Gt => Operator::Gt,
    };

    Ok(match operands.len() {
        1 => {
            let operand = operands.into_iter().next().unwrap();
            ExprKind::UnaryOperation(
                operator,
                Box::new(convert_stream_expression(operand, None, sr2sr)?),
            )
        }
        2 => {
            let (lhs, rhs) = {
                let mut op_iter = operands.into_iter();
                (op_iter.next().unwrap(), op_iter.next().unwrap())
            };
            ExprKind::BinaryOperation(
                operator,
                Box::new(convert_stream_expression(lhs, None, sr2sr)?),
                Box::new(convert_stream_expression(rhs, None, sr2sr)?),
            )
        }
        _ => unreachable!(),
    })
}

fn translate_ite(
    condition: mir::Expression,
    consequence: mir::Expression,
    alternative: mir::Expression,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::Ite(
        Box::new(convert_stream_expression(condition, None, sr2sr)?),
        Box::new(convert_stream_expression(consequence, None, sr2sr)?),
        Box::new(convert_stream_expression(alternative, None, sr2sr)?),
    ))
}

fn translate_sync_access(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::SyncStreamAccess {
        target: sr2sr[&target],
        parameters: parameters
            .into_iter()
            .map(|expr| convert_stream_expression(expr, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    })
}

fn translate_offset_access(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    offset: mir::Offset,
    default: Expr,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    let offset = match offset {
        mir::Offset::Future(_) => return Err(LoweringError::FutureAccess),
        mir::Offset::Past(i) => i,
    };

    Ok(ExprKind::OffsetStreamAccess {
        target: sr2sr[&target],
        offset,
        default: Box::new(default),
        parameters: parameters
            .into_iter()
            .map(|parameter| convert_stream_expression(parameter, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    })
}

fn translate_hold_access(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    default: Expr,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::HoldStreamAccess {
        target: sr2sr[&target],
        default: Box::new(default),
        parameters: parameters
            .into_iter()
            .map(|parameter| convert_stream_expression(parameter, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    })
}

fn translate_window(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    window: mir::WindowReference,
    default: Option<Expr>,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    let default = default.map(Box::new);

    Ok(ExprKind::WindowAccess {
        target: sr2sr[&target],
        window: window.into(),
        parameters: parameters
            .into_iter()
            .map(|x| convert_stream_expression(x, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
        default,
    })
}

fn translate_convert(
    expr: mir::Expression,
    convert_to: &Type,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::Cast(
        convert_to.clone(),
        Box::new(convert_stream_expression(expr, None, sr2sr)?),
    ))
}

fn translate_parameter_access(
    target: mir::StreamReference,
    parameter: usize,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::ParameterAccess(sr2sr[&target], parameter))
}

fn translate_is_fresh(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::IsFresh {
        target: sr2sr[&target],
        parameters: parameters
            .into_iter()
            .map(|x| convert_stream_expression(x, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    })
}

fn translate_get(
    target: mir::StreamReference,
    parameters: Vec<mir::Expression>,
    default: Expr,
    sr2sr: &HashMap<mir::StreamReference, StreamReference>,
) -> Result<ExprKind, LoweringError> {
    Ok(ExprKind::GetAccess {
        target: sr2sr[&target],
        default: Box::new(default),
        parameters: parameters
            .into_iter()
            .map(|x| convert_stream_expression(x, None, sr2sr))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    })
}
