use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use itertools::Itertools;
use rtlola2rust::{MainFunction, RustFormatter};
use streamir_lib::{ir::StreamReference, optimize_all, parse, translate, ParserConfig};

#[derive(Parser)]
struct Args {
    spec: PathBuf,
    #[clap(short, long)]
    optimize: bool,
    #[clap(short = 'd', long, default_value = ".")]
    output_dir: PathBuf,
    #[clap(long)]
    overwrite: bool,
    #[clap(short, long, value_enum, default_value_t=MainFunction::NoMain)]
    main: MainFunction,
    #[clap(short, long, default_value_t=Verbosity::Outputs, value_enum)]
    verbosity: Verbosity,
    #[clap(long)]
    /// Only output the following streams
    output_streams: Vec<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Verbosity {
    Streams,
    Outputs,
    Trigger,
    Silent,
}

fn main() -> anyhow::Result<()> {
    let Args {
        spec,
        optimize,
        output_dir,
        overwrite,
        main,
        verbosity,
        output_streams,
    } = Args::parse();

    let config = ParserConfig::from_path(spec).context("Loading specification file")?;
    let mut ir = parse(&config).context("parsing specification")?;
    if optimize {
        ir = optimize_all(ir).context("optimizing StreamIR")?;
    }

    let verdict_streams: Vec<StreamReference> = if !output_streams.is_empty() {
        output_streams
            .iter()
            .flat_map(|s| s.split(','))
            .map(|s| s.trim())
            .map(|stream_name| {
                ir.sr2memory
                    .iter()
                    .find_map(|(sr, m)| (m.name == stream_name).then_some(*sr))
                    .ok_or_else(|| {
                        anyhow::anyhow!("stream {stream_name} does not exist in the specification")
                    })
            })
            .collect::<anyhow::Result<_>>()?
    } else {
        match verbosity {
            Verbosity::Silent => Vec::new(),
            Verbosity::Trigger => ir.triggers().sorted().map(StreamReference::Out).collect(),
            Verbosity::Outputs => ir.outputs().sorted().map(StreamReference::Out).collect(),
            Verbosity::Streams => ir.streams().sorted().collect(),
        }
    };

    let formatter = RustFormatter::new(&ir, output_dir, overwrite, main, verdict_streams);
    translate(ir, formatter).context("generating rust code")
}
