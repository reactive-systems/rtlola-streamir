use itertools::Itertools;

use crate::{
    constructs::{FunctionDefinition, FunctionVisibility, RequirementKey},
    io::{ExternalEventStruct, InternalEventFromExternal, VerdictStruct},
    memory::StreamMemoryConstructor,
    schedule::{QueueConstructor, QueueNextFunction},
    windows::WindowMemoryConstructor,
    MonitorStruct, QueueStruct, RustFormatter, RustType, StreamMemoryStruct, StructDefinition,
    WindowMemory,
};

pub(crate) fn verdicts_result_ty(f: &RustFormatter) -> RustType {
    RustType::Other(format!("Vec<{}>", VerdictStruct.struct_name(f))).result()
}

pub(crate) struct AcceptEventFunction;

impl FunctionDefinition for AcceptEventFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        "accept_event".into()
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(f))
    }

    fn body(self, f: &RustFormatter) -> String {
        f.call_self_function::<_, String>(CloseMonitorFunction, &[]);
        [
            &format!(
                "let event = {};",
                f.call_function(
                    InternalEventFromExternal,
                    &[f.time_argument_name(), ExternalEventStruct.argument_name(f),]
                )
            ),
            &format!(
                "let mut verdicts = {}?;",
                f.call_self_function(AcceptTimeFunction, &[f.time_argument_name()])
            ),
            &format!("verdicts.push(self.{}(event)?);", f.cycle_function_name()),
            "Ok(verdicts)",
        ]
        .join("\n")
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.require_struct(ExternalEventStruct);
        vec![ExternalEventStruct.as_argument(f), f.time_argument()]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(verdicts_result_ty(_f))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::AcceptEventFunction
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

pub(crate) struct AcceptTimeFunction;

impl FunctionDefinition for AcceptTimeFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        "accept_time".into()
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(f))
    }

    fn body(self, f: &RustFormatter) -> String {
        if f.dynamic_deadlines.is_empty() && f.static_deadlines.is_empty() {
            return "Ok(Vec::new())".into();
        }

        [
            "let mut verdicts = Vec::new();",
            &format!(
                "while let Some(timed_event) = {} {{",
                f.call_function(
                    QueueNextFunction,
                    &["self.queue".into(), f.time_argument_name(), "false".into()]
                )
            ),
            &format!(
                "let verdict = self.{}(timed_event)?;",
                f.cycle_function_name()
            ),
            "verdicts.push(verdict)",
            "}",
            "return Ok(verdicts);",
        ]
        .join("\n")
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.require_struct(ExternalEventStruct);
        vec![f.time_argument()]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(verdicts_result_ty(_f))
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::AcceptTimeFunction
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

pub(crate) struct MonitorConstructor;

impl FunctionDefinition for MonitorConstructor {
    fn name(&self, _f: &RustFormatter) -> String {
        "new".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        let fields = [
            Some((
                StreamMemoryStruct.argument_name(f),
                f.call_function(StreamMemoryConstructor, &[f.start_time_argument_name()]),
            )),
            (!(f.dynamic_deadlines.is_empty() && f.static_deadlines.is_empty())).then(|| {
                (
                    QueueStruct.argument_name(f),
                    f.call_function(QueueConstructor, &[f.start_time_argument_name()]),
                )
            }),
            (!f.wref2window.is_empty()).then(|| {
                (
                    WindowMemory.argument_name(f),
                    f.call_function(WindowMemoryConstructor, &[f.start_time_argument_name()]),
                )
            }),
            Some((f.time_argument_name(), f.start_time_argument_name())),
            Some((f.spawned_argument_name(), "Vec::new()".into())),
            Some((f.closed_argument_name(), "Vec::new()".into())),
        ]
        .into_iter()
        .flatten()
        .map(|(name, init)| format!("{name}: {init}"))
        .join(",\n");
        format!(
            "Self {{
            {fields}
	}}",
        )
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.import("std::time::Duration", self.file(f));
        vec![f.start_time_argument()]
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::MonitorConstructor
    }

    fn method_of(&self, f: &RustFormatter) -> Option<String> {
        Some(MonitorStruct.struct_name(f))
    }

    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

struct CloseMonitorFunction;

impl FunctionDefinition for CloseMonitorFunction {
    fn key(&self) -> RequirementKey {
        RequirementKey::CloseMonitor
    }

    fn name(&self, _f: &RustFormatter) -> String {
        "close".into()
    }

    fn arguments(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![f.time_argument()]
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(_f.monitor_struct_name())
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(verdicts_result_ty(_f))
    }

    fn mut_self(&self) -> bool {
        true
    }

    fn body(self, f: &RustFormatter) -> String {
        if f.dynamic_deadlines.is_empty() && f.static_deadlines.is_empty() {
            return "Ok(Vec::new())".into();
        }

        [
            "let mut verdicts = Vec::new();",
            &format!(
                "while let Some(timed_event) = self.{} {{",
                f.call_function(
                    QueueNextFunction,
                    &[
                        QueueStruct.argument_name(f),
                        f.time_argument_name(),
                        "true".into()
                    ]
                )
            ),
            "let verdict = self.cycle(timed_event)?;",
            "verdicts.push(verdict)",
            "}",
            "return Ok(verdicts);",
        ]
        .join("\n")
    }

    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}
