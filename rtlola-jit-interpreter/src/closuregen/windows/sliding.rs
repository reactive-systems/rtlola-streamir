use std::collections::VecDeque;

use crate::value::Value;
use crate::Time;

use super::aggregations::Aggregation;
use super::SlidingWindowTrait;

#[derive(Clone, Debug)]
pub(crate) struct SlidingWindow<const WAIT: bool, Inner: Aggregation> {
    buckets: VecDeque<Inner>,
    start_time: Time,
    bucket_duration: Time,
    bucket_end: Time,
}

impl<const WAIT: bool, Inner: Aggregation> SlidingWindow<WAIT, Inner> {
    pub(crate) fn new(bucket_count: usize, ts: Time, bucket_duration: Time) -> Self {
        Self {
            buckets: vec![Inner::default(ts); bucket_count].into_iter().collect(),
            start_time: ts,
            bucket_duration,
            bucket_end: ts + bucket_duration,
        }
    }
}

impl<const WAIT: bool, Inner: Aggregation> SlidingWindow<WAIT, Inner> {
    pub(crate) fn get_value(&self, ts: Time) -> Value {
        if WAIT && ts < self.start_time + (self.bucket_duration * self.buckets.len() as u32) {
            return Value::None;
        }
        let index = self.index(ts) as usize;
        if index >= self.buckets.len() {
            Inner::default(ts).lower()
        } else {
            self.buckets
                .range(0..(self.buckets.len() - index))
                .cloned()
                .reduce(|lhs, rhs| rhs + lhs)
                .unwrap()
                .lower()
        }
    }

    pub(crate) fn accept_value(&mut self, v: Value, ts: Time) {
        let index = self.index(ts);
        if index == -1 {
            if let Some(bucket) = self.buckets.get_mut(1) {
                *bucket += Inner::from_value(v, ts);
            }
            return;
        }
        self.shift(index as usize, ts);
        if let Some(inner) = self.buckets.front_mut() {
            *inner += Inner::from_value(v, ts);
        }
    }

    fn shift(&mut self, index: usize, ts: Time) {
        self.bucket_end += self.bucket_duration * (index as u32);
        if index >= self.buckets.len() {
            self.buckets
                .iter_mut()
                .for_each(|bucket| *bucket = Inner::default(ts));
            return;
        }
        self.buckets.rotate_right(index);
        self.buckets
            .range_mut(0..index)
            .for_each(|bucket| *bucket = Inner::default(ts));
    }

    fn index(&self, ts: Time) -> i32 {
        let diff = ts - (self.bucket_end - self.bucket_duration);
        let index = (diff.as_nanos() / self.bucket_duration.as_nanos()) as i32;
        if diff == self.bucket_duration * (index as u32) {
            index - 1
        } else {
            index
        }
    }
}

impl<const WAIT: bool, Inner: Aggregation> SlidingWindowTrait for SlidingWindow<WAIT, Inner> {
    fn get_value(&self, ts: Time) -> Value {
        self.get_value(ts)
    }

    fn accept_value(&mut self, v: Value, ts: Time) {
        self.accept_value(v, ts);
    }

    fn activate(&mut self, ts: Time) {
        self.buckets
            .iter_mut()
            .for_each(|bucket| *bucket = Inner::default(ts));
        self.start_time = ts;
        self.bucket_end = ts + self.bucket_duration;
    }
}
