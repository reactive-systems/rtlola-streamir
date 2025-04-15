use crate::{
    ir::{Guard, IfStmt, LivetimeEquivalences, Stmt},
    rewrite_rules::{CombineSeq, RemoveSkip},
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// Combines the guards of nested if statments to a single if statment with a conjunction of the guards.
pub struct CombineNestedIf;

impl RewriteRule for CombineNestedIf {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &std::collections::HashMap<crate::ir::StreamReference, crate::ir::memory::Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::If(IfStmt {
                guard: outer_guard,
                cons,
                alt: outer_alt,
            }) => match *cons {
                Stmt::If(IfStmt {
                    guard: inner_guard,
                    cons,
                    alt,
                }) if matches!(*alt, Stmt::Skip) => Ok((
                    Stmt::If(IfStmt {
                        guard: Guard::And {
                            lhs: Box::new(outer_guard),
                            rhs: Box::new(inner_guard),
                        },
                        cons,
                        alt: outer_alt,
                    }),
                    ChangeSet::local_change(),
                )),
                other => Ok((
                    Stmt::If(IfStmt {
                        guard: outer_guard,
                        cons: Box::new(other),
                        alt: outer_alt,
                    }),
                    ChangeSet::default(),
                )),
            },
            other => Ok((other, ChangeSet::default())),
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
        rewrite_rules::{nested_ifs::CombineNestedIf, Rewriter},
    };

    #[test]
    fn combined_ifs() {
        let ir = parse_ir(
            "
		if @0 then
			if @1 then
				input 0
			fi
		else
			input 1
		fi	
		",
        );
        let reference = parse_ir(
            "
		if @0 && @1 then
			input 0
		else
			input 1
		fi",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineNestedIf {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn inside_with_else() {
        let ir = parse_ir(
            "
		if @0 then
			if @1 then
				input 0
			else
				input 1
			fi
		fi	
		",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineNestedIf {})]);
        let (res, changed) = rewriter.apply(ir.clone()).unwrap();
        assert!(!changed);
        assert!(res.stmt.eq(&ir.stmt));
    }
}
