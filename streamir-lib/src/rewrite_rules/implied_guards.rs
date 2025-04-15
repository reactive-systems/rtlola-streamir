use std::iter;

use crate::ir::{Guard, IfStmt, LivetimeEquivalences, Stmt};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// Remove guard conditions that are already implied because the guard is nested inside an outer guard block.
pub struct ImpliedGuards;

impl RewriteRule for ImpliedGuards {
    fn apply_stmt(
        &self,
        stmt: Stmt,
        _memory: &std::collections::HashMap<crate::ir::StreamReference, crate::ir::memory::Memory>,
        _le: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        Self::remove_implied_guards(stmt, Vec::new())
    }
}

impl ImpliedGuards {
    fn remove_implied_guards(
        stmt: Stmt,
        implied_guards: Vec<Guard>,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Shift(_)
            | Stmt::Input(_)
            | Stmt::Spawn { .. }
            | Stmt::Eval { .. }
            | Stmt::Close { .. }
            | Stmt::Skip => Ok((stmt, ChangeSet::default())),
            Stmt::Seq(stmts) => {
                let mut cs = ChangeSet::default();
                let stmt = Stmt::Seq(
                    stmts
                        .into_iter()
                        .map(|stmt| {
                            let (stmt, cur_cs) =
                                Self::remove_implied_guards(stmt, implied_guards.clone())?;
                            cs += cur_cs;
                            Ok(stmt)
                        })
                        .collect::<Result<_, _>>()?,
                );
                Ok((stmt, cs))
            }
            Stmt::Parallel(stmts) => {
                let mut cs = ChangeSet::default();
                let stmt = Stmt::Parallel(
                    stmts
                        .into_iter()
                        .map(|stmt| {
                            let (stmt, cur_cs) =
                                Self::remove_implied_guards(stmt, implied_guards.clone())?;
                            cs += cur_cs;
                            Ok(stmt)
                        })
                        .collect::<Result<_, _>>()?,
                );
                Ok((stmt, cs))
            }
            Stmt::Iterate { sr, stmt } => {
                let (stmt, cs) = Self::remove_implied_guards(*stmt, implied_guards)?;
                Ok((
                    Stmt::Iterate {
                        sr,
                        stmt: Box::new(stmt),
                    },
                    cs,
                ))
            }
            Stmt::Assign {
                parameter_expr,
                sr,
                stmt,
            } => {
                let (stmt, cs) = Self::remove_implied_guards(*stmt, implied_guards)?;
                Ok((
                    Stmt::Assign {
                        parameter_expr,
                        sr,
                        stmt: Box::new(stmt),
                    },
                    cs,
                ))
            }
            Stmt::If(IfStmt { guard, cons, alt }) => {
                let (new_guard, changed) = guard.remove_implied(&implied_guards);
                let mut cons_implied_guards = implied_guards.clone();
                cons_implied_guards.extend(new_guard.implied_subguards());
                let (cons, cs1) = Self::remove_implied_guards(*cons, cons_implied_guards)?;
                let (alt, cs2) = Self::remove_implied_guards(*alt, implied_guards)?;
                let stmt = Stmt::If(IfStmt {
                    guard: new_guard,
                    cons: Box::new(cons),
                    alt: Box::new(alt),
                });
                let cs = if changed {
                    cs1 + cs2 + ChangeSet::local_change()
                } else {
                    cs1 + cs2
                };
                Ok((stmt, cs))
            }
        }
    }
}

impl Guard {
    pub(crate) fn implied_subguards(&self) -> Box<dyn Iterator<Item = Guard> + '_> {
        match self {
            Guard::Stream(_)
            | Guard::Alive(_)
            | Guard::Dynamic(_)
            | Guard::GlobalFreq(_)
            | Guard::LocalFreq(_)
            | Guard::Or { .. }
            | Guard::Constant(_)
            | Guard::FastOr(_) => Box::new(iter::once(self.clone())),
            Guard::And { lhs, rhs } => {
                Box::new(lhs.implied_subguards().chain(rhs.implied_subguards()))
            }
            Guard::FastAnd(_) => panic!("FastAnd not supported for this rewrite rule"),
        }
    }

    fn remove_implied(self, guards: &Vec<Guard>) -> (Guard, bool) {
        if let Guard::Constant(true) = self {
            return (self, false);
        }
        if guards.contains(&self) {
            return (Guard::Constant(true), true);
        }
        match self {
            Guard::And { lhs, rhs } => {
                let (new_lhs, changed1) = lhs.remove_implied(guards);
                let (new_rhs, changed2) = rhs.remove_implied(guards);
                (
                    Guard::And {
                        lhs: Box::new(new_lhs),
                        rhs: Box::new(new_rhs),
                    },
                    changed1 || changed2,
                )
            }
            Guard::Or { lhs, rhs } => {
                let (new_lhs, changed1) = lhs.remove_implied(guards);
                let (new_rhs, changed2) = rhs.remove_implied(guards);
                (
                    Guard::Or {
                        lhs: Box::new(new_lhs),
                        rhs: Box::new(new_rhs),
                    },
                    changed1 || changed2,
                )
            }
            _ => (self, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{
            implied_guards::ImpliedGuards, remove_ifs::RemoveIfs, simplify_guard::SimplifyGuard,
            Rewriter,
        },
    };

    #[test]
    fn implied_simple() {
        let ir = parse_ir(
            "
			if @0 then
				if @0 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
		if @0 then
			input 0
		fi",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn implied_simple2() {
        let ir = parse_ir(
            "
			if @0 && @1 then
				if @0 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
		if @0 && @1 then
			input 0
		fi",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn implied_simple3() {
        let ir = parse_ir(
            "
			if @1 then
				if @0 && @1 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
		if @1 then
			if @0 then 
				input 0
			fi
		fi",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn or1() {
        let ir = parse_ir(
            "
			if @1 then
				if @0 || @1 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
		if @1 then
			input 0
		fi",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn or2() {
        let ir = parse_ir(
            "
			if @0 || @1 then
				if @0 then
					input 0
				fi
			fi	
		",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir.clone()).unwrap();
        assert!(!changed);
        assert!(res.stmt.eq(&res.stmt));
    }

    #[test]
    fn or3() {
        let ir = parse_ir(
            "
			if @0 || @1 then
				if @0 || @1 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
			if @0 || @1 then
				input 0
			fi	
		",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn deeper_nesting() {
        let ir = parse_ir(
            "
			if @0 then
				if @1 then
					if @0 then
						input 0
					fi
				else
					if @0 then
						input 1
					fi
				fi
			else
				if @0 then
					input 0
				fi
			fi	
		",
        );
        let reference = parse_ir(
            "
			if @0 then
				if @1 then
					input 0
				else
					input 1
				fi
			else
				if @0 then
					input 0
				fi
			fi	
		",
        );
        let rewriter = Rewriter::new(vec![
            Box::new(ImpliedGuards {}),
            Box::new(SimplifyGuard {}),
            Box::new(RemoveIfs {}),
        ]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
