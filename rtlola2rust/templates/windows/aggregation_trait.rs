trait WindowAggregation {
    type BucketType: Copy;
    type ValueType: Copy;
    type ResultType: Copy;

    fn aggregate(buffer1: Self::BucketType, buffer2: Self::BucketType) -> Self::BucketType;
    fn map(value: Self::ValueType, time: Duration) -> Self::BucketType;
    fn lower(buffer: Self::BucketType) -> Self::ResultType;
    fn initial_value(time: Duration) -> Self::BucketType;
}

trait Window<AggregationFunction: WindowAggregation, const NUM_BUCKETS: usize, const WAIT: bool> {
    fn shift(&mut self, index: usize, time: Duration);
    fn accept_value(&mut self, _time: Duration, new_value: AggregationFunction::ValueType);
    fn buckets(&self) -> &[AggregationFunction::BucketType; NUM_BUCKETS];
    fn current_bucket(&self) -> usize;
    fn index(&self, ts: Duration) -> i32;
    fn start_time(&self) -> Duration;
    fn bucket_duration(&self) -> Duration;

    fn get_value(&mut self, time: Duration) -> Option<AggregationFunction::ResultType> {
        if WAIT && time < self.start_time() + (self.bucket_duration() * NUM_BUCKETS as u32) {
            return None;
        }

        let index = self.index(time) as usize;
        if index >= NUM_BUCKETS {
            Some(AggregationFunction::lower(
                AggregationFunction::initial_value(time),
            ))
        } else {
            Some(AggregationFunction::lower(
                self.buckets()
                    .iter()
                    .cycle()
                    .skip(self.current_bucket())
                    .take(NUM_BUCKETS - index)
                    .cloned()
                    .reduce(|b1, b2| AggregationFunction::aggregate(b2, b1))
                    .unwrap(),
            ))
        }
    }
}
