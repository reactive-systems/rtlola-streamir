use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use itertools::Itertools;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        types::TypeFormatter,
    },
    ir::{windows::WindowOperation, InputReference, OutputReference, StreamReference, Type},
};

use crate::{error::MonitorError, RustFormatter};

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
/// Sorts the construct in the resulting files
///
/// Constructs with keys listed higher at the enum declaration below are sorted higher in the resulting output files.
pub enum RequirementKey {
    Import(String),
    MonitorError,
    MonitorStruct,
    MonitorConstructor,
    StreamMemory,
    StreamMemoryConstructor,
    StreamBufferTrait,
    StaticStreamBuffer,
    DynamicStreamBuffer,
    InstanceStreamBuffer,
    Schedule(ScheduleKey),
    Windows(WindowsKey),
    ExternalEventStruct,
    InternalEventStruct,
    InternalEventFromExternal,
    InternalEventFromDeadline,
    InternalEventEmpty,
    VerdictStruct,
    VerdictConstructor,
    VerdictDisplay,
    VerdictHeader,
    ExprFunction(usize),
    ClearActivations,
    Cycle,
    StreamAccess(StreamAccessType),
    Statement(StatementType),
    AcceptEventFunction,
    AcceptTimeFunction,
    CloseMonitor,
    MainFunction,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum ScheduleKey {
    StreamReferenceEnum,
    DeadlineEnum,
    StateStruct,
    StateAfter,
    StateTraitImplementations,
    QueueDefinition,
    QueueConstructor,
    QueuePop,
    QueuePush,
    QueueCollectAndAdd,
    QueueRemove,
    QueueNext,
    Clear,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum WindowsKey {
    Memory,
    MemoryConstructor,
    AggregationTrait,
    Aggregation(WindowOperation),
    SlidingBuffer,
    ParameterizedSlidingBuffer,
    ConditionalInstanceAggregation,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum StreamAccessType {
    SyncAccess(StreamReference),
    OffsetAccess(StreamReference),
    Hold(StreamReference),
    IsFresh(StreamReference),
    GetAccess(StreamReference),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum StatementType {
    Input(InputReference),
    Eval(OutputReference, usize),
}

impl FilesFormatter for RustFormatter {
    type Key = RequirementKey;

    fn get_construct_store(&self) -> &streamir_lib::formatter::files::ConstructStore<Self> {
        &self.construct_store
    }

    fn overwrite(&self) -> bool {
        self.overwrite
    }
}

/// Trait to represent the definition of a function
///
/// Use `RustFormatter.call_function()` to add the function as a requirement to the store and return the expression for calling the function.
pub(crate) trait FunctionDefinition {
    /// The name of the function
    fn name(&self, _f: &RustFormatter) -> String;

    /// The struct this function is a method of (or None if not a method)
    fn method_of(&self, _f: &RustFormatter) -> Option<String> {
        None
    }

    /// Whether a `self` argument is added to the function
    fn self_argument(&self, _f: &RustFormatter) -> bool {
        self.method_of(_f).is_some()
    }

    /// The arguments of this function. A list of argument name-type pairs.
    fn arguments(&self, _f: &RustFormatter) -> Vec<(String, RustType)> {
        Vec::new()
    }

    /// The return type of the function
    fn returns(&self, _f: &RustFormatter) -> Option<RustType> {
        None
    }

    /// The body of the function
    fn body(self, _f: &RustFormatter) -> String;

    /// The requirement key of this function (where it is sorted in the file)
    fn key(&self) -> RequirementKey;

    /// The visibility of the function
    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Private
    }

    /// The expression that is used to call the function with the given [Self::call_arguments].
    fn call_string(&self, f: &RustFormatter, args: Vec<String>) -> String {
        if self.self_argument(f) {
            format!("{}.{}({})", args[0], self.name(f), args[1..].join(", "))
        } else if let Some(m) = self.method_of(f) {
            format!("{m}::{}({})", self.name(f), args.join(", "))
        } else {
            format!("{}({})", self.name(f), args.join(", "))
        }
    }

    /// Whether this function takes a &mut self reference (otherwise it takes &self if it is a method).
    fn mut_self(&self) -> bool {
        false
    }

    fn file(&self, _f: &RustFormatter) -> PathBuf;
}

pub(crate) enum FunctionVisibility {
    Private,
    Crate,
    Public,
}

impl FunctionVisibility {
    fn prefix_str(&self) -> &'static str {
        match self {
            FunctionVisibility::Private => "",
            FunctionVisibility::Crate => "pub(crate) ",
            FunctionVisibility::Public => "pub ",
        }
    }
}

/// Wrapper for types implementing the FunctionDefinition trait
/// to be able to generically implement the `Requirement` trait for it.
struct FD<F: FunctionDefinition>(F);

impl<F: FunctionDefinition> Requirement<RustFormatter> for FD<F> {
    fn key(&self) -> RequirementKey {
        self.0.key()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        let name = self.0.name(formatter);
        let method_of = self.0.method_of(formatter);
        let arguments = self.0.arguments(formatter);
        let s = self
            .0
            .self_argument(formatter)
            .then(|| {
                let mut s = if self.0.mut_self() {
                    "&mut self"
                } else {
                    "&self"
                }
                .to_owned();
                if !arguments.is_empty() {
                    s.push(',');
                }
                s
            })
            .unwrap_or_else(String::new);
        let args = arguments
            .into_iter()
            .map(|(name, ty)| format!("{name}: {}", formatter.rust_ty(ty)))
            .join(", ");
        let ret = self
            .0
            .returns(formatter)
            .map(|ty| formatter.rust_ty(ty))
            .unwrap_or_else(|| "()".into());
        let vis = self.0.visibility().prefix_str();
        let header = format!("{vis}fn {name}({s}{args}) -> {ret}");
        let body = self.0.body(formatter);
        let function = format!("{header} {{\n{body}\n}}");
        if let Some(of) = method_of {
            format!("impl {of} {{\n{function}\n}}")
        } else {
            function
        }
    }

    fn file(&self, formatter: &RustFormatter) -> PathBuf {
        self.0.file(formatter)
    }
}

/// Trait representing a struct definition
pub(crate) trait StructDefinition {
    fn key(&self) -> RequirementKey;

    fn file(&self, _f: &RustFormatter) -> PathBuf;

    /// The type name of the struct
    fn struct_name(&self, _f: &RustFormatter) -> String;

    /// The typical name of the argument/variable of this type.
    fn argument_name(&self, _f: &RustFormatter) -> String {
        self.struct_name(_f).to_lowercase()
    }

    /// The fields of the struct
    fn fields(&self, _f: &RustFormatter) -> Vec<(String, RustType)>;

    /// The visibility of the struct
    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Private
    }

    /// Returns the struct as a type
    fn as_ty(&self, _f: &RustFormatter) -> RustType {
        RustType::Other(self.struct_name(_f))
    }

    /// Returns the pair to use the struct as an argument to another function.
    fn as_argument(&self, _f: &RustFormatter) -> (String, RustType) {
        (self.argument_name(_f), self.as_ty(_f))
    }

    /// A line that is inserted above the struct definition (for derive macros etc.)
    fn decorator(&self, _f: &RustFormatter) -> Option<String> {
        None
    }
}

struct ST<E: StructDefinition>(E);

impl<E: StructDefinition> Requirement<RustFormatter> for ST<E> {
    fn key(&self) -> RequirementKey {
        self.0.key()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        let vis = self.0.visibility().prefix_str();
        let name = self.0.struct_name(formatter);
        let fields = self
            .0
            .fields(formatter)
            .into_iter()
            .map(|(name, ty)| format!("{name}: {},", formatter.rust_ty(ty)))
            .join("\n");
        let decorator = self
            .0
            .decorator(formatter)
            .map(|s| format!("{s}\n"))
            .unwrap_or_default();
        format!("{decorator}{vis}struct {name} {{\n{fields}\n}}")
    }

    fn file(&self, formatter: &RustFormatter) -> PathBuf {
        self.0.file(formatter)
    }
}

pub(crate) struct Import(String, PathBuf);

impl Requirement<RustFormatter> for Import {
    fn key(&self) -> RequirementKey {
        RequirementKey::Import(self.0.clone())
    }

    fn file(&self, _f: &RustFormatter) -> PathBuf {
        self.1.clone()
    }

    fn format(self, _f: &RustFormatter) -> String {
        format!("use {};", self.0)
    }
}

pub(crate) trait EnumDefinition {
    fn key(&self) -> RequirementKey;

    fn file(&self, _f: &RustFormatter) -> PathBuf;

    /// The type name of the struct
    fn enum_name(&self, _f: &RustFormatter) -> String;

    /// The typical name of the argument/variable of this type.
    fn argument_name(&self, _f: &RustFormatter) -> String {
        self.enum_name(_f).to_lowercase()
    }

    /// The fields of the struct
    fn variants(&self, _f: &RustFormatter) -> Vec<String>;

    /// The visibility of the struct
    fn visibility(&self) -> FunctionVisibility {
        FunctionVisibility::Private
    }

    /// Returns the struct as a type
    fn as_ty(&self, _f: &RustFormatter) -> RustType {
        RustType::Other(self.enum_name(_f))
    }

    /// Returns the pair to use the struct as an argument to another function.
    fn as_argument(&self, _f: &RustFormatter) -> (String, RustType) {
        (self.argument_name(_f), self.as_ty(_f))
    }

    /// A line that is inserted above the struct definition (for derive macros etc.)
    fn decorator(&self, _f: &RustFormatter) -> Option<String> {
        None
    }
}

struct ED<E: EnumDefinition>(E);

impl<E: EnumDefinition> Requirement<RustFormatter> for ED<E> {
    fn key(&self) -> RequirementKey {
        self.0.key()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        let vis = self.0.visibility().prefix_str();
        let name = self.0.enum_name(formatter);
        let variants = self
            .0
            .variants(formatter)
            .into_iter()
            .map(|variant| format!("{variant},"))
            .join("\n");
        let decorator = self
            .0
            .decorator(formatter)
            .map(|s| format!("{s}\n"))
            .unwrap_or_default();
        format!("{decorator}{vis}enum {name} {{\n{variants}\n}}")
    }

    fn file(&self, formatter: &RustFormatter) -> PathBuf {
        self.0.file(formatter)
    }
}

pub(crate) enum RustType {
    Lola(Type),
    Other(String),
    Option(Box<RustType>),
    Result(Box<RustType>),
    Tuple(Vec<RustType>),
    Usize,
    Unit,
    SelfTy,
    MutRef(Box<RustType>),
    Vec(Box<RustType>),
    HashMap(Box<RustType>, Box<RustType>),
    Duration,
    Bool,
}

impl From<Type> for RustType {
    fn from(value: Type) -> Self {
        Self::Lola(value)
    }
}

impl RustFormatter {
    pub(crate) fn rust_ty(&self, ty: RustType) -> String {
        match ty {
            RustType::Lola(ty) => self.ty(ty),
            RustType::Other(s) => s,
            RustType::Option(ty) => format!("Option<{}>", self.rust_ty(*ty)),
            RustType::Result(ty) => {
                MonitorError::add_requirement(self);
                format!("Result<{}, {}>", self.rust_ty(*ty), self.error_name())
            }
            RustType::Usize => "usize".to_string(),
            RustType::Unit => "()".to_string(),
            RustType::Tuple(inner) => match inner.len() {
                0 => self.rust_ty(RustType::Unit),
                1 => format!(
                    "({},)",
                    inner.into_iter().map(|s| self.rust_ty(s)).join(", ")
                ),
                2.. => format!(
                    "({},)",
                    inner.into_iter().map(|s| self.rust_ty(s)).join(", ")
                ),
            },
            RustType::SelfTy => "Self".into(),
            RustType::MutRef(ty) => format!("&mut {}", self.rust_ty(*ty)),
            RustType::Vec(inner) => format!("Vec<{}>", self.rust_ty(*inner)),
            RustType::HashMap(from, to) => {
                format!(
                    "std::collections::HashMap<{}, {}>",
                    self.rust_ty(*from),
                    self.rust_ty(*to)
                )
            }
            RustType::Duration => "std::time::Duration".into(),
            RustType::Bool => "bool".into(),
        }
    }
}

impl RustType {
    pub(crate) fn optional(self) -> RustType {
        RustType::Option(Box::new(self))
    }

    pub(crate) fn result(self) -> RustType {
        RustType::Result(Box::new(self))
    }

    pub(crate) fn mut_reference(self) -> RustType {
        RustType::MutRef(Box::new(self))
    }
}

impl RustFormatter {
    pub(crate) fn main_file(&self) -> PathBuf {
        self.output_folder.join("main.rs")
    }

    pub(crate) fn memory_file(&self) -> PathBuf {
        self.main_file()
    }

    pub(crate) fn windows_file(&self) -> PathBuf {
        self.main_file()
    }

    pub(crate) fn call_function<F: FunctionDefinition, S: AsRef<str>>(
        &self,
        f: F,
        args: &[S],
    ) -> String {
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_owned()).collect();
        let call = f.call_string(self, args);
        let fd = FD(f);
        self.add_requirement(fd);
        call
    }

    pub(crate) fn call_self_function<F: FunctionDefinition, S: AsRef<str>>(
        &self,
        f: F,
        args: &[S],
    ) -> String {
        let mut args: Vec<_> = args.iter().map(|s| s.as_ref()).collect();
        args.insert(0, "self");
        self.call_function(f, &args)
    }

    pub(crate) fn import<P: AsRef<Path>, S: AsRef<str>>(&self, i: S, into: P) {
        self.add_requirement(Import(i.as_ref().to_owned(), into.as_ref().to_owned()));
    }

    pub(crate) fn require_struct<E: StructDefinition>(&self, s: E) {
        let ev = ST(s);
        self.add_requirement(ev);
    }

    pub(crate) fn require_enum<E: EnumDefinition>(&self, s: E) {
        let ev = ED(s);
        self.add_requirement(ev);
    }

    pub(crate) fn parameter_arguments(&self, sr: StreamReference) -> Vec<(String, RustType)> {
        self.sr2parameters[&sr]
            .iter()
            .map(|p| (p.name.to_owned(), RustType::from(p.ty.clone())))
            .collect()
    }

    pub(crate) fn format_duration(&self, duration: Duration) -> String {
        format!(
            "Duration::new({}, {})",
            duration.as_secs(),
            duration.subsec_nanos()
        )
    }
}
