pub(crate) trait StreamBufferTrait<StreamType, const STREAM_SIZE: usize> {
    fn stream_buffer_as_mut(&mut self) -> &mut StreamBuffer<StreamType, STREAM_SIZE>;
    fn stream_buffer(&self) -> &StreamBuffer<StreamType, STREAM_SIZE>;

    fn get(&self, offset: usize) -> Result<Option<&StreamType>, MonitorError> {
        let stream_buffer = self.stream_buffer();
        let index = (stream_buffer.current + STREAM_SIZE - offset) % STREAM_SIZE;
        stream_buffer
            .values
            .get(index)
            .map(Option::as_ref)
            .ok_or_else(|| MonitorError::OutOfBoundsAccess {
                accessed_offset: offset,
                buffer_size: STREAM_SIZE,
            })
    }

    fn update(&mut self, new_value: StreamType) -> Result<(), MonitorError> {
        let stream_buffer = self.stream_buffer_as_mut();
        let current_index: usize = stream_buffer.current;
        let value = stream_buffer.values.get_mut(current_index).ok_or_else(|| {
            MonitorError::OutOfBoundsAccess {
                accessed_offset: current_index,
                buffer_size: STREAM_SIZE,
            }
        })?;
        *value = Some(new_value);
        stream_buffer.fresh = true;
        Ok(())
    }

    fn shift(&mut self) {
        let stream_buffer = self.stream_buffer_as_mut();
        stream_buffer.current = (stream_buffer.current + 1) % STREAM_SIZE;
    }

    fn is_fresh(&self) -> bool {
        let stream_buffer = self.stream_buffer();
        stream_buffer.fresh
    }

    fn clear_activation(&mut self) {
        let stream_buffer = self.stream_buffer_as_mut();
        stream_buffer.fresh = false;
    }
}
