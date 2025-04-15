use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    ir::{memory::Memory, Guard, IfStmt, LivetimeEquivalences, Stmt, StreamReference},
    rewrite_rules::{CombineSeq, RemoveSkip},
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule combining if conditions in sequences/parallel that have the same guard condition
pub struct CombineIf;

impl CombineIf {
    fn apply(lhs: Stmt, rhs: Stmt) -> Result<Stmt, (Stmt, Stmt)> {
        match (lhs, rhs) {
            (
                Stmt::If(IfStmt {
                    guard: lhs_g,
                    cons: lhs_cons,
                    alt: lhs_alt,
                }),
                Stmt::If(IfStmt {
                    guard: rhs_g,
                    cons: rhs_cons,
                    alt: rhs_alt,
                }),
            ) if lhs_g.eq(&rhs_g) => Ok(Stmt::If(IfStmt {
                guard: lhs_g,
                cons: Box::new(Stmt::seq([*lhs_cons, *rhs_cons])),
                alt: Box::new(Stmt::seq([*lhs_alt, *rhs_alt])),
            })),
            (lhs, rhs) => Err((lhs, rhs)),
        }
    }
}

impl RewriteRule for CombineIf {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Seq(inner) => {
                let old_length = inner.len();
                let inner: Vec<Stmt> = inner.into_iter().coalesce(Self::apply).collect();
                let cs = if old_length == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                Ok((Stmt::seq(inner), cs))
            }
            Stmt::Parallel(inner) => {
                let (guarded, unguarded): (Vec<_>, Vec<_>) =
                    inner.into_iter().partition_map(|stmt| match stmt {
                        Stmt::If(IfStmt { guard, cons, alt }) => {
                            itertools::Either::Left((guard, cons, alt))
                        }
                        stmt => itertools::Either::Right(stmt),
                    });
                let num_guarded = guarded.len();
                let guarded = guarded.into_iter().fold(
                    Vec::new(),
                    |mut guarded: Vec<(Guard, Vec<Stmt>, Vec<Stmt>)>, (guard, cons, alt)| {
                        if let Some((_, conss, alts)) = guarded
                            .iter_mut()
                            .find(|(g, _, _)| g.eq_liveness(&guard, liveness_equivalences))
                        {
                            conss.push(*cons);
                            alts.push(*alt);
                        } else {
                            guarded.push((guard, vec![*cons], vec![*alt]));
                        }
                        guarded
                    },
                );
                let guarded: Vec<Stmt> = guarded
                    .into_iter()
                    .map(|(g, cons, alt)| {
                        Stmt::If(IfStmt {
                            guard: g,
                            cons: Box::new(Stmt::parallel(cons)),
                            alt: Box::new(Stmt::parallel(alt)),
                        })
                    })
                    .collect();
                let cs = if num_guarded == guarded.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                let stmts = guarded.into_iter().chain(unguarded);
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
        rewrite_rules::{combine_if::CombineIf, Rewriter},
    };

    #[test]
    fn seq() {
        let ir = parse_ir(
            "seq {
                if @0 then
                    input 0
                fi;
                if @0 then
                    input 1
                fi
        }",
        );
        let reference = parse_ir(
            "if @0 then
                seq {
                    input 0;
                    input 1
                }
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineIf {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn alive() {
        let ir = parse_ir(
            "seq {
                if ?0 then
                    input 0
                fi;
                if ?0 then
                    input 1
                fi
        }",
        );
        let reference = parse_ir(
            "if ?0 then
                seq {
                    input 0;
                    input 1
                }
            fi
        ",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineIf {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
