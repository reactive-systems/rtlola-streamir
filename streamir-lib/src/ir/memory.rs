//! Contains the internal representation of the memory in the StreamIR.

use std::ops::Add;

use super::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// All memory information of a stream
pub struct Memory {
    /// The buffer of the stream
    pub buffer: StreamMemory,
    /// The value type of the stream
    pub ty: Type,
    /// The name of the stream
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The kind of a stream's memory
pub enum StreamMemory {
    /// No memory is needed for this stream
    NoMemory,
    /// Memory is needed, but only for a single instance living for the whole runtime
    Static(StreamBuffer),
    /// Memory is needed for a single instance that dynamically spawns and closes
    Dynamic {
        /// The buffer of the memory
        buffer: StreamBuffer,
        /// Whether this stream is dynamically spawned
        has_spawn: bool,
        /// Whether this stream is dynamically closed
        has_close: bool,
    },
    /// Memory is required for a stream spawning multiple instances
    Instances {
        /// The buffer of each stream's instance
        buffer: StreamBuffer,
        /// The parameter of the stream
        parameter: Vec<Parameter>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
/// The kind of buffer for a stream's instance
pub enum StreamBuffer {
    /// Only the newest value needs to be stored
    SingleValue,
    /// Only a bounded number of values needs to be stored
    Bounded(usize),
    /// All values need to be stored
    UnBounded,
}

impl StreamBuffer {
    /// Returns the Memorybound of the Streambuffer. Returns None if the Buffer is unbounded.
    pub fn bound(&self) -> Option<usize> {
        match self {
            StreamBuffer::SingleValue => Some(1),
            StreamBuffer::Bounded(b) => Some(*b),
            StreamBuffer::UnBounded => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A single parameter of a parameterized stream
pub struct Parameter {
    /// The name of the parameter
    pub name: String,
    /// The type of the parameter
    pub ty: Type,
}

impl Memory {
    /// Returns the parameters of a stream if it is parameterized (or None otherwise)
    pub fn parameters(&self) -> Option<&[Parameter]> {
        self.buffer.parameters()
    }

    /// Returns the number of parameters of a parameterized streams or 0 for unparameterized streams
    pub fn num_parameters(&self) -> usize {
        self.buffer.num_parameters()
    }
}

impl StreamMemory {
    /// Returns the parameters of a stream if it is parameterized (or None otherwise)
    pub fn parameters(&self) -> Option<&[Parameter]> {
        match self {
            StreamMemory::Instances {
                buffer: _,
                parameter,
            } => Some(parameter),
            _ => None,
        }
    }

    /// Returns the number of parameters of a parameterized streams or 0 for unparameterized streams
    pub fn num_parameters(&self) -> usize {
        match self {
            StreamMemory::Instances {
                buffer: _,
                parameter,
            } => parameter.len(),
            _ => 0,
        }
    }
}

impl StreamMemory {
    /// Returns the [StreamBuffer] of the memory. Returns [None] if no memory is required
    pub fn buffer(&self) -> Option<&StreamBuffer> {
        match self {
            StreamMemory::NoMemory => None,
            StreamMemory::Static(buffer)
            | StreamMemory::Dynamic { buffer, .. }
            | StreamMemory::Instances { buffer, .. } => Some(buffer),
        }
    }
}

impl Add for StreamMemory {
    type Output = StreamMemory;

    fn add(self, rhs: Self) -> Self::Output {
        assert!(self == rhs);
        self
    }
}
