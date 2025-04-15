use std::collections::HashMap;

use crate::{
    ir::{memory::Memory, Guard, IfStmt, LivetimeEquivalences, Stmt, StreamReference},
    rewrite_rules::{CombineSeq, RemoveSkip},
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that moves if statements outside of iterate statements if the guard does not make any statements of the parameter values.
pub struct MoveIfOutside;

impl RewriteRule for MoveIfOutside {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Iterate { sr, stmt } => match *stmt {
                Stmt::If(IfStmt { guard, cons, alt }) => match guard.split_without_param_access() {
                    (None, None) => unreachable!(),
                    (None, Some(guard)) => Ok((
                        Stmt::Iterate {
                            sr,
                            stmt: Box::new(Stmt::If(IfStmt { guard, cons, alt })),
                        },
                        ChangeSet::default(),
                    )),
                    (Some(before), Some(behind)) => Ok((
                        Stmt::If(IfStmt {
                            guard: before,
                            cons: Box::new(Stmt::Iterate {
                                sr: sr.clone(),
                                stmt: Box::new(Stmt::If(IfStmt {
                                    guard: behind,
                                    cons,
                                    alt: alt.clone(),
                                })),
                            }),
                            alt: Box::new(Stmt::Iterate { sr, stmt: alt }),
                        }),
                        ChangeSet::local_change(),
                    )),
                    (Some(before), None) => Ok((
                        Stmt::If(IfStmt {
                            guard: before,
                            cons: Box::new(Stmt::Iterate {
                                sr: sr.clone(),
                                stmt: cons,
                            }),
                            alt: Box::new(Stmt::Iterate { sr, stmt: alt }),
                        }),
                        ChangeSet::local_change(),
                    )),
                },
                stmt => Ok((
                    Stmt::Iterate {
                        sr,
                        stmt: Box::new(stmt),
                    },
                    ChangeSet::default(),
                )),
            },
            stmt => Ok((stmt, ChangeSet::default())),
        }
    }

    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        vec![Box::new(RemoveSkip), Box::new(CombineSeq)]
    }
}

impl Guard {
    fn split_without_param_access(self) -> (Option<Guard>, Option<Guard>) {
        match self {
            Guard::Stream(sr) => (Some(Guard::Stream(sr)), None),
            Guard::FastAnd(inner) => (Some(Guard::FastAnd(inner)), None),
            Guard::FastOr(inner) => (Some(Guard::FastOr(inner)), None),
            Guard::Alive(sr) => (Some(Guard::Alive(sr)), None),
            Guard::Dynamic(expr) if expr.contains_parameter_access().is_some() => {
                (None, Some(Guard::Dynamic(expr)))
            }
            Guard::Dynamic(expr) => (Some(Guard::Dynamic(expr)), None),
            Guard::GlobalFreq(duration) => (Some(Guard::GlobalFreq(duration)), None),
            Guard::LocalFreq(i) => (None, Some(Guard::LocalFreq(i))),
            Guard::And { lhs, rhs } => {
                let (lhs_before, lhs_after) = lhs.split_without_param_access();
                let (rhs_before, rhs_after) = rhs.split_without_param_access();
                (
                    Self::combine_with(lhs_before, rhs_before, |lhs, rhs| Guard::And { lhs, rhs }),
                    Self::combine_with(lhs_after, rhs_after, |lhs, rhs| Guard::And { lhs, rhs }),
                )
            }
            Guard::Or { lhs, rhs } => {
                let (lhs_before, lhs_after) = lhs.split_without_param_access();
                let (rhs_before, rhs_after) = rhs.split_without_param_access();
                (
                    Self::combine_with(lhs_before, rhs_before, |lhs, rhs| Guard::Or { lhs, rhs }),
                    Self::combine_with(lhs_after, rhs_after, |lhs, rhs| Guard::Or { lhs, rhs }),
                )
            }
            Guard::Constant(b) => (Some(Guard::Constant(b)), None),
        }
    }

    fn combine_with(
        lhs: Option<Self>,
        rhs: Option<Self>,
        f: impl Fn(Box<Guard>, Box<Guard>) -> Guard,
    ) -> Option<Self> {
        match (lhs, rhs) {
            (None, None) => None,
            (None, Some(x)) | (Some(x), None) => Some(x),
            (Some(lhs), Some(rhs)) => Some(f(Box::new(lhs), Box::new(rhs))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::{expressions::Expr, parse::parse_ir, Guard, StreamReference},
        rewrite_rules::{skip::RemoveSkip, Rewriter},
    };

    use super::MoveIfOutside;

    #[test]
    fn only_ac() {
        let ir = parse_ir(
            "iterate 0
                if @0 then
                    eval 0 true 
                fi
        ",
        );
        let reference = parse_ir(
            "if @0 then
                iterate 0
                    eval 0 true 
                fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(MoveIfOutside {}), Box::new(RemoveSkip {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn only_parameter_access() {
        let ir = parse_ir(
            "iterate 0
                if Expr(s0 == p0) then
                    eval 0 true 
                fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(MoveIfOutside {}), Box::new(RemoveSkip {})]);
        let (res, changed) = rewriter.apply(ir.clone()).unwrap();
        assert!(!changed);
        assert!(res.stmt.eq(&ir.stmt));
    }

    #[test]
    fn both_kinds() {
        let ir = parse_ir(
            "iterate 0
                if @0 && Expr(s0 == p0) && ?1 then
                    eval 0 true 
                fi
        ",
        );
        let reference = parse_ir(
            "if @0 && ?1 then
                iterate 0
                    if Expr(s0 == p0) then
                        eval 0 true
                    fi
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(MoveIfOutside {}), Box::new(RemoveSkip {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn test() {
        let expr = Expr {
            ty: crate::ir::Type::Bool,
            kind: crate::ir::expressions::ExprKind::WindowAccess {
                target: StreamReference::In(0),
                window: crate::ir::WindowReference::Sliding(0),
                parameters: vec![],
                default: None,
            },
        };
        let g1 = Guard::LocalFreq(0);
        let g2 = Guard::Dynamic(expr);
        let g = Guard::And {
            lhs: Box::new(g1),
            rhs: Box::new(g2),
        };
        dbg!(g.split_without_param_access());
    }
}
