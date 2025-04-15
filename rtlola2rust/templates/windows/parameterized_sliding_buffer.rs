struct ParameterizedSlidingWindowBuffer<
    AggregationFunction: WindowAggregation,
    Parameter: Hash + Eq,
    const NUM_BUCKETS: usize,
    const WAIT: bool,
> {
    windows: HashMap<Parameter, SlidingWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>>,
    bucket_size: Duration,
}

impl<
        AggregationFunction: WindowAggregation,
        Parameter: Hash + Eq,
        const NUM_BUCKETS: usize,
        const WAIT: bool,
    > ParameterizedSlidingWindowBuffer<AggregationFunction, Parameter, NUM_BUCKETS, WAIT>
{
    fn get_window(
        &mut self,
        parameter: &Parameter,
    ) -> Option<&mut SlidingWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>> {
        self.windows.get_mut(parameter)
    }

    fn new(bucket_size: Duration) -> Self {
        Self {
            windows: HashMap::new(),
            bucket_size,
        }
    }

    fn spawn_window(&mut self, parameter: Parameter, start_time: Duration) {
        self.windows.insert(
            parameter,
            SlidingWindowBuffer::new(start_time, self.bucket_size),
        );
    }
}
