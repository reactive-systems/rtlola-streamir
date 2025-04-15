#[derive(Debug, Clone)]
struct VarianceAggregation<BucketType>(PhantomData<BucketType>);
impl<Value: Copy + Debug + Add<Output = Value> + Div<Output = Value> + Into<f64>> WindowAggregation
    for VarianceAggregation<Value>
{
    type BucketType = (usize, f64, f64);
    type ResultType = Option<f64>;
    type ValueType = Value;

    fn aggregate((count1, var1, sum1): Self::BucketType, (count2, var2, sum2): Self::BucketType) -> Self::BucketType {
        if count1 == 0 {
            return (count2, var2, sum2);
        }
        if count2 == 0 {
            return (count1, var1, sum1);
        }

        let mean_diff = (sum2 / count2 as f64) - (sum1 / count1 as f64);
        let new_var = var1 + var2 + mean_diff * mean_diff * (count1 * count2) as f64 / (count1 + count2) as f64;
        let new_count = count1 + count2;
        (new_count, new_var, sum1 + sum2)
    }

    fn map(value: Self::ValueType, _time: Duration) -> Self::BucketType {
        (1, 0.0, value.into())
    }

    fn lower((count, var, sum): Self::BucketType) -> Self::ResultType {
        if count == 0 {
            None
        } else {
            Some(var / count as f64)
        }
    }

    fn initial_value(_time: Duration) -> Self::BucketType {
        (0, 0.0, 0.0)
    }
}
