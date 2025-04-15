#![allow(clippy::suspicious_arithmetic_impl)]
#![allow(clippy::suspicious_op_assign_impl)]

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign};

use ordered_float::NotNan;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive, Zero};
use rust_decimal::Decimal;

use crate::value::Value;
use crate::Time;

pub(crate) trait Aggregation:
    Clone + Add<Output = Self> + AddAssign + Sized + Debug
{
    fn default(ts: Time) -> Self;

    fn from_value(v: Value, ts: Time) -> Self;

    fn lower(&self) -> Value;
}

#[derive(Debug, Clone)]
pub(crate) struct Sum<T: FromValue> {
    v: Value,
    _phantom: PhantomData<T>,
}
impl<T: FromValue> Sum<T> {
    fn new(v: Value) -> Self {
        Self {
            v,
            _phantom: PhantomData,
        }
    }
}

impl<T: FromValue> Aggregation for Sum<T> {
    fn default(_ts: Time) -> Self {
        Self::new(T::from_value(Value::Unsigned(0)))
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self::new(T::from_value(v))
    }

    fn lower(&self) -> Value {
        self.v.clone()
    }
}

impl<T: FromValue> Add for Sum<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.v + rhs.v)
    }
}

impl<T: FromValue> AddAssign for Sum<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.v += rhs.v
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Min<T: FromValue> {
    v: Option<Value>,
    _phantom: PhantomData<T>,
}

impl<T: FromValue> Aggregation for Min<T> {
    fn default(_ts: Time) -> Self {
        Self {
            v: None,
            _phantom: PhantomData,
        }
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self {
            v: Some(v),
            _phantom: PhantomData,
        }
    }

    fn lower(&self) -> Value {
        self.v.clone().unwrap_or(Value::None)
    }
}

impl<T: FromValue> Add for Min<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let v = match (self.v, rhs.v) {
            (Some(lhs), Some(rhs)) => Some(lhs.min(rhs)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };
        Self {
            v,
            _phantom: PhantomData,
        }
    }
}

impl<T: FromValue> AddAssign for Min<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.v = match (self.v.take(), rhs.v) {
            (Some(lhs), Some(rhs)) => Some(lhs.min(rhs)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Max<T: FromValue> {
    v: Option<Value>,
    _phantom: PhantomData<T>,
}

impl<T: FromValue> Aggregation for Max<T> {
    fn default(_ts: Time) -> Self {
        Self {
            v: None,
            _phantom: PhantomData,
        }
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self {
            v: Some(v),
            _phantom: PhantomData,
        }
    }

    fn lower(&self) -> Value {
        self.v.clone().unwrap_or(Value::None)
    }
}

impl<T: FromValue> Add for Max<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let v = match (self.v, rhs.v) {
            (Some(lhs), Some(rhs)) => Some(lhs.max(rhs)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };
        Self {
            v,
            _phantom: PhantomData,
        }
    }
}

impl<T: FromValue> AddAssign for Max<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.v = match (self.v.take(), rhs.v) {
            (Some(lhs), Some(rhs)) => Some(lhs.max(rhs)),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Avg<T: FromValue> {
    sum: Value,
    num: u64,
    _phantom: PhantomData<T>,
}

impl<T: FromValue> Aggregation for Avg<T> {
    fn default(_ts: Time) -> Self {
        Self {
            sum: Value::None,
            num: 0,
            _phantom: PhantomData,
        }
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self {
            sum: v,
            num: 1,
            _phantom: PhantomData,
        }
    }

    fn lower(&self) -> Value {
        match self.sum {
            Value::None => Value::None,
            Value::Unsigned(v) => Value::Unsigned(v / self.num),
            Value::Signed(v) => Value::Signed(v / self.num as i64),
            Value::Float(v) => Value::Float(NotNan::new(v / self.num as f64).unwrap()),
            Value::Str(_) | Value::Bool(_) | Value::Tuple(_) => todo!(),
        }
    }
}

impl<T: FromValue> Add for Avg<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (&self.sum, &rhs.sum) {
            (Value::None, Value::None) => self,
            (_, Value::None) => self,
            (Value::None, _) => rhs,
            (_, _) => Self {
                sum: self.sum + rhs.sum,
                num: self.num + rhs.num,
                _phantom: PhantomData,
            },
        }
    }
}

impl<T: FromValue> AddAssign for Avg<T> {
    fn add_assign(&mut self, rhs: Self) {
        match (&self.sum, &rhs.sum) {
            (Value::None, Value::None) | (_, Value::None) => {}
            (Value::None, _) => *self = rhs,
            (_, _) => {
                self.sum += rhs.sum;
                self.num += rhs.num;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Count(usize);

impl Aggregation for Count {
    fn default(_ts: Time) -> Self {
        Count(0)
    }

    fn from_value(_v: Value, _ts: Time) -> Self {
        Count(1)
    }

    fn lower(&self) -> Value {
        Value::Unsigned(self.0 as u64)
    }
}

impl Add for Count {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Count(self.0 + rhs.0)
    }
}

impl AddAssign for Count {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Conjunction(Value);

impl Aggregation for Conjunction {
    fn default(_ts: Time) -> Self {
        Self(Value::Bool(true))
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self(v)
    }

    fn lower(&self) -> Value {
        self.0.clone()
    }
}

impl Add for Conjunction {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl AddAssign for Conjunction {
    fn add_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Disjunction(Value);

impl Aggregation for Disjunction {
    fn default(_ts: Time) -> Self {
        Self(Value::Bool(false))
    }

    fn from_value(v: Value, _ts: Time) -> Self {
        Self(v)
    }

    fn lower(&self) -> Value {
        self.0.clone()
    }
}

impl Add for Disjunction {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl AddAssign for Disjunction {
    fn add_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Integral<T: FromValue> {
    volume: Decimal,
    end_value: Decimal,
    end_time: Time,
    start_value: Decimal,
    start_time: Time,
    valid: bool,
    _phantom: PhantomData<T>,
}

impl<T: FromValue> Aggregation for Integral<T> {
    fn default(ts: Time) -> Self {
        Self {
            volume: Decimal::zero(),
            end_value: Decimal::zero(),
            end_time: ts,
            start_value: Decimal::zero(),
            start_time: ts,
            valid: false,
            _phantom: PhantomData,
        }
    }

    fn from_value(v: Value, ts: Time) -> Self {
        let f = match v {
            Value::Signed(i) => Decimal::from(i),
            Value::Unsigned(u) => Decimal::from(u),
            Value::Float(f) => Decimal::from_f64(f.into()).unwrap(),
            _ => unreachable!("Type error."),
        };
        Self {
            volume: Decimal::zero(),
            end_value: f,
            end_time: ts,
            start_value: f,
            start_time: ts,
            valid: true,
            _phantom: PhantomData,
        }
    }

    fn lower(&self) -> Value {
        let f = self.volume.to_f64().unwrap();
        T::from_value(Value::Float(NotNan::new(f).unwrap()))
    }
}

impl<T: FromValue> Add for Integral<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if !rhs.valid {
            return self;
        }
        if !self.valid {
            return rhs;
        }
        assert!(
            rhs.start_time >= self.end_time,
            "Time does not behave monotonically!"
        );
        let time_diff_dur = rhs.start_time - self.end_time;
        let time_diff = (Decimal::from(time_diff_dur.as_secs()))
            + (Decimal::from(time_diff_dur.subsec_nanos()) / Decimal::from(100_000_000));
        let value_sum = rhs.start_value + self.end_value;

        let additional_volume = value_sum * time_diff / Decimal::from(2);
        Self {
            volume: self.volume + additional_volume + rhs.volume,
            end_value: rhs.end_value,
            end_time: rhs.end_time,
            start_value: self.start_value,
            start_time: self.start_time,
            valid: true,
            _phantom: PhantomData,
        }
    }
}

impl<T: FromValue> AddAssign for Integral<T> {
    fn add_assign(&mut self, rhs: Self) {
        if !rhs.valid {
            return;
        }
        if !self.valid {
            *self = rhs;
            return;
        }
        assert!(
            rhs.start_time >= self.end_time,
            "Time does not behave monotonically!"
        );
        let time_diff_dur = rhs.start_time - self.end_time;
        let time_diff = (Decimal::from(time_diff_dur.as_secs()))
            + (Decimal::from(time_diff_dur.subsec_nanos()) / Decimal::from(100_000_000));
        let value_sum = rhs.start_value + self.end_value;

        let additional_volume = value_sum * time_diff / Decimal::from(2);

        self.volume += additional_volume + rhs.volume;
        self.end_value = rhs.end_value;
        self.end_time = rhs.end_time;
    }
}

pub(crate) trait FromValue: Debug + Clone + Copy {
    fn from_value(v: Value) -> Value;
}

#[derive(Debug, Clone, Copy)]
pub struct Unsigned {}
impl FromValue for Unsigned {
    fn from_value(v: Value) -> Value {
        match v {
            Value::Bool(true) => Value::Unsigned(1),
            Value::Bool(false) => Value::Unsigned(0),
            Value::Unsigned(_) => v,
            Value::Signed(s) => Value::Unsigned(s as u64),
            Value::Float(_) | Value::Str(_) | Value::Tuple(_) | Value::None => {
                unreachable!("{}", v)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Signed {}
impl FromValue for Signed {
    fn from_value(v: Value) -> Value {
        match v {
            Value::Bool(true) => Value::Signed(1),
            Value::Bool(false) => Value::Signed(0),
            Value::Unsigned(v) => Value::Signed(v as i64),
            Value::Signed(_) => v,
            Value::Float(_) | Value::Str(_) | Value::Tuple(_) | Value::None => {
                unreachable!("{}", v)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Float {}
impl FromValue for Float {
    fn from_value(v: Value) -> Value {
        let f = match v {
            Value::Signed(i) => i as f64,
            Value::Unsigned(u) => u as f64,
            Value::Float(f) => f.into(),
            _ => unreachable!("Type error."),
        };
        Value::Float(NotNan::new(f).unwrap())
    }
}
