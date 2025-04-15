#[derive(Debug, Clone)]
struct DiscreteWindowBuffer<AggregationFunction: WindowAggregation, const NUM_VALUES: usize, const WAIT: bool> {
    values: [AggregationFunction::BucketType; NUM_VALUES],
    current: usize,
    num_values: usize,
    _phantom: PhantomData<AggregationFunction>,
}

impl<AggregationFunction: WindowAggregation, const NUM_BUCKETS: usize, const WAIT: bool>
    DiscreteWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>
{
    fn new(start_time: Duration) -> Self {
        let values = std::array::from_fn(|_| AggregationFunction::initial_value(start_time));

        DiscreteWindowBuffer {
            values,
            current: NUM_BUCKETS - 1,
            _phantom: PhantomData,
            num_values: 0,
        }
    }
}

impl<AggregationFunction: WindowAggregation, const NUM_BUCKETS: usize, const WAIT: bool>
    Window<AggregationFunction, NUM_BUCKETS, WAIT> for DiscreteWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>
{
    fn shift(&mut self, _time: Duration) {
        // a discrete window does not change buckets depending on the current time
    }

    fn update(&mut self, time: Duration, new_value: AggregationFunction::ValueType) {
        // but rather an every new value
        self.current = (self.current + 1) % NUM_BUCKETS;
        self.num_values += 1;
        let cur_bucket = self.values.get_mut(self.current).expect("Cannot fail");
        *cur_bucket = AggregationFunction::map(new_value, time);
    }

    fn buckets(&self) -> &[AggregationFunction::BucketType; NUM_BUCKETS] {
        &self.values
    }

    fn num_buckets(&self) -> usize {
        self.num_values
    }

    fn current_bucket(&self) -> usize {
        self.current
    }
}
