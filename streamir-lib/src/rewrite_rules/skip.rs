use std::collections::HashMap;

use crate::ir::{memory::Memory, IfStmt, LivetimeEquivalences, Stmt, StreamReference};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that removes Skip instructions.
pub struct RemoveSkip;

impl RewriteRule for RemoveSkip {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Seq(inner) => {
                let len = inner.len();
                let inner: Vec<_> = inner
                    .into_iter()
                    .filter(|stmt| !matches!(stmt, Stmt::Skip))
                    .collect();
                let cs = if len == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                if inner.is_empty() {
                    Ok((Stmt::Skip, ChangeSet::local_change()))
                } else {
                    Ok((Stmt::Seq(inner), cs))
                }
            }
            Stmt::Parallel(inner) => {
                let len = inner.len();
                let inner: Vec<_> = inner
                    .into_iter()
                    .filter(|stmt| !matches!(stmt, Stmt::Skip))
                    .collect();
                let cs = if len == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                if inner.is_empty() {
                    Ok((Stmt::Skip, ChangeSet::local_change()))
                } else {
                    Ok((Stmt::Parallel(inner), cs))
                }
            }
            Stmt::If(IfStmt {
                guard: _,
                cons,
                alt,
            }) if matches!(*cons, Stmt::Skip) && matches!(*alt, Stmt::Skip) => {
                Ok((Stmt::Skip, ChangeSet::local_change()))
            }
            Stmt::Iterate { stmt, .. } | Stmt::Assign { stmt, .. }
                if matches!(*stmt, Stmt::Skip) =>
            {
                Ok((Stmt::Skip, ChangeSet::local_change()))
            }
            other => Ok((other, ChangeSet::default())),
        }
    }
}
