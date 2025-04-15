use std::fs::File;
use std::io::{stderr, Stderr};
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use interpreter::csv::{CsvEventSource, CsvVerdictSink};
use interpreter::Monitor;
use streamir_lib::ir::StreamReference;
use streamir_lib::{parse, ParserConfig};

#[derive(Parser, Debug, Clone)]
struct Args {
    spec: PathBuf,
    trace: PathBuf,
    #[arg(short, long, value_enum, default_value_t=Verbosity::Outputs)]
    verbosity: Verbosity,
    #[arg(long, conflicts_with = "verbosity")]
    output_streams: Vec<String>,
    #[arg(short, long)]
    optimize: bool,
    #[arg(long)]
    benchmark: bool,
    // #[arg(long)]
    // cache: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug)]
enum Verbosity {
    Silent,
    Trigger,
    Outputs,
    Streams,
}

// fn load_cache(config: &ParserConfig, cache: &PathBuf) -> anyhow::Result<Option<RtLolaMir>> {
//     let file = File::open(cache)?;
//     let hashed_mir: HashedMir =
//         serde_json::from_reader(file).map_err(|e| anyhow::anyhow!("Hash Error: {e:?}"))?;
//     Ok(hashed_mir.check(config).ok())
// }

fn build(
    config: &ParserConfig,
    trace: PathBuf,
    verbosity: Verbosity,
    output_streams: Vec<String>,
    optimize: bool,
) -> anyhow::Result<(Monitor, CsvEventSource<File>, CsvVerdictSink<Stderr>)> {
    let streamir = parse(config).context("parsing spec")?;
    let csv_source = CsvEventSource::new(File::open(trace)?, &streamir);

    let csv_fields: Vec<StreamReference> = if !output_streams.is_empty() {
        output_streams
            .iter()
            .flat_map(|s| s.split(','))
            .map(|s| s.trim())
            .map(|stream_name| {
                streamir
                    .sr2memory
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
            Verbosity::Trigger => streamir.triggers().map(StreamReference::Out).collect(),
            Verbosity::Outputs => streamir.outputs().map(StreamReference::Out).collect(),
            Verbosity::Streams => streamir.streams().collect(),
        }
    };

    let csv_writer = CsvVerdictSink::new(stderr(), &streamir, &csv_fields)
        .context("building csv output writer")?;

    let monitor = Monitor::build(streamir, optimize);

    Ok((monitor, csv_source, csv_writer))
}

fn run(
    mut monitor: Monitor,
    mut csv_source: CsvEventSource<File>,
    mut csv_writer: CsvVerdictSink<Stderr>,
    benchmark: bool,
) {
    let start = Instant::now();
    let mut last_ts = Duration::new(0, 0);
    while let Some((inputs, ts)) = csv_source.next_event().unwrap() {
        let verdict = monitor.accept_event(inputs, ts);
        if !benchmark {
            for (ts, timed_verdict) in verdict.timed {
                csv_writer.accept_verdict(ts, timed_verdict).unwrap();
            }
            csv_writer
                .accept_verdict(verdict.ts, verdict.event)
                .unwrap();
        } else {
            unsafe {
                std::mem::forget(ptr::read_volatile(&verdict));
            }
        }
        last_ts = ts;
    }
    let verdicts = monitor.finish(last_ts);
    if !benchmark {
        for (ts, timed_verdict) in verdicts {
            csv_writer.accept_verdict(ts, timed_verdict).unwrap();
        }
    } else {
        unsafe {
            std::mem::forget(ptr::read_volatile(&verdicts));
        }
        println!("{}", start.elapsed().as_secs_f64());
    }
}

fn main() -> anyhow::Result<()> {
    let Args {
        spec,
        trace,
        verbosity,
        output_streams,
        optimize,
        benchmark,
        // cache,
    } = Args::parse();

    let config = match ParserConfig::from_path(spec).context("loading specification file") {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let (monitor, source, sink) = build(&config, trace, verbosity, output_streams, optimize)?;
    run(monitor, source, sink, benchmark);
    Ok(())
}
