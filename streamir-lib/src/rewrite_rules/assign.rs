use std::collections::HashMap;

use crate::{
    ir::{
        expressions::{Expr, ExprKind, Operator},
        memory::Memory,
        Guard, IfStmt, LivetimeEquivalences, Stmt, StreamReference,
    },
    rewrite_rules::{CombineSeq, RemoveSkip},
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that transforms an iterate statement into an assign if the parameter is uniquely defined by an
/// conditional directly inside the iterate.
pub struct IterateAssign;

impl RewriteRule for IterateAssign {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Iterate { sr, stmt } => match *stmt {
                Stmt::If(IfStmt { guard, cons, alt }) if matches!(*alt, Stmt::Skip) => {
                    match guard
                        .split_unique_parameter(memory.get(&sr[0].sr()).unwrap().num_parameters())
                    {
                        Ok((assignment, remaining)) => Ok((
                            Stmt::Assign {
                                parameter_expr: assignment,
                                sr,
                                stmt: Box::new(Stmt::If(IfStmt {
                                    guard: remaining,
                                    cons,
                                    alt,
                                })),
                            },
                            ChangeSet::local_change(),
                        )),
                        Err(guard) => Ok((
                            Stmt::Iterate {
                                sr,
                                stmt: Box::new(Stmt::If(IfStmt { guard, cons, alt })),
                            },
                            ChangeSet::default(),
                        )),
                    }
                }
                other => Ok((
                    Stmt::Iterate {
                        sr,
                        stmt: Box::new(other),
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
    fn split_unique_parameter(self, num_parameters: usize) -> Result<(Vec<Expr>, Guard), Guard> {
        match self.clone().split_unique_parameter_prime() {
            Ok((assignment, remaining)) => {
                let parameter_exprs = assignment
                    .into_iter()
                    .try_fold(vec![None; num_parameters], |mut exprs, (idx, expr)| {
                        if exprs[idx].is_none() {
                            exprs[idx] = Some(expr);
                            Ok(exprs)
                        } else {
                            Err(())
                        }
                    })
                    .and_then(|exprs| {
                        exprs
                            .into_iter()
                            .map(|i| i.ok_or(()))
                            .collect::<Result<Vec<_>, ()>>()
                    });
                match parameter_exprs {
                    Ok(exprs) => Ok((exprs, remaining)),
                    Err(()) => Err(self),
                }
            }
            Err(e) => Err(e),
        }
    }
    fn split_unique_parameter_prime(self) -> Result<(Vec<(usize, Expr)>, Guard), Guard> {
        match self {
            Guard::Constant(_)
            | Guard::Stream(_)
            | Guard::Alive(_)
            | Guard::GlobalFreq(_)
            | Guard::LocalFreq(_)
            | Guard::FastAnd(_)
            | Guard::FastOr(_) => Err(self),
            Guard::Dynamic(Expr {
                kind: ExprKind::BinaryOperation(Operator::Eq, lhs, rhs),
                ty,
            }) => match (*lhs, *rhs) {
                (
                    Expr {
                        kind: ExprKind::ParameterAccess(_, idx),
                        ..
                    },
                    other,
                )
                | (
                    other,
                    Expr {
                        kind: ExprKind::ParameterAccess(_, idx),
                        ..
                    },
                ) => Ok((vec![(idx, other)], Guard::Constant(true))),
                (lhs, rhs) => Err(Guard::Dynamic(Expr {
                    kind: ExprKind::BinaryOperation(Operator::Eq, Box::new(lhs), Box::new(rhs)),
                    ty,
                })),
            },
            Guard::Dynamic(_) => Err(self),
            Guard::And { lhs, rhs } => {
                let lhs = lhs.split_unique_parameter_prime();
                let rhs = rhs.split_unique_parameter_prime();
                match (lhs, rhs) {
                    (Ok((mut l_assignment, lhs)), Ok((r_assignment, rhs))) => {
                        l_assignment.extend(r_assignment);
                        Ok((
                            l_assignment,
                            Guard::And {
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                            },
                        ))
                    }
                    (Ok((assignment, remaining)), Err(other))
                    | (Err(other), Ok((assignment, remaining))) => Ok((
                        assignment,
                        Guard::And {
                            lhs: Box::new(remaining),
                            rhs: Box::new(other),
                        },
                    )),
                    (Err(lhs), Err(rhs)) => Err(Guard::And {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    }),
                }
            }
            Guard::Or { lhs, rhs } => Err(Guard::Or { lhs, rhs }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{
            assign::IterateAssign, skip::RemoveSkip, RemoveIfs, Rewriter, SimplifyGuard,
        },
    };

    #[test]
    fn not_enough_assignments() {
        // NOTE: in the test setup every stream has 3 parameters
        let ir = parse_ir(
            "
            iterate 0
                if Expr(p0 == s0) then
                    input 0
                fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(IterateAssign {})]);
        let (res, changed) = rewriter.apply(ir.clone()).unwrap();
        assert!(!changed);
        assert!(res.stmt.eq(&ir.stmt));
    }

    #[test]
    fn enough_assignments() {
        // NOTE: in the test setup every stream has 3 parameters
        let ir = parse_ir(
            "
            iterate 0
                if Expr(p0 == s0) && Expr(s0 == p1) && Expr(p2 == s1) then
                    input 0
                fi
        ",
        );
        let reference = parse_ir(
            "
            assign 0 (s0,s0,s1)
                input 0
        ",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(IterateAssign {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
            Box::new(RemoveSkip {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
