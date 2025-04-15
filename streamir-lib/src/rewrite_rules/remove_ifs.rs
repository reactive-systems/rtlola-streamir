use std::collections::HashMap;

use crate::ir::{memory::Memory, Guard, IfStmt, LivetimeEquivalences, Stmt, StreamReference};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that removes if statements with a constant guard condition.
pub struct RemoveIfs;

impl RewriteRule for RemoveIfs {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::If(IfStmt {
                guard: Guard::Constant(true),
                cons,
                alt: _,
            }) => Ok((*cons, ChangeSet::local_change())),
            Stmt::If(IfStmt {
                guard: Guard::Constant(false),
                cons: _,
                alt,
            }) => Ok((*alt, ChangeSet::local_change())),
            other => Ok((other, ChangeSet::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{remove_ifs::RemoveIfs, Rewriter},
    };

    #[test]
    fn constant_true() {
        let ir = parse_ir(
            "
		if true then
			input 0
		else
			input 1
		fi	
		",
        );
        let reference = parse_ir("input 0");
        let rewriter = Rewriter::new(vec![Box::new(RemoveIfs {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn constant_false() {
        let ir = parse_ir(
            "
		if false then
			input 0
		else
			input 1
		fi	
		",
        );
        let reference = parse_ir("input 1");
        let rewriter = Rewriter::new(vec![Box::new(RemoveIfs {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }

    #[test]
    fn no_constant() {
        let ir = parse_ir(
            "
		if @0 then
			input 0
		else
			input 1
		fi	
		",
        );
        let rewriter = Rewriter::new(vec![Box::new(RemoveIfs {})]);
        let (res, changed) = rewriter.apply(ir.clone()).unwrap();
        assert!(!changed);
        assert!(res.stmt.eq(&ir.stmt));
    }
}
