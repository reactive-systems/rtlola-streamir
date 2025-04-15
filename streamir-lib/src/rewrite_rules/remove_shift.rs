use std::collections::HashMap;

use crate::ir::{
    memory::{Memory, StreamBuffer},
    LivetimeEquivalences, Stmt, StreamReference,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule removing shift statements if the memory is SingleValue or NoMemory.
pub struct RemoveShift;

impl RewriteRule for RemoveShift {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Shift(sr) => {
                let can_be_removed = match &memory.get(&sr).unwrap().buffer.buffer() {
                    None | Some(StreamBuffer::SingleValue) => true,
                    Some(StreamBuffer::Bounded(_) | StreamBuffer::UnBounded) => false,
                };
                if can_be_removed {
                    Ok((Stmt::Skip, ChangeSet::local_change()))
                } else {
                    Ok((Stmt::Shift(sr), ChangeSet::default()))
                }
            }
            other => Ok((other, ChangeSet::default())),
        }
    }
}
