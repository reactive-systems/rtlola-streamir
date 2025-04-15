#[derive(Clone, Debug)]
pub(crate) struct DynamicStreamBuffer<StreamType, const STREAM_SIZE: usize> {
    stream_buffer: StreamBuffer<StreamType, STREAM_SIZE>,
    alive: bool,
}

impl<StreamType, const STREAM_SIZE: usize> DynamicStreamBuffer<StreamType, STREAM_SIZE> {
    pub(crate) fn new(alive: bool) -> Self {
        Self {
            stream_buffer: StreamBuffer::new(),
            alive,
        }
    }
}

impl<StreamType, const STREAM_SIZE: usize> StreamBufferTrait<StreamType, STREAM_SIZE>
    for DynamicStreamBuffer<StreamType, STREAM_SIZE>
{
    fn stream_buffer(&self) -> &StreamBuffer<StreamType, STREAM_SIZE> {
        &self.stream_buffer
    }

    fn stream_buffer_as_mut(&mut self) -> &mut StreamBuffer<StreamType, STREAM_SIZE> {
        &mut self.stream_buffer
    }
}

impl<StreamType, const STREAM_SIZE: usize> DynamicStreamBuffer<StreamType, STREAM_SIZE> {
    pub(crate) fn is_alive(&self) -> bool {
        self.alive
    }

    pub(crate) fn spawn(&mut self) -> Result<(), MonitorError> {
        self.alive = true;
        Ok(())
    }

    pub(crate) fn close(&mut self) -> Result<(), MonitorError> {
        self.alive = false;
        self.stream_buffer = StreamBuffer::new();
        Ok(())
    }
}
