use clap::ValueEnum;
use serde_json::json;
use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        names::GetStreamName,
    },
    ir::Type,
};
use tera::Context;

use crate::{constructs::RequirementKey, io::VerdictHeaderFunction, RustFormatter};

#[derive(Clone, Debug, ValueEnum, Copy)]
/// The kind of main function to generate
pub enum MainFunction {
    /// No main function
    NoMain,
    /// A main function reading a trace from a CSV file
    CsvOffline,
}

impl MainFunction {
    pub(crate) fn insert_requirement(&self, f: &RustFormatter) {
        match self {
            MainFunction::NoMain => {}
            MainFunction::CsvOffline => {
                f.add_requirement(CsvOffline);
            }
        }
    }
}

struct CsvOffline;

impl CsvOffline {
    pub fn parse_ty_from_str(&self, ty: &Type, str: &str) -> String {
        match ty {
            Type::Int(_) | Type::Float64 | Type::Float32 | Type::Bool => {
                format!("{str}.parse().unwrap()")
            }
            Type::UInt(b) => format!("{str}.parse::<i64>().unwrap() as u{b}"),
            _ => unimplemented!(),
        }
    }
}

impl Requirement<RustFormatter> for CsvOffline {
    fn key(&self) -> RequirementKey {
        RequirementKey::MainFunction
    }

    fn format(self, formatter: &RustFormatter) -> String {
        let mut context = Context::new();
        context.insert(
            "inputs",
            &formatter
                .inputs()
                .map(|input| {
                    json!({
                        "name": formatter.stream_name(input),
                        "parse_code": self.parse_ty_from_str(formatter.lola_stream_type(input), &format!("{}_str", formatter.stream_name(input)))
                    })
                })
                .collect::<Vec<_>>(),
        );

        context.insert(
            "outputs",
            &formatter
                .outputs()
                .map(|o| formatter.stream_name(o))
                .collect::<Vec<_>>(),
        );
        context.insert(
            "verdict_header_function",
            &formatter.call_function::<_, String>(VerdictHeaderFunction, &[]),
        );
        context.insert("silent", &formatter.verdict_streams.is_empty());

        formatter.import("std::fs::File", self.file(formatter));
        formatter.import("std::time::Duration", self.file(formatter));
        formatter.import("std::io::BufReader", self.file(formatter));
        formatter.import("std::io::BufRead", self.file(formatter));
        formatter.import("std::process::exit", self.file(formatter));
        formatter.import("core::ptr", self.file(formatter));

        formatter
            .tera
            .render("main_function/csv_offline.rs", &context)
            .unwrap()
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.main_file()
    }
}
