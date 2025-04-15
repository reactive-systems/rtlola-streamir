use std::{path::PathBuf, process::exit};

use clap::{Parser, ValueEnum};
use rtlola_frontend::{Handler, ParserConfig};
use streamir_lib::{ir::DebugFormatter, parse, translate, ParseError};

#[derive(Parser)]
struct Args {
    spec: PathBuf,
    #[clap(short, long)]
    optimize_all: bool,
}

#[derive(ValueEnum, Clone)]
enum Optimization {}

fn print(config: &ParserConfig, optimize_all: bool) -> Result<String, ParseError> {
    let streamir = parse(config)?;
    let streamir = if optimize_all {
        streamir_lib::optimize_all(streamir).expect("optimize error")
    } else {
        streamir
    };
    let formatter = DebugFormatter::new(&streamir);
    Ok(translate(streamir, formatter))
}

fn main() {
    let Args { spec, optimize_all } = Args::parse();

    let config = ParserConfig::from_path(spec).unwrap();
    match print(&config, optimize_all) {
        Ok(s) => println!("{s}"),
        Err(ParseError::FrontendError(e)) => {
            let handler = Handler::from(&config);
            handler.emit_error(&e);
            exit(1)
        }
        Err(other) => {
            eprintln!("{other}");
            exit(1)
        }
    }
}
