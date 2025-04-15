#[derive(Debug, Clone)]
struct ConjunctionAggregation;
impl WindowAggregation for ConjunctionAggregation {
    type BucketType = bool;
    type ResultType = bool;
    type ValueType = bool;

    fn aggregate(current: Self::BucketType, value: Self::BucketType) -> Self::BucketType {
        current && value
    }

    fn map(value: Self::ValueType, time: Duration) -> Self::BucketType {
        value
    }

    fn lower(value: Self::BucketType) -> Self::ResultType {
        value
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        true
    }
}
