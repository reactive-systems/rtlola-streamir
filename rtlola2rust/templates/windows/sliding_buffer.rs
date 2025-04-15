#[derive(Debug, Clone)]
struct SlidingWindowBuffer<
    AggregationFunction: WindowAggregation,
    const NUM_BUCKETS: usize,
    const WAIT: bool,
> {
    buckets: [AggregationFunction::BucketType; NUM_BUCKETS],
    current: usize,
    current_bucket_end: Duration,
    bucket_size: Duration,
    start_time: Duration,
    _phantom: PhantomData<AggregationFunction>,
}

impl<AggregationFunction: WindowAggregation, const NUM_BUCKETS: usize, const WAIT: bool>
    SlidingWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>
{
    fn new(start_time: Duration, bucket_size: Duration) -> Self {
        let buckets = std::array::from_fn(|_| AggregationFunction::initial_value(start_time));

        SlidingWindowBuffer {
            buckets,
            current: 0,
            current_bucket_end: start_time + bucket_size,
            _phantom: PhantomData,
            bucket_size,
            start_time,
        }
    }
}

impl<AggregationFunction: WindowAggregation, const NUM_BUCKETS: usize, const WAIT: bool>
    Window<AggregationFunction, NUM_BUCKETS, WAIT>
    for SlidingWindowBuffer<AggregationFunction, NUM_BUCKETS, WAIT>
{
    fn index(&self, ts: Duration) -> i32 {
        let diff = ts - (self.current_bucket_end - self.bucket_size);
        let index = (diff.as_nanos() / self.bucket_size.as_nanos()) as i32;
        if diff == self.bucket_size * (index as u32) {
            index - 1
        } else {
            index
        }
    }

    fn shift(&mut self, index: usize, time: Duration) {
        self.current_bucket_end += self.bucket_size * (index as u32);
        if index >= NUM_BUCKETS {
            self.buckets
                .iter_mut()
                .for_each(|bucket| *bucket = AggregationFunction::initial_value(time));
            return;
        }
        self.current = (self.current + NUM_BUCKETS - index) % NUM_BUCKETS;
        let (second_half, first_half) = self.buckets.split_at_mut(self.current);
        first_half
            .iter_mut()
            .chain(second_half.iter_mut())
            .take(index)
            .for_each(|bucket| *bucket = AggregationFunction::initial_value(time));
    }

    fn accept_value(&mut self, time: Duration, new_value: AggregationFunction::ValueType) {
        let index = self.index(time);
        if index == -1 {
            debug_assert_eq!(self.current, 0);
            self.buckets
                .get_mut(1)
                .map(|v| *v = AggregationFunction::map(new_value, time));
            return;
        }
        self.shift(index as usize, time);
        let cur_bucket = self.buckets.get_mut(self.current).expect("Cannot fail");
        let new_value = AggregationFunction::map(new_value, time);

        *cur_bucket = AggregationFunction::aggregate(*cur_bucket, new_value)
    }

    fn buckets(&self) -> &[AggregationFunction::BucketType; NUM_BUCKETS] {
        &self.buckets
    }

    fn current_bucket(&self) -> usize {
        self.current
    }

    fn start_time(&self) -> Duration {
        self.start_time
    }
    fn bucket_duration(&self) -> Duration {
        self.bucket_size
    }
}
