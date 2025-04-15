use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    ir::{
        expressions::Expr, memory::Memory, LivetimeEquivalences, OutputReference, Stmt,
        StreamReference,
    },
    rewrite_rules::{CombineSeq, RemoveSkip},
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewring rule combining iterates in sequences/parallel that have the same spawn/close behavior.
pub struct CombineIterate;

impl CombineIterate {
    fn apply(lhs: Stmt, rhs: Stmt, eq: &LivetimeEquivalences) -> Result<Stmt, (Stmt, Stmt)> {
        match (lhs, rhs) {
            (
                Stmt::Iterate {
                    sr: sr1,
                    stmt: stmt1,
                },
                Stmt::Iterate {
                    sr: sr2,
                    stmt: stmt2,
                },
            ) if eq.is_equivalent_outputs(sr1[0], sr2[0]) => Ok(Stmt::Iterate {
                sr: sr1.into_iter().chain(sr2).unique().collect(),
                stmt: Box::new(Stmt::seq([*stmt1, *stmt2])),
            }),
            (
                Stmt::Assign {
                    sr: sr1,
                    parameter_expr: exp1,
                    stmt: stmt1,
                },
                Stmt::Assign {
                    sr: sr2,
                    parameter_expr: exp2,
                    stmt: stmt2,
                },
            ) if exp1.eq(&exp2) => Ok(Stmt::Assign {
                sr: sr1.into_iter().chain(sr2).unique().collect(),
                parameter_expr: exp1,
                stmt: Box::new(Stmt::seq([*stmt1, *stmt2])),
            }),
            (lhs, rhs) => Err((lhs, rhs)),
        }
    }
}

impl RewriteRule for CombineIterate {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Seq(inner) => {
                let old_length = inner.len();
                let inner: Vec<Stmt> = inner
                    .into_iter()
                    .coalesce(|a, b| Self::apply(a, b, liveness_equivalences))
                    .collect();
                let cs = if old_length == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                Ok((Stmt::seq(inner), cs))
            }
            Stmt::Parallel(inner) => {
                let (iterated, assigned, other) = inner.into_iter().fold(
                    (Vec::new(), Vec::new(), Vec::new()),
                    |(mut iterated, mut assigned, mut other), stmt| {
                        match stmt {
                            Stmt::Iterate { sr, stmt } => {
                                iterated.push((sr, stmt));
                            }
                            Stmt::Assign {
                                sr,
                                parameter_expr,
                                stmt,
                            } => {
                                assigned.push((sr, parameter_expr, stmt));
                            }
                            stmt => other.push(stmt),
                        }
                        (iterated, assigned, other)
                    },
                );
                let num_iterated = iterated.len();
                let num_assigned = assigned.len();
                let iterated = iterated.into_iter().fold(
                    Vec::new(),
                    |mut iterated: Vec<(Vec<OutputReference>, Vec<Stmt>)>, (sr, stmt)| {
                        if let Some((srs, stmt2)) = iterated.iter_mut().find(|(sr1, _)| {
                            liveness_equivalences.is_equivalent_outputs(sr1[0], sr[0])
                        }) {
                            stmt2.push(*stmt);
                            srs.extend(sr);
                        } else {
                            iterated.push((sr, vec![*stmt]));
                        }
                        iterated
                    },
                );
                let assigned = assigned.into_iter().fold(
                    Vec::new(),
                    |mut assigned: Vec<(Vec<OutputReference>, Vec<Expr>, Vec<Stmt>)>,
                     (sr, expr, stmt)| {
                        if let Some((srs, _, stmt2)) = assigned.iter_mut().find(|(_, expr1, _)| {
                            expr.iter()
                                .zip_longest(expr1.iter())
                                .all(|r| r.both().map(|(a, b)| a.eq(b)).unwrap_or_default())
                        }) {
                            stmt2.push(*stmt);
                            srs.extend(sr);
                        } else {
                            assigned.push((sr, expr, vec![*stmt]));
                        }
                        assigned
                    },
                );
                let iterated: Vec<Stmt> = iterated
                    .into_iter()
                    .map(|(sr, stmt)| Stmt::Iterate {
                        sr: sr.into_iter().unique().collect(),
                        stmt: Box::new(Stmt::parallel(stmt)),
                    })
                    .collect();
                let assigned: Vec<Stmt> = assigned
                    .into_iter()
                    .map(|(sr, parameter_expr, stmt)| Stmt::Assign {
                        sr: sr.into_iter().unique().collect(),
                        parameter_expr,
                        stmt: Box::new(Stmt::parallel(stmt)),
                    })
                    .collect();
                let cs = if num_iterated == iterated.len() && num_assigned == assigned.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                let stmts = iterated.into_iter().chain(assigned).chain(other);
                Ok((Stmt::parallel(stmts), cs))
            }
            stmt => Ok((stmt, ChangeSet::default())),
        }
    }

    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        vec![Box::new(RemoveSkip), Box::new(CombineSeq)]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{combine_iterate::CombineIterate, Rewriter},
    };

    #[test]
    fn seq() {
        let ir = parse_ir(
            "seq {
                iterate 0
                    input 0;
                iterate 0
                    input 1
        }",
        );
        let reference = parse_ir(
            "iterate 0
                seq {
                    input 0;
                    input 1
                }
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineIterate {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn alive() {
        let ir = parse_ir(
            "seq {
                iterate 0
                    input 0;
                iterate 0
                    input 1
        }",
        );
        let reference = parse_ir(
            "iterate 0
                seq {
                    input 0;
                    input 1
                }
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineIterate {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
