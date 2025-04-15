#[derive(Debug, Clone)]
struct DisjunctionAggregation;
impl WindowAggregation for DisjunctionAggregation {
    type BucketType = bool;
    type ResultType = bool;
    type ValueType = bool;

    fn aggregate(current: Self::BucketType, value: Self::BucketType) -> Self::BucketType {
        current || value
    }

    fn map(value: Self::ValueType, _time: Duration) -> Self::BucketType {
        value
    }

    fn lower(value: Self::BucketType) -> Self::ResultType {
        value
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        false
    }
}
