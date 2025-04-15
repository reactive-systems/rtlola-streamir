use streamir_lib::{
    formatter::files::{FilesFormatter, Requirement},
    ir::{
        windows::{WindowKind, WindowOperation},
        WindowReference,
    },
};
use tera::Context;

use crate::{
    constructs::{RequirementKey, WindowsKey},
    windows::aggregations::{
        AverageAggregation, ConjunctionAggregation, CountAggregation, DisjunctionAggregation,
        IntegralAggregation, MaxAggregation, MinAggregation, VarianceAggregation,
    },
    RustFormatter,
};

use super::aggregations::SumAggregation;

pub(crate) struct SlidingWindowBuffer;

impl RustFormatter {
    pub(crate) fn window_aggregation(&self, w: WindowReference) -> String {
        let window = &self.wref2window[&w];
        match &window.op {
            WindowOperation::Sum => SumAggregation::ty(self, window.ty.clone()),
            WindowOperation::Average => AverageAggregation::ty(self, window.ty.clone()),
            WindowOperation::Count => CountAggregation::ty(
                self,
                self.lola_stream_type(window.target).clone(),
                window.ty.clone(),
            ),
            WindowOperation::Min => MinAggregation::ty(self, window.ty.clone()),
            WindowOperation::Max => MaxAggregation::ty(self, window.ty.clone()),
            WindowOperation::Conjunction => ConjunctionAggregation::ty(self),
            WindowOperation::Disjunction => DisjunctionAggregation::ty(self),
            WindowOperation::Integral => IntegralAggregation::ty(
                self,
                self.lola_stream_type(window.target).clone(),
                window.ty.clone(),
            ),
            WindowOperation::Variance => VarianceAggregation::ty(self, window.ty.clone()),
            WindowOperation::Product
            | WindowOperation::Last
            | WindowOperation::Covariance
            | WindowOperation::StandardDeviation
            | WindowOperation::NthPercentile(_) => unimplemented!("unsupported window operation"),
        }
    }
}

impl SlidingWindowBuffer {
    pub(crate) fn ty(w: usize, f: &RustFormatter) -> String {
        f.add_requirement(Self);
        let w = WindowReference::Sliding(w);
        let aggregation = f.window_aggregation(w);
        let WindowKind::Sliding {
            duration: _,
            bucket_count,
            bucket_duration: _,
            wait,
        } = &f.wref2window[&w].kind
        else {
            unreachable!("is sliding window")
        };
        format!(
            "SlidingWindowBuffer<{}, {}, {}>",
            aggregation, bucket_count, wait
        )
    }

    pub(crate) fn constructor(w: usize, f: &RustFormatter, time: &str) -> String {
        let window = &f.wref2window[&WindowReference::Sliding(w)];
        let WindowKind::Sliding {
            duration: _,
            bucket_count: _,
            bucket_duration,
            wait: _,
        } = &window.kind
        else {
            unreachable!("is sliding window")
        };
        format!(
            "SlidingWindowBuffer::new({time}, {})",
            f.format_duration(*bucket_duration)
        )
    }
}

impl Requirement<RustFormatter> for SlidingWindowBuffer {
    fn key(&self) -> <RustFormatter as FilesFormatter>::Key {
        RequirementKey::Windows(WindowsKey::SlidingBuffer)
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter
            .tera
            .render("windows/sliding_buffer.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct ParameterizedSlidingWindowBuffer;

impl ParameterizedSlidingWindowBuffer {
    pub(crate) fn ty(w: usize, f: &RustFormatter) -> String {
        f.add_requirement(Self);
        let wref = WindowReference::Sliding(w);
        let window = &f.wref2window[&wref];
        let WindowKind::Sliding {
            duration: _,
            bucket_count,
            bucket_duration: _,
            wait,
        } = &window.kind
        else {
            unreachable!("is sliding window")
        };
        let aggregation = f.window_aggregation(wref);
        let parameters = f.parameter_ty(window.target).unwrap();
        format!(
            "ParameterizedSlidingWindowBuffer<{}, {}, {}, {}>",
            aggregation,
            f.rust_ty(parameters),
            bucket_count,
            wait
        )
    }

    pub(crate) fn constructor(w: usize, f: &RustFormatter) -> String {
        let window = &f.wref2window[&WindowReference::Sliding(w)];
        let WindowKind::Sliding {
            duration: _,
            bucket_count: _,
            bucket_duration,
            wait: _,
        } = &window.kind
        else {
            unreachable!("is sliding window")
        };
        format!(
            "ParameterizedSlidingWindowBuffer::new({})",
            f.format_duration(*bucket_duration)
        )
    }
}

impl Requirement<RustFormatter> for ParameterizedSlidingWindowBuffer {
    fn key(&self) -> <RustFormatter as FilesFormatter>::Key {
        RequirementKey::Windows(WindowsKey::ParameterizedSlidingBuffer)
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(SlidingWindowBuffer);
        formatter.import("std::hash::Hash", self.file(formatter));
        formatter
            .tera
            .render("windows/parameterized_sliding_buffer.rs", &Context::new())
            .unwrap()
    }
}
