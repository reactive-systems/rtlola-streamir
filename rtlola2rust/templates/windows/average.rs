#[derive(Debug, Clone)]
struct AvgAggregation<BucketType>(PhantomData<BucketType>);
impl<Value: Copy + Debug + Add<Output = Value> + Div<Output = Value> + TryFrom<f64>> WindowAggregation
    for AvgAggregation<Value>
{
    type BucketType = (Value, usize);
    type ResultType = Option<Value>;
    type ValueType = Value;

    fn aggregate((sum0, count0): Self::BucketType, (sum1, count1): Self::BucketType) -> Self::BucketType {
        (sum0 + sum1, count0 + count1)
    }

    fn map(value: Self::ValueType, _time: Duration) -> Self::BucketType {
        (value, 1)
    }

    fn lower((sum, count): Self::BucketType) -> Self::ResultType {
        (count > 0).then(|| {
            sum / f64::try_from(count as i32)
                .unwrap()
                .try_into()
                .map_err(|_| todo!())
                .unwrap()
        })
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        (0f64.try_into().map_err(|_| todo!()).unwrap(), 0)
    }
}
