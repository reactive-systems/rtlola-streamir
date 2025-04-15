use std::collections::HashMap;

use crate::ir::{
    memory::{Memory, StreamMemory},
    LivetimeEquivalences, Stmt, StreamReference,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Copy, Clone, Debug)]
/// Remove spawn statements when the stream lives for the whole runtime of the monitor
pub struct RemoveSpawn;

impl RewriteRule for RemoveSpawn {
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        match stmt {
            Stmt::Spawn { sr, .. } => match &memory[&sr.sr()].buffer {
                StreamMemory::NoMemory
                | StreamMemory::Static(_)
                | StreamMemory::Dynamic {
                    buffer: _,
                    has_spawn: false,
                    has_close: _,
                } => Ok((Stmt::Skip, ChangeSet::local_change())),
                StreamMemory::Dynamic {
                    buffer: _,
                    has_spawn: true,
                    has_close: _,
                }
                | StreamMemory::Instances { .. } => Ok((stmt, ChangeSet::default())),
            },
            other => Ok((other, ChangeSet::default())),
        }
    }
}
