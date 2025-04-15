#[derive(Debug, Clone)]
pub(crate) struct InstanceStreamBuffer<
    Parameter: Eq + std::hash::Hash + Clone,
    StreamType: Clone,
    const STREAM_SIZE: usize,
> {
    stream_buffer: HashMap<Parameter, StreamBuffer<StreamType, STREAM_SIZE>>,
}

impl<Parameter: Eq + std::hash::Hash + Clone, StreamType: Clone, const STREAM_SIZE: usize>
    InstanceStreamBuffer<Parameter, StreamType, STREAM_SIZE>
{
    pub(crate) fn new() -> Self {
        InstanceStreamBuffer {
            stream_buffer: HashMap::new(),
        }
    }

    pub(crate) fn get_instance(
        &self,
        parameter: &Parameter,
    ) -> Option<&StreamBuffer<StreamType, STREAM_SIZE>> {
        self.stream_buffer.get(parameter)
    }

    pub(crate) fn get_instance_mut(
        &mut self,
        parameter: &Parameter,
    ) -> Option<&mut StreamBuffer<StreamType, STREAM_SIZE>> {
        self.stream_buffer.get_mut(parameter)
    }

    pub(crate) fn is_alive(&self, parameter: &Parameter) -> bool {
        self.stream_buffer.contains_key(parameter)
    }

    pub(crate) fn spawn(&mut self, parameter: Parameter) -> Result<(), MonitorError> {
        let _ = self
            .stream_buffer
            .entry(parameter)
            .or_insert_with(StreamBuffer::new);
        Ok(())
    }

    pub(crate) fn close(&mut self, parameter: Parameter) -> Result<(), MonitorError> {
        let res = self.stream_buffer.remove(&parameter);
        assert!(res.is_some());
        Ok(())
    }

    pub(crate) fn alive_parameters(&self) -> Vec<Parameter> {
        self.stream_buffer.keys().cloned().collect()
    }

    pub(crate) fn clear_activation(&mut self) {
        for instance in self.stream_buffer.values_mut() {
            instance.clear_activation();
        }
    }

    fn fresh_instances(&self) -> Result<HashMap<Parameter, StreamType>, MonitorError> {
        self.stream_buffer
            .iter()
            .filter(|(_, s)| s.fresh)
            .map(|(p, s)| Ok((p.clone(), s.get(0)?.cloned().unwrap())))
            .collect()
    }
}
