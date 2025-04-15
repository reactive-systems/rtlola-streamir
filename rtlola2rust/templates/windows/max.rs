#[derive(Debug, Clone)]
struct MaxAggregation<BucketType>(PhantomData<BucketType>);
impl<Value: Copy + Debug + PartialOrd> WindowAggregation for MaxAggregation<Value> {
    type BucketType = Option<Value>;
    type ResultType = Option<Value>;
    type ValueType = Value;

    fn aggregate(value1: Self::BucketType, value2: Self::BucketType) -> Self::BucketType {
        match (value1, value2) {
            (None, v) | (v, None) => v,
            (Some(v1), Some(v2)) if v1 < v2 => Some(v2),
            (Some(v1), Some(v2)) => Some(v1),
        }
    }

    fn map(value: Self::ValueType, _time: Duration) -> Self::BucketType {
        Some(value)
    }

    fn lower(value: Self::BucketType) -> Self::ResultType {
        value
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        None
    }
}
