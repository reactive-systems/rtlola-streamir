use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};

use itertools::Itertools;

use crate::ir::{memory::Memory, Guard, IfStmt, LivetimeEquivalences, Stmt, StreamReference};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// Tries to find guard conditions that are used in multiple conditionals in sequences/parallel and moves them outside
pub struct MoveCommonGuardsOutside;

impl RewriteRule for MoveCommonGuardsOutside {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Seq(inner) => {
                let inner_len = inner.len();
                let new_inner = inner
                    .into_iter()
                    .coalesce(|lhs, rhs| match (lhs, rhs) {
                        (
                            Stmt::If(IfStmt {
                                guard: g1,
                                cons: c1,
                                alt: a1,
                            }),
                            Stmt::If(IfStmt {
                                guard: g2,
                                cons: c2,
                                alt: a2,
                            }),
                        ) if matches!(*a1, Stmt::Skip) && matches!(*a2, Stmt::Skip) => {
                            let lhs_conjuncts = g1.implied_subguards().collect::<HashSet<_>>();
                            let rhs_conjuncts = g2.implied_subguards().collect::<HashSet<_>>();
                            let common = lhs_conjuncts
                                .intersection(&rhs_conjuncts)
                                .collect::<Vec<_>>();
                            if common.is_empty() {
                                Err((
                                    Stmt::If(IfStmt {
                                        guard: g1,
                                        cons: c1,
                                        alt: a1,
                                    }),
                                    Stmt::If(IfStmt {
                                        guard: g2,
                                        cons: c2,
                                        alt: a2,
                                    }),
                                ))
                            } else {
                                // we keep the inner guards and rely on the `ImpliedGuards` rewriting rule to remove those
                                let common_guard = common
                                    .into_iter()
                                    .cloned()
                                    .reduce(|a, b| Guard::And {
                                        lhs: Box::new(a),
                                        rhs: Box::new(b),
                                    })
                                    .unwrap();
                                Ok(Stmt::If(IfStmt {
                                    guard: common_guard,
                                    cons: Box::new(Stmt::Seq(vec![
                                        Stmt::If(IfStmt {
                                            guard: g1,
                                            cons: c1,
                                            alt: Box::new(Stmt::Skip),
                                        }),
                                        Stmt::If(IfStmt {
                                            guard: g2,
                                            cons: c2,
                                            alt: Box::new(Stmt::Skip),
                                        }),
                                    ])),
                                    alt: Box::new(Stmt::Skip),
                                }))
                            }
                        }
                        (lhs, rhs) => Err((lhs, rhs)),
                    })
                    .collect::<Vec<_>>();
                if inner_len == new_inner.len() {
                    Ok((Stmt::Seq(new_inner), ChangeSet::default()))
                } else {
                    Ok((Stmt::Seq(new_inner), ChangeSet::local_change()))
                }
            }
            Stmt::Parallel(inner) => {
                let subguard_map = inner
                    .iter()
                    .enumerate()
                    .flat_map(|(i, stmt)| match stmt {
                        Stmt::If(IfStmt {
                            guard,
                            cons: _,
                            alt,
                        }) if matches!(**alt, Stmt::Skip) => {
                            guard.implied_subguards().map(|g| (i, g)).collect()
                        }
                        _ => vec![],
                    })
                    .fold(HashMap::<_, HashSet<_>>::new(), |mut hm, (i, g)| {
                        hm.entry(g).or_default().insert(i);
                        hm
                    });
                let Some((guard, stmts)) = subguard_map
                    .iter()
                    .sorted_by_key(|(g, _)| {
                        // for the tests we want some deterministic order
                        // (disregard hash collisions for now)
                        let mut hasher = DefaultHasher::new();
                        g.hash(&mut hasher);
                        hasher.finish()
                    })
                    .max_by_key(|(_, stmts)| stmts.len())
                    .filter(|(_, stmts)| stmts.len() > 1)
                else {
                    // there are no common if condition in the parallel statement
                    return Ok((Stmt::Parallel(inner), ChangeSet::default()));
                };

                let mut inner = inner;
                let combination = stmts
                    .iter()
                    .sorted()
                    .rev()
                    .map(|stmt_idx| inner.remove(*stmt_idx))
                    .collect::<Vec<_>>();
                inner.push(Stmt::If(IfStmt {
                    guard: guard.clone(),
                    cons: Box::new(Stmt::Parallel(combination)),
                    alt: Box::new(Stmt::Skip),
                }));
                Ok((Stmt::Parallel(inner), ChangeSet::local_change()))
            }
            other => Ok((other, ChangeSet::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{
            CombineSeq, ImpliedGuards, MoveCommonGuardsOutside, RemoveIfs, Rewriter, SimplifyGuard,
        },
    };

    #[test]
    fn test() {
        let ir = parse_ir(
            "
		seq {
			input 0 ;
			if @0 && Expr(s0) && @2 then
				input 1
			fi ;
			if Expr(s0) && (@3 || @4) then
				input 2
			fi ;
			if Expr(s0) && (@3 || @4) then
				input 3
			fi ;
			input 4
		}",
        );
        let reference = parse_ir(
            "
		seq {
			input 0;
			if Expr(s0) then
				seq {
					if @0 && @2 then
						input 1
					fi ;
					if @3 || @4 then
						input 2
					fi ;
					if @3 || @4 then
						input 3
					fi
				}
			fi ;
			input 4	
		}",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(MoveCommonGuardsOutside),
            Box::new(ImpliedGuards),
            Box::new(SimplifyGuard),
            Box::new(RemoveIfs),
            Box::new(CombineSeq),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));

        let reference2 = parse_ir(
            "
		seq {
			input 0;
			if Expr(s0) then
				seq {
					if @0 && @2 then
						input 1
					fi ;
					if @3 || @4 then
						seq {
						input 2;
						input 3
						}
					fi
				}
			fi ;
			input 4	
		}",
        );

        let (res2, changed) = rewriter.apply(res).unwrap();
        assert!(changed);
        assert!(res2.stmt.eq(&reference2.stmt));
    }

    #[test]
    fn test2() {
        let ir = parse_ir(
            "par {
				if @0 && @1 then
					input 0
				fi ; 	
				if @1 && @2 then
					input 1
				fi ; 	
				if @0 && @1 && @2 then
					input 2
				fi ; 	
				if @2 && @3 && @4 then
					input 3
				fi
			}",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(MoveCommonGuardsOutside),
            Box::new(ImpliedGuards),
            Box::new(SimplifyGuard),
            Box::new(RemoveIfs),
            Box::new(CombineSeq),
        ]);
        let res = rewriter.run(ir).unwrap();
        let reference = parse_ir(
            "par {
				if @2 && @3 && @4 then
					input 3
				fi ;
				if @1 then 
					par {
						if @2 then
							input 1
						fi ;	
						if @0 then
							par {
								input 0 ;
								if @2 then
									input 2
								fi
							}
						fi
					}
				fi
			}",
        );
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn test3() {
        let ir = parse_ir(
            "par {
				if @0 && Expr(s0) then
					input 0
				fi ; 	
				if @1 && (@0 && @2) then
					input 0
				fi ; 	
			}",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(MoveCommonGuardsOutside),
            Box::new(ImpliedGuards),
            Box::new(SimplifyGuard),
            Box::new(RemoveIfs),
            Box::new(CombineSeq),
        ]);
        let res = rewriter.run(ir).unwrap();
        println!("{}", res.display());
    }
}
