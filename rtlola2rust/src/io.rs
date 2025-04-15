use itertools::Itertools;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        names::GetStreamName,
    },
    ir::{StreamReference, Type},
};

use crate::{
    constructs::{EnumDefinition, RequirementKey, RustType, StructDefinition},
    expressions::get::GetAccess,
    schedule::StreamReferenceEnum,
    FunctionDefinition, MonitorStruct, RustFormatter, StreamMemoryStruct,
};

pub(crate) struct ExternalEventStruct;

impl StructDefinition for ExternalEventStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::ExternalEventStruct
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        "Event".into()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.sr2name
            .iter()
            .filter(|(sr, _)| matches!(sr, StreamReference::In(_)))
            .map(|(sr, name)| (name.clone(), RustType::from(f.sr2ty[sr].clone()).optional()))
            .collect()
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }

    fn visibility(&self) -> crate::FunctionVisibility {
        crate::FunctionVisibility::Public
    }
}

pub(crate) struct InternalEventStruct;

impl StructDefinition for InternalEventStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::InternalEventStruct
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        "InternalEvent".into()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.inputs()
            .map(|sr| {
                (
                    f.stream_name(sr).clone(),
                    RustType::from(f.sr2ty[&sr].clone()).optional(),
                )
            })
            .chain(
                f.static_deadlines
                    .iter()
                    .map(|dl| (f.static_deadline_event_name(*dl), RustType::Bool)),
            )
            .chain(f.dynamic_deadlines.iter().map(|dl| {
                (
                    f.dynamic_deadline_event_name(*dl),
                    RustType::Vec(Box::new(StreamReferenceEnum.as_ty(f))),
                )
            }))
            .chain(std::iter::once(f.time_argument()))
            .collect()
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

pub(crate) struct InternalEventEmpty;

impl FunctionDefinition for InternalEventEmpty {
    fn name(&self, _f: &RustFormatter) -> String {
        "empty".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        let fields = f
            .inputs()
            .map(|sr| format!("{}: None", f.stream_name(sr).clone()))
            .chain(
                f.static_deadlines
                    .iter()
                    .map(|dl| format!("{}: false", f.static_deadline_event_name(*dl))),
            )
            .chain(
                f.dynamic_deadlines
                    .iter()
                    .map(|dl| format!("{}: Vec::new()", f.dynamic_deadline_event_name(*dl))),
            )
            .chain(std::iter::once(format!("{0}: {0}", f.time_argument_name())))
            .join(",\n");
        format!("Self {{ {fields} }}")
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(InternalEventStruct.struct_name(_f))
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![_f.time_argument()]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::SelfTy)
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::InternalEventEmpty
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        InternalEventStruct.file(_f)
    }
}

pub(crate) struct InternalEventFromExternal;

impl FunctionDefinition for InternalEventFromExternal {
    fn name(&self, _f: &RustFormatter) -> String {
        "from_event".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        let fields = f
            .inputs()
            .map(|sr| {
                let name = f.stream_name(sr);
                let event_arg = ExternalEventStruct.argument_name(f);
                format!("{name}: {event_arg}.{name}")
            })
            .chain(
                f.static_deadlines
                    .iter()
                    .map(|dl| format!("{}: false", f.static_deadline_event_name(*dl))),
            )
            .chain(
                f.dynamic_deadlines
                    .iter()
                    .map(|dl| format!("{}: Vec::new()", f.dynamic_deadline_event_name(*dl))),
            )
            .chain(std::iter::once(format!("{0}: {0}", f.time_argument_name())))
            .join("\n,");
        format!("Self {{\n{fields}\n}}")
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::InternalEventFromExternal
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(InternalEventStruct.struct_name(_f))
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![_f.time_argument(), ExternalEventStruct.as_argument(_f)]
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(InternalEventStruct.as_ty(_f))
    }
}

pub(crate) struct VerdictStruct;

impl StructDefinition for VerdictStruct {
    fn key(&self) -> RequirementKey {
        RequirementKey::VerdictStruct
    }

    fn struct_name(&self, _f: &RustFormatter) -> String {
        "Verdict".into()
    }

    fn fields(&self, f: &RustFormatter) -> Vec<(String, RustType)> {
        f.add_requirement(VerdictDisplay);
        f.verdict_streams
            .iter()
            .map(|sr| {
                let name = f.stream_name(*sr);
                let parameters = f.stream_parameter(*sr);
                if parameters.is_empty() {
                    (name.clone(), RustType::from(f.sr2ty[sr].clone()).optional())
                } else {
                    (
                        name.clone(),
                        RustType::HashMap(
                            Box::new(f.parameter_ty(*sr).unwrap()),
                            Box::new(f.sr2ty[sr].clone().into()),
                        ),
                    )
                }
            })
            .chain(std::iter::once(f.time_argument()))
            .collect()
    }

    fn visibility(&self) -> crate::FunctionVisibility {
        crate::FunctionVisibility::Public
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

struct VerdictDisplay;

impl Requirement<RustFormatter> for VerdictDisplay {
    fn key(&self) -> RequirementKey {
        RequirementKey::VerdictDisplay
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.import("std::fmt::Display", self.file(formatter));
        let fields = formatter
            .verdict_streams.iter()
            .map(|sr| {
                let name = &formatter.stream_name(*sr);
                let memory = formatter.stream_memory(*sr);
                if memory.parameters().is_some() {
                    // TODO: we can't display parameterized streams in csv
                    // for now, simply display a symbol indicating if any instance was updated
                        format!("write!(f, \"{{}},\", if self.{}.is_empty() {{\"#\"}} else {{\"!\"}});", name)
                } else {
                    format!(
                        "write!(f, \"{{}},\", self.{}.as_ref().map(|v|{}).unwrap_or_else(||\"#\".into()))?;",
                        name,
                        if *formatter.lola_stream_type(*sr) == Type::String {
                            r#"format!("\"{v}\"")"#
                        } else {
                            "v.to_string()"
                        }
                    )
                }
            })
            .join("\n");
        format!(
            "
impl Display for Verdict {{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        {fields}
		writeln!(f, \"{{}}\", self.time.as_secs_f64())
        }}
    }}"
        )
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.main_file()
    }
}

pub(crate) struct VerdictConstructor;

impl FunctionDefinition for VerdictConstructor {
    fn name(&self, _f: &RustFormatter) -> String {
        "new".into()
    }

    fn body(self, f: &RustFormatter) -> String {
        let fields = f
            .verdict_streams
            .iter()
            .map(|sr| {
                let name = f.stream_name(*sr);
                if f.stream_parameter(*sr).is_empty() {
                    let get_access =
                        f.call_function(GetAccess(*sr), &[MonitorStruct.argument_name(f)]);
                    format!("{name}: {get_access}?")
                } else {
                    format!(
                        "{name}: {}.{}.{name}.{}()?",
                        MonitorStruct.argument_name(f),
                        StreamMemoryStruct.argument_name(f),
                        f.fresh_instances_function()
                    )
                }
            })
            .chain(["time: monitor.time".to_string()])
            .join(",\n");
        format!("Ok(Self {{\n{fields}\n}})")
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(VerdictStruct.struct_name(_f))
    }

    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        vec![(
            MonitorStruct.argument_name(_f),
            MonitorStruct.as_ty(_f).mut_reference(),
        )]
    }

    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        Some(RustType::Other("Verdict".into()).result())
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::VerdictConstructor
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        _f.main_file()
    }
}

pub(crate) struct VerdictHeaderFunction;

impl FunctionDefinition for VerdictHeaderFunction {
    fn name(&self, _f: &RustFormatter) -> String {
        _f.verdict_header_function()
    }

    fn body(self, f: &RustFormatter) -> String {
        let s = f
            .verdict_streams
            .iter()
            .map(|sr| f.stream_name(*sr))
            .chain(Some("time".into()))
            .join(",");
        format!("println!(\"{s}\");")
    }

    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        Some(VerdictStruct.struct_name(_f))
    }

    fn self_argument(&self, _f: &RustFormatter) -> bool {
        false
    }

    fn key(&self) -> RequirementKey {
        RequirementKey::VerdictHeader
    }

    fn file(&self, _f: &RustFormatter) -> std::path::PathBuf {
        VerdictStruct.file(_f)
    }
}
