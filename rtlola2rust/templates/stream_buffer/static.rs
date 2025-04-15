#[derive(Debug, Clone)]
pub(crate) struct StreamBuffer<StreamType, const STREAM_SIZE: usize> {
    values: [Option<StreamType>; STREAM_SIZE],
    current: usize,
    fresh: bool,
}

impl<StreamType, const STREAM_SIZE: usize> StreamBuffer<StreamType, STREAM_SIZE> {
    pub(crate) fn new() -> Self {
        let values = std::array::from_fn(|_| None);
        Self {
            values,
            current: 0,
            fresh: false,
        }
    }
}

impl<StreamType, const STREAM_SIZE: usize> StreamBufferTrait<StreamType, STREAM_SIZE>
    for StreamBuffer<StreamType, STREAM_SIZE>
{
    fn stream_buffer(&self) -> &StreamBuffer<StreamType, STREAM_SIZE> {
        self
    }

    fn stream_buffer_as_mut(&mut self) -> &mut StreamBuffer<StreamType, STREAM_SIZE> {
        self
    }
}
