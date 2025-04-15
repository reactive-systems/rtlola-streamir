#[derive(Debug, Clone, Copy)]
struct IntegralIv<T> {
    volume: T,
    end_value: T,
    end_time: Duration,
    start_value: T,
    start_time: Duration,
    valid: bool,
}

#[derive(Debug, Clone)]
struct IntegralAggregation<BucketType, ResultTy>(PhantomData<BucketType>, PhantomData<ResultTy>);
impl<
        Value: Copy + Debug + Add + Div<Output = Value> + Into<ResultTy>,
        ResultTy: Copy + Add<Output = ResultTy> + Div<Output = ResultTy> + Mul<Output = ResultTy> + From<f64>,
    > WindowAggregation for IntegralAggregation<Value, ResultTy>
{
    type BucketType = IntegralIv<ResultTy>;
    type ResultType = ResultTy;
    type ValueType = Value;

    fn aggregate(bucket1: Self::BucketType, bucket2: Self::BucketType) -> Self::BucketType {
        // from rtlola interpreter
        match (bucket1.valid, bucket2.valid) {
            (false, false) => return bucket1,
            (false, true) => return bucket2,
            (true, false) => return bucket1,
            (true, true) => {},
        }

        let start_volume = bucket1.volume + bucket2.volume;
        assert!(
            bucket2.start_time >= bucket1.end_time,
            "Time does not behave monotonically!"
        );
        let time_diff_dur = bucket2.start_time - bucket1.end_time;
        let time_diff = (time_diff_dur.as_secs() as f64) + (time_diff_dur.subsec_nanos() as f64 / 100_000_000 as f64);
        let value_sum = bucket2.start_value + bucket1.end_value;

        let additional_volume = value_sum * ResultTy::from(time_diff) / 2.0.into();

        let volume = start_volume + additional_volume;
        let end_value = bucket2.end_value;
        let end_time = bucket2.end_time;
        let start_value = bucket1.start_value;
        let start_time = bucket1.start_time;

        IntegralIv {
            volume,
            end_value,
            end_time,
            start_value,
            start_time,
            valid: true,
        }
    }

    fn map(value: Self::ValueType, time: Duration) -> Self::BucketType {
        IntegralIv {
            volume: 0.0.into(),
            end_value: value.into(),
            end_time: time,
            start_value: value.into(),
            start_time: time,
            valid: true,
        }
    }

    fn lower(iv: Self::BucketType) -> Self::ResultType {
        iv.volume
    }

    fn initial_value(time: Duration) -> Self::BucketType {
        IntegralIv {
            volume: 0.0.into(),
            end_value: 0.0.into(),
            end_time: time,
            start_value: 0.0.into(),
            start_time: time,
            valid: false,
        }
    }
}
