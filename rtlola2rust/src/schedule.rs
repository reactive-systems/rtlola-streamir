use std::time::Duration;

use itertools::Itertools;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        names::GetStreamName,
    },
    ir::memory::StreamMemory,
};

use crate::{
    constructs::{EnumDefinition, RequirementKey, ScheduleKey},
    io::{InternalEventEmpty, InternalEventStruct},
    FunctionDefinition, MonitorStruct, RustFormatter, RustType, StreamMemoryStruct,
    StructDefinition,
};

pub(crate) struct DeadlineEnum;

impl DeadlineEnum {
    pub(crate) fn dynamic_variant(
        duration: Duration,
        reference: &str,
        f: &RustFormatter,
    ) -> String {
        f.require_enum(DeadlineEnum);
        format!(
            "{}::Dynamic{}({reference})",
            Self.enum_name(f),
            f.format_duration_name(duration)
        )
    }

    pub(crate) fn static_variant(duration: Duration, f: &RustFormatter) -> String {
        f.require_enum(DeadlineEnum);
        format!(
            "{}::Static{}",
            Self.enum_name(f),
            f.format_duration_name(duration)
        )
    }
}

impl EnumDefinition for DeadlineEnum {
    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::DeadlineEnum)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        StateStruct.file(_f)
    }

    fn enum_name(&self, _f: &RustFormatter) -> String {
        _f.deadline_enum_name()
    }

    fn variants(&self, f: &RustFormatter) -> Vec<String> {
        f.require_enum(StreamReferenceEnum);
        f.static_deadlines
            .iter()
            .map(|d| format!("Static{}", f.format_duration_name(*d)))
            .chain(f.dynamic_deadlines.iter().map(|d| {
                format!(
                    "Dynamic{}({})",
                    f.format_duration_name(*d),
                    f.rust_ty(RustType::Vec(Box::new(StreamReferenceEnum.as_ty(f))))
                )
            }))
            .collect()
    }

    fn decorator(&self, _f: &RustFormatter) -> Option<String> {
        Some("#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]".into())
    }
}

pub(crate) struct StreamReferenceEnum;

impl EnumDefinition for StreamReferenceEnum {
    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::StreamReferenceEnum)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }

    fn enum_name(&self, _f: &RustFormatter) -> String {
        _f.stream_reference_name()
    }

    fn variants(&self, f: &RustFormatter) -> Vec<String> {
        f.streams()
            .map(|s| {
                let variant_name = f.stream_reference_variant(s);
                if let Some(ty) = f.parameter_ty(s) {
                    format!("{variant_name}({})", f.rust_ty(ty))
                } else {
                    variant_name
                }
            })
            .collect()
    }

    fn decorator(&self, _f: &RustFormatter) -> Option<String> {
        Some("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]".into())
    }
}

struct StateStruct;

impl StructDefinition for StateStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::StateStruct)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        _f.state_struct_name()
    }

    fn fields(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        _f.add_requirement(StateTraitImplementations);
        vec![_f.time_argument(), DeadlineEnum.as_argument(_f)]
    }

    fn decorator(&self, _f: &RustFormatter) -> Option<String> {
        Some("#[derive(Debug, Clone, PartialEq, Eq)]".into())
    }
}

struct StateNewAfterFunction;

impl FunctionDefinition for StateNewAfterFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.state_new_after_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        let deadlines = f
            .dynamic_deadlines
            .iter()
            .map(|d| (DeadlineEnum::dynamic_variant(*d, "_", f), d))
            .chain(
                f.static_deadlines
                    .iter()
                    .map(|d| (DeadlineEnum::static_variant(*d, f), d)),
            )
            .map(|(variant, dl)| {
                format!(
                    "{} => State {{
        time: time + {},
        deadline
    }}",
                    variant,
                    f.format_duration(*dl)
                )
            })
            .join(",\n");
        format!(
            "match deadline {{
            {deadlines}
        }}"
        )
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![
            ("deadline".into(), DeadlineEnum.as_ty(_f)),
            _f.time_argument(),
        ]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        _f.require_struct(StateStruct);
        Some(StateStruct.struct_name(_f))
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::StateAfter)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        StateStruct.file(_f)
    }
}

struct StateTraitImplementations;

impl Requirement<RustFormatter> for StateTraitImplementations {
    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::StateTraitImplementations)
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        StateStruct.file(formatter)
    }

    fn format(self, _formatter: &RustFormatter) -> String {
        format!(
            "impl Ord for {0} {{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {{
        other.time.cmp(&self.time)
    }}
}}

impl PartialOrd for {0} {{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {{
        Some(self.cmp(other))
    }}
}}",
            StateStruct.struct_name(_formatter)
        )
    }
}

pub(crate) struct QueueStruct;

impl StructDefinition for QueueStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueueDefinition)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        _f.queue_struct_name()
    }

    fn fields(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![(
            "heap".into(),
            RustType::Other(format!(
                "std::collections::BinaryHeap<{}>",
                StateStruct.struct_name(_f)
            )),
        )]
    }
}

pub(crate) struct QueueConstructor;

impl FunctionDefinition for QueueConstructor {
    fn name(&self, _f: &RustFormatter) -> String {
        "new".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        let init = f
            .static_deadlines
            .iter()
            .map(|dl| {
                format!(
                    "State {{\n
            time: {} + {},
            deadline: {}
        \n}}",
                    f.start_time_argument_name(),
                    f.format_duration(*dl),
                    DeadlineEnum::static_variant(*dl, f)
                )
            })
            .join(",\n");
        format!("Self{{heap: std::collections::BinaryHeap::from(vec![\n{init}\n])}}")
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueueConstructor)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![f.start_time_argument()]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(QueueStruct.struct_name(_f))
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }
}

struct QueuePopFunction;

impl FunctionDefinition for QueuePopFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.queue_pop_function_name()
    }

    fn body(self, _f: &RustFormatter) -> String {
        "self.heap.pop()".into()
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(QueueStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::Option(Box::new(StateStruct.as_ty(_f))))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueuePop)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}

struct QueuePushFunction;

impl FunctionDefinition for QueuePushFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.queue_push_function_name()
    }

    fn body(self, _f: &RustFormatter) -> String {
        "self.heap.push(state)".into()
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![("state".into(), StateStruct.as_ty(_f))]
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(QueueStruct.struct_name(_f))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueuePush)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}

struct QueueCollectAndAddFunction;

impl FunctionDefinition for QueueCollectAndAddFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.queue_collect_and_add_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        /*
                           {%- for deadline in deadlines %}{% if deadline.2 %}
                    {% if not loop.first %}| {% endif %}(Deadline::{{ deadline.1 }}(ref mut last), Deadline::{{ deadline.1 }}(deadline))
                    {%- endif %}{% endfor %} => last.extend(deadline),
                    (_, _) => {}
        k */
        let match_cases = f
            .dynamic_deadlines
            .iter()
            .map(|dl| {
                format!(
                    "({}, {}) => last.extend(deadline)",
                    DeadlineEnum::dynamic_variant(*dl, "ref mut last", f),
                    DeadlineEnum::dynamic_variant(*dl, "deadline", f)
                )
            })
            .chain(
                (!f.static_deadlines.is_empty() || (f.dynamic_deadlines.len() > 1))
                    .then(|| "(_, _) => {}".into()),
            )
            .join(",\n");
        [
            "spawned_streams.sort();",
            "let deadlines = spawned_streams.into_iter().fold(Vec::new(), |mut acc, deadline| {",
            "if let Some(last) = acc.last_mut() {",
                "match (last, deadline) {",
                    &match_cases,
                "};",
            "} else {",
                "acc.push(deadline);",
            "}",
            "acc",
            "});",
            "self.heap.extend(deadlines.into_iter().map(|deadline| State::new_after(deadline, time)))"
            ].join("\n")
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![
            (
                "mut spawned_streams".into(),
                RustType::Vec(Box::new(DeadlineEnum.as_ty(_f))),
            ),
            _f.time_argument(),
        ]
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(QueueStruct.struct_name(_f))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueueCollectAndAdd)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}

struct QueueRemoveFunction;

impl FunctionDefinition for QueueRemoveFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.queue_remove_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        let match_cases = f
            .dynamic_deadlines
            .iter()
            .map(|d| {
                format!(
                    "{} => {{
                    streams.retain(|sr| !closed_streams.contains(sr));
                    if streams.is_empty() {{
                        None
                    }} else {{
                        Some(State {{
                            time,
                            deadline: {}
                        }}) 
                    }}
    }}",
                    DeadlineEnum::dynamic_variant(*d, "mut streams", f),
                    DeadlineEnum::dynamic_variant(*d, "streams", f)
                )
            })
            .chain(
                (!f.static_deadlines.is_empty()).then(|| "_ => Some(State{time, deadline})".into()),
            )
            .join(",\n");

        [
            "if !closed_streams.is_empty() {",
            "self.heap = self",
            ".heap",
            ".drain()",
            ".filter_map(|State { time, deadline }| {",
            "match deadline {",
            &match_cases,
            "}",
            "}).collect()",
            "}",
        ]
        .join("\n")
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![(
            "closed_streams".into(),
            RustType::Vec(Box::new(StreamReferenceEnum.as_ty(_f))),
        )]
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(QueueStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueueRemove)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}

pub(crate) struct QueueNextFunction;

impl FunctionDefinition for QueueNextFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.queue_next_function_name()
    }

    fn body(self, f: &RustFormatter) -> String {
        let deadline_variants = f
            .dynamic_deadlines
            .iter()
            .map(|dl| {
                format!(
                    "{} => {{current_event.{}.extend(v)}}",
                    DeadlineEnum::dynamic_variant(*dl, "v", f),
                    f.dynamic_deadline_event_name(*dl)
                )
            })
            .chain(f.static_deadlines.iter().map(|dl| {
                format!(
                    "{} => {{current_event.{} = true;}}",
                    DeadlineEnum::static_variant(*dl, f),
                    f.static_deadline_event_name(*dl)
                )
            }))
            .join(",\n");
        vec![
            &format!(
                "let mut current: Option<{}> = None;",
                InternalEventStruct.struct_name(f)
            ),
            &format!(
                "while let Some(state) = {} {{",
                f.call_self_function::<_, String>(QueuePopFunction, &[])
            ),
            "if (!inclusive && state.time >= end) || state.time > end {",
            &format!("{};", f.call_self_function(QueuePushFunction, &["state"])),
            "return current;",
            "}",
            "if let Some(current_event) = &current {",
            &format!(
                "if state.{0} > current_event.{0} {{",
                f.time_argument_name()
            ),
            &format!("{};", f.call_self_function(QueuePushFunction, &["state"])),
            "return current;",
            "}",
            "}",
            "let State { time, deadline } = state;",
            &format!(
                "let current_event = current.get_or_insert_with(|| {});",
                f.call_function(InternalEventEmpty, &["state.time"])
            ),
            "match &deadline {",
            &deadline_variants,
            "}",
            &format!(
                "let new_state = {};",
                f.call_function(StateNewAfterFunction, &["deadline", "time"])
            ),
            &format!(
                "{};",
                f.call_self_function(QueuePushFunction, &["new_state"])
            ),
            "}",
            "current",
        ]
        .join("\n")
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![
            ("end".into(), RustType::Duration),
            ("inclusive".into(), RustType::Bool),
        ]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::Option(Box::new(InternalEventStruct.as_ty(_f))))
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        _f.require_struct(QueueStruct);
        Some(QueueStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::QueueNext)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}

pub(crate) struct ScheduleClearFunction;

impl FunctionDefinition for ScheduleClearFunction {
    fn name(&self, f: &RustFormatter) -> String {
        f.schedule_clear_function()
    }

    fn body(self, f: &RustFormatter) -> String {
        f.require_enum(StreamReferenceEnum);
        let mut fallback_required = false;
        let mut match_cases = f
            .streams()
            .filter_map(|sr| {
                let mem = f.stream_memory(sr);
                match mem {
                    StreamMemory::NoMemory | StreamMemory::Static(_) => {
                        fallback_required = true;
                        None
                    }
                    StreamMemory::Dynamic { .. } => Some(format!(
                        "StreamReference::{} => {{self.{}.{}.close()?;}}",
                        f.stream_reference_variant(sr),
                        StreamMemoryStruct.argument_name(f),
                        f.stream_name(sr)
                    )),
                    StreamMemory::Instances { .. } => Some(format!(
                        "StreamReference::{}(p) => {{self.{}.{}.close(p)?;}}",
                        f.stream_reference_variant(sr),
                        StreamMemoryStruct.argument_name(f),
                        f.stream_name(sr)
                    )),
                }
            })
            .collect::<Vec<_>>();
        if fallback_required {
            match_cases.push("_ => unreachable!()".into())
        };
        let match_cases = match_cases.join(",\n");
        let mut r = vec![
            format!(
                "let spawned: Vec<_> = std::mem::take(self.{}.as_mut());",
                f.spawned_argument_name()
            ),
            format!(
                "let closed: Vec<StreamReference> = std::mem::take(self.{}.as_mut());",
                f.closed_argument_name()
            ),
            "for &sr in closed.iter() {".into(),
            "match sr {".into(),
            match_cases,
            "}".into(),
            "}".into(),
        ];
        if !f.dynamic_deadlines.is_empty() || !f.static_deadlines.is_empty() {
            r.extend([
                format!(
                    "self.{};",
                    f.call_function(
                        QueueCollectAndAddFunction,
                        &[
                            QueueStruct.argument_name(f),
                            "spawned".into(),
                            format!("self.{}", f.time_argument_name())
                        ]
                    )
                ),
                format!(
                    "self.{};",
                    f.call_function(
                        QueueRemoveFunction,
                        &[QueueStruct.argument_name(f), "closed".into()]
                    )
                ),
            ]);
        }
        r.extend(["Ok(())".into()]);
        r.join("\n")
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::Unit.result())
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::Schedule(ScheduleKey::Clear)
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        QueueStruct.file(_f)
    }
}
