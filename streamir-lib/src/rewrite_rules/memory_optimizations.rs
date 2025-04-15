use crate::ir::{
    memory::{StreamBuffer, StreamMemory},
    StreamReference,
};

use super::{ChangeSet, RewriteError, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// Simplifies the memory by using special variants for stream with specific memory bounds.
pub struct MemoryOptimizations;

impl RewriteRule for MemoryOptimizations {
    fn rewrite_memory(
        &self,
        _sr: StreamReference,
        memory: StreamMemory,
    ) -> Result<(StreamMemory, ChangeSet), RewriteError> {
        match memory {
            StreamMemory::Dynamic {
                buffer,
                has_spawn: false,
                has_close: false,
            } => Ok((StreamMemory::Static(buffer), ChangeSet::local_change())),
            StreamMemory::Static(StreamBuffer::Bounded(0)) => {
                Ok((StreamMemory::NoMemory, ChangeSet::local_change()))
            }
            StreamMemory::Static(other) => Ok((StreamMemory::Static(other), ChangeSet::default())),
            StreamMemory::Dynamic {
                buffer: StreamBuffer::Bounded(0),
                has_spawn,
                has_close,
            } if has_spawn || has_close => Ok((
                StreamMemory::Dynamic {
                    buffer: StreamBuffer::SingleValue,
                    has_spawn,
                    has_close,
                },
                ChangeSet::local_change(),
            )),
            StreamMemory::Instances {
                buffer: StreamBuffer::Bounded(0),
                parameter,
            } => Ok((
                StreamMemory::Instances {
                    buffer: StreamBuffer::SingleValue,
                    parameter,
                },
                ChangeSet::local_change(),
            )),
            other => Ok((other, ChangeSet::default())),
        }
    }

    fn rewrite_buffer(
        &self,
        _sr: StreamReference,
        buffer: StreamBuffer,
    ) -> Result<(StreamBuffer, ChangeSet), RewriteError> {
        match buffer {
            StreamBuffer::Bounded(1) => Ok((StreamBuffer::SingleValue, ChangeSet::local_change())),
            other => Ok((other, ChangeSet::default())),
        }
    }
}
