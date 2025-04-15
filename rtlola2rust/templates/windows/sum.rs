#[derive(Debug, Clone)]
struct SumAggregation<BucketType>(PhantomData<BucketType>);
impl<Value: Copy + Debug + Add<Output = Value> + From<u8>> WindowAggregation for SumAggregation<Value> {
    type BucketType = Value;
    type ResultType = Value;
    type ValueType = Value;

    fn aggregate(buffer1: Self::BucketType, buffer2: Self::BucketType) -> Self::BucketType {
        buffer1 + buffer2
    }

    fn map(value: Self::ValueType, _time: Duration) -> Self::BucketType {
        value
    }

    fn lower(buffer: Self::BucketType) -> Self::ResultType {
        buffer
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        0.into()
    }
}
