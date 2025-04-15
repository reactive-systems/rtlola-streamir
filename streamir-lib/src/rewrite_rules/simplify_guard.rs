use std::collections::HashMap;

use itertools::Itertools;

use crate::ir::{
    expressions::{Constant, Expr, ExprKind, Operator},
    memory::Memory,
    Guard, LivetimeEquivalences, StreamReference,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule simplifying guard conditions by moving conjunctions/disjunctions and constants from expression level to guard level.
pub struct SimplifyGuard;

impl RewriteRule for SimplifyGuard {
    fn rewrite_guard(
        &self,
        guard: Guard,
        _memory: &HashMap<StreamReference, Memory>,
        livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(Guard, ChangeSet), RewriteError> {
        match guard {
            Guard::LocalFreq(_) | Guard::Constant(_) | Guard::GlobalFreq(_) | Guard::Stream(_) => {
                Ok((guard, ChangeSet::default()))
            }
            Guard::FastAnd(inner) => {
                let inner_len = inner.len();
                let new_inner: Vec<_> = inner.into_iter().sorted().unique().collect();
                if inner_len != new_inner.len() {
                    Ok((Guard::FastAnd(new_inner), ChangeSet::local_change()))
                } else {
                    Ok((Guard::FastAnd(new_inner), ChangeSet::default()))
                }
            }
            Guard::FastOr(inner) => {
                let inner_len = inner.len();
                let new_inner: Vec<_> = inner.into_iter().sorted().unique().collect();
                if inner_len != new_inner.len() {
                    Ok((Guard::FastOr(new_inner), ChangeSet::local_change()))
                } else {
                    Ok((Guard::FastOr(new_inner), ChangeSet::default()))
                }
            }
            Guard::Dynamic(expr) => {
                let (guard, changed) = expr.split_guards();
                if changed {
                    Ok((guard, ChangeSet::local_change()))
                } else {
                    Ok((guard, ChangeSet::default()))
                }
            }
            Guard::And { lhs, rhs } => match (*lhs, *rhs) {
                (Guard::Constant(true), other) | (other, Guard::Constant(true)) => {
                    Ok((other, ChangeSet::local_change()))
                }
                (Guard::Constant(false), _) | (_, Guard::Constant(false)) => {
                    Ok((Guard::Constant(false), ChangeSet::local_change()))
                }
                (lhs, rhs) => Ok((
                    Guard::And {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ChangeSet::default(),
                )),
            },
            Guard::Or { lhs, rhs } => match (*lhs, *rhs) {
                (Guard::Constant(true), _) | (_, Guard::Constant(true)) => {
                    Ok((Guard::Constant(true), ChangeSet::local_change()))
                }
                (Guard::Constant(false), other) | (other, Guard::Constant(false)) => {
                    Ok((other, ChangeSet::local_change()))
                }
                (lhs, rhs) => Ok((
                    Guard::Or {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ChangeSet::default(),
                )),
            },
            Guard::Alive(sr) => {
                if livetime_equivalences.is_static(sr) {
                    Ok((Guard::Constant(true), ChangeSet::local_change()))
                } else {
                    Ok((Guard::Alive(sr), ChangeSet::default()))
                }
            }
        }
    }
}

impl Expr {
    fn split_guards(self) -> (Guard, bool) {
        let Self { ty, kind } = self;
        match kind {
            ExprKind::BinaryOperation(Operator::And, lhs, rhs) => {
                let (lhs, _) = lhs.split_guards();
                let (rhs, _) = rhs.split_guards();
                (
                    Guard::And {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    true,
                )
            }
            ExprKind::BinaryOperation(Operator::Or, lhs, rhs) => {
                let (lhs, _) = lhs.split_guards();
                let (rhs, _) = rhs.split_guards();
                (
                    Guard::Or {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    true,
                )
            }
            ExprKind::Constant(Constant::Bool(c)) => (Guard::Constant(c), true),
            kind => (Guard::Dynamic(Expr { kind, ty }), false),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{simplify_guard::SimplifyGuard, Rewriter},
    };

    #[test]
    fn remove_constants() {
        let ir = parse_ir(
            "
			if true && ( Expr(s1) || true ) then
				input 0
			fi
        ",
        );
        let reference = parse_ir(
            "if true then
				input 0
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(SimplifyGuard {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn split_expressions() {
        let ir = parse_ir(
            "
			if Expr(s0 && (s1 || s2)) then
				input 0
			fi
        ",
        );
        let reference = parse_ir(
            "if Expr(s0) && (Expr(s1) || Expr(s2)) then
				input 0
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(SimplifyGuard {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
