use itertools::Itertools;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        names::GetStreamName,
    },
    ir::StreamReference,
};
use tera::Context;

use crate::{
    constructs::{RequirementKey, RustType, StructDefinition},
    FunctionDefinition, RustFormatter,
};

pub(crate) struct StreamMemoryStruct;

impl StructDefinition for StreamMemoryStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::StreamMemory
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        "StreamMemory".to_string()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.streams()
            .map(|sr| {
                let m = f.stream_memory(sr);
                let ty = f.stream_type(sr);
                let ty = match m {
                    streamir_lib::ir::memory::StreamMemory::NoMemory => {
                        f.add_requirement(StaticStreamBuffer);
                        StaticStreamBuffer::ty(ty, 1, f)
                    }
                    streamir_lib::ir::memory::StreamMemory::Static(buffer) => {
                        f.add_requirement(StaticStreamBuffer);
                        StaticStreamBuffer::ty(
                            ty,
                            buffer.bound().expect("ensured by type checker"),
                            f,
                        )
                    }
                    streamir_lib::ir::memory::StreamMemory::Dynamic {
                        buffer,
                        has_spawn: _,
                        has_close: _,
                    } => {
                        f.add_requirement(DynamicStreamBuffer);
                        DynamicStreamBuffer::ty(
                            ty,
                            buffer.bound().expect("ensured by type checker"),
                            f,
                        )
                    }
                    streamir_lib::ir::memory::StreamMemory::Instances { buffer, .. } => {
                        f.add_requirement(InstanceStreamBuffer);
                        InstanceStreamBuffer::ty(
                            sr,
                            ty,
                            buffer.bound().expect("ensured by type checker"),
                            f,
                        )
                    }
                };
                (f.stream_name(sr), RustType::Other(ty))
            })
            .collect()
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.memory_file()
    }
}

pub(crate) struct StreamMemoryConstructor;

impl FunctionDefinition for StreamMemoryConstructor {
    fn name(&self, _f: &RustFormatter) -> String {
        "new".into()
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(StreamMemoryStruct.struct_name(f))
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![f.start_time_argument()]
    }

    fn body(self, f: &RustFormatter) -> String {
        let fields = f
            .streams()
            .map(|sr| {
                let m = f.stream_memory(sr);
                let constructor = match m {
                    streamir_lib::ir::memory::StreamMemory::NoMemory => {
                        StaticStreamBuffer::constructor()
                    }
                    streamir_lib::ir::memory::StreamMemory::Static(_) => {
                        f.add_requirement(StaticStreamBuffer);
                        StaticStreamBuffer::constructor()
                    }
                    streamir_lib::ir::memory::StreamMemory::Dynamic {
                        buffer: _,
                        has_spawn,
                        has_close: _,
                    } => {
                        f.add_requirement(DynamicStreamBuffer);
                        DynamicStreamBuffer::constructor(*has_spawn)
                    }
                    streamir_lib::ir::memory::StreamMemory::Instances { .. } => {
                        f.add_requirement(InstanceStreamBuffer);
                        InstanceStreamBuffer::constructor()
                    }
                };
                format!("{}: {}", f.stream_name(sr), constructor)
            })
            .join(",\n");
        format!("Self {{\n{fields}\n}}")
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::StreamMemoryConstructor
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.memory_file()
    }
}

struct StreamBufferTrait;

impl Requirement<RustFormatter> for StreamBufferTrait {
    fn key(&self) -> <RustFormatter as FilesFormatter>::Key {
        RequirementKey::StreamBufferTrait
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter
            .tera
            .render("stream_buffer/trait.rs", &Context::new())
            .unwrap()
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.memory_file()
    }
}

pub(crate) struct StaticStreamBuffer;

impl StaticStreamBuffer {
    fn ty(ty: RustType, len: usize, f: &RustFormatter) -> String {
        format!("StreamBuffer<{}, {len}>", f.rust_ty(ty))
    }

    fn constructor() -> String {
        "StreamBuffer::new()".to_string()
    }
}

impl Requirement<RustFormatter> for StaticStreamBuffer {
    fn key(&self) -> RequirementKey {
        RequirementKey::StaticStreamBuffer
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(StreamBufferTrait);
        formatter
            .tera
            .render("stream_buffer/static.rs", &Context::new())
            .unwrap()
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.memory_file()
    }
}

pub(crate) struct DynamicStreamBuffer;

impl DynamicStreamBuffer {
    fn ty(ty: RustType, len: usize, f: &RustFormatter) -> String {
        format!("DynamicStreamBuffer<{}, {len}>", f.rust_ty(ty))
    }

    fn constructor(has_spawn: bool) -> String {
        format!("DynamicStreamBuffer::new({})", !has_spawn)
    }
}

impl Requirement<RustFormatter> for DynamicStreamBuffer {
    fn key(&self) -> RequirementKey {
        RequirementKey::DynamicStreamBuffer
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(StreamBufferTrait);
        formatter
            .tera
            .render("stream_buffer/dynamic.rs", &Context::new())
            .unwrap()
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.memory_file()
    }
}

pub(crate) struct InstanceStreamBuffer;

impl InstanceStreamBuffer {
    fn ty(sr: StreamReference, ty: RustType, len: usize, f: &RustFormatter) -> String {
        let parameter = f.parameter_ty(sr).unwrap();
        format!(
            "InstanceStreamBuffer<{}, {}, {len}>",
            f.rust_ty(parameter),
            f.rust_ty(ty),
        )
    }

    fn constructor() -> String {
        "InstanceStreamBuffer::new()".to_string()
    }
}

impl Requirement<RustFormatter> for InstanceStreamBuffer {
    fn key(&self) -> RequirementKey {
        RequirementKey::InstanceStreamBuffer
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(StreamBufferTrait);
        formatter.import("std::collections::HashMap", self.file(formatter));
        formatter
            .tera
            .render("stream_buffer/instance.rs", &Context::new())
            .unwrap()
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.memory_file()
    }
}
