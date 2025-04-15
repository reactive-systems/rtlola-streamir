#[derive(Debug, Clone)]
struct CountAggregation<T, U>(PhantomData<T>, PhantomData<U>);

impl<Value: Copy, Result: From<u8> + Add<Output = Result> + Copy> WindowAggregation
    for CountAggregation<Value, Result>
{
    type BucketType = Result;
    type ResultType = Result;
    type ValueType = Value;

    fn aggregate(count0: Self::BucketType, count1: Self::BucketType) -> Self::BucketType {
        count0 + count1
    }

    fn map(_: Self::ValueType, _time: Duration) -> Self::BucketType {
        1.into()
    }

    fn lower(bucket: Self::BucketType) -> Self::ResultType {
        bucket
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        0.into()
    }
}
