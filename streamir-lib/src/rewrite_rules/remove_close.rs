use std::collections::HashMap;

use crate::ir::{
    memory::{Memory, StreamMemory},
    LivetimeEquivalences, Stmt, StreamReference,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Copy, Clone, Debug)]
/// Remove spawn statements when the stream lives for the whole runtime of the monitor
pub struct RemoveClose;

impl RewriteRule for RemoveClose {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Close { sr, .. } => match &memory[&StreamReference::Out(sr)].buffer {
                StreamMemory::NoMemory
                | StreamMemory::Static(_)
                | StreamMemory::Dynamic {
                    buffer: _,
                    has_spawn: _,
                    has_close: false,
                } => Ok((Stmt::Skip, ChangeSet::local_change())),
                StreamMemory::Dynamic {
                    buffer: _,
                    has_spawn: _,
                    has_close: true,
                }
                | StreamMemory::Instances { .. } => Ok((stmt, ChangeSet::default())),
            },
            other => Ok((other, ChangeSet::default())),
        }
    }
}
