use itertools::Itertools;
use sliding::{ParameterizedSlidingWindowBuffer, SlidingWindowBuffer};
use streamir_lib::{
    formatter::{files::Requirement, names::GetStreamName},
    ir::WindowReference,
};
use tera::Context;

use crate::{
    constructs::{RequirementKey, WindowsKey},
    FunctionDefinition, RustFormatter, RustType, StructDefinition,
};

mod aggregations;
pub(crate) mod instance;
pub(crate) mod sliding;

struct AggregationTrait;

impl Requirement<RustFormatter> for AggregationTrait {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::AggregationTrait)
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.import("std::marker::PhantomData", self.file(formatter));
        formatter.import("std::fmt::Debug", self.file(formatter));
        formatter.import("std::ops::Add", self.file(formatter));
        formatter
            .tera
            .render("windows/aggregation_trait.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct WindowMemory;

impl StructDefinition for WindowMemory {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Memory)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.windows_file()
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        _f.windows_memory_struct_name()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.sliding_windows()
            .map(|w| {
                let wref = WindowReference::Sliding(w);
                (
                    f.window_name(wref),
                    if f.stream_parameter(f.wref2window[&wref].target).is_empty() {
                        RustType::Other(SlidingWindowBuffer::ty(w, f))
                    } else {
                        RustType::Other(ParameterizedSlidingWindowBuffer::ty(w, f))
                    },
                )
            })
            .collect()
    }
}

pub(crate) struct WindowMemoryConstructor;

impl FunctionDefinition for WindowMemoryConstructor {
    fn name(&self, _f: &RustFormatter) -> String {
        "new".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        f.require_struct(WindowMemory);
        let fields = f
            .sliding_windows()
            .map(|w| {
                let wref = WindowReference::Sliding(w);
                if f.stream_parameter(f.wref2window[&wref].target).is_empty() {
                    format!(
                        "{}: {}",
                        f.window_name(wref),
                        SlidingWindowBuffer::constructor(w, f, &f.start_time_argument_name())
                    )
                } else {
                    format!(
                        "{}: {}",
                        f.window_name(wref),
                        ParameterizedSlidingWindowBuffer::constructor(w, f)
                    )
                }
            })
            .join(",\n");
        format!("Self {{\n{fields}\n}}")
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![_f.start_time_argument()]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(WindowMemory.struct_name(_f))
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::MemoryConstructor)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.windows_file()
    }
}
