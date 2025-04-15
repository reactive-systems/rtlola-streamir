use std::collections::HashMap;

use crate::{
    ir::{memory::Memory, LivetimeEquivalences, Stmt, StreamReference},
    rewrite_rules::RemoveSkip,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that simplifies sequences/parallel nested inside each other.
pub struct CombineSeq;

impl RewriteRule for CombineSeq {
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
                    .flat_map(|stmt| match stmt {
                        Stmt::Seq(inner) => inner,
                        stmt => vec![stmt],
                    })
                    .collect();
                let cs = if len == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                match inner.len() {
                    2.. => Ok((Stmt::Seq(inner), cs)),
                    1 => Ok((inner.into_iter().next().unwrap(), ChangeSet::local_change())),
                    0 => Ok((Stmt::Skip, ChangeSet::local_change())),
                }
            }
            Stmt::Parallel(inner) => {
                let len = inner.len();
                let inner: Vec<_> = inner
                    .into_iter()
                    .flat_map(|stmt| match stmt {
                        Stmt::Parallel(inner) => inner,
                        stmt => vec![stmt],
                    })
                    .collect();
                let cs = if len == inner.len() {
                    ChangeSet::default()
                } else {
                    ChangeSet::local_change()
                };
                match inner.len() {
                    2.. => Ok((Stmt::Parallel(inner), cs)),
                    1 => Ok((inner.into_iter().next().unwrap(), ChangeSet::local_change())),
                    0 => Ok((Stmt::Skip, ChangeSet::local_change())),
                }
            }
            stmt => Ok((stmt, ChangeSet::default())),
        }
    }

    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        vec![Box::new(RemoveSkip)]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ir::parse::parse_ir,
        rewrite_rules::{combine_seq::CombineSeq, Rewriter},
    };

    #[test]
    fn simple() {
        let ir = parse_ir(
            "seq {
            input 0;
            seq {
                input 1;
                seq {};
                input 2
            };
            seq { seq { input 3 }}
        }",
        );
        let reference = parse_ir(
            "seq {
            input 0;
            input 1;
            input 2;
            input 3
        }",
        );
        let rewriter = Rewriter::new(vec![Box::new(CombineSeq {})]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
