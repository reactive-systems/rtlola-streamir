use anyhow::Context;
use rtlola2solidity::{
    interface::InterfaceConfig, SolidityFormatter, TriggerAction, TriggerFunctionMode,
};
use std::path::PathBuf;
use streamir_lib::{
    optimize, parse,
    rewrite_rules::{
        CombineIf, CombineIterate, CombineNestedIf, CombineSeq, ImpliedGuards, IterateAssign,
        MemoryOptimizations, MoveCommonGuardsOutside, MoveIfOutside, RemoveClose, RemoveIfs,
        RemoveShift, RemoveSpawn, SimplifyGuard,
    },
    translate, ParserConfig,
};

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// A tool to compile RTLola specifications to Solidity
struct Args {
    /// Path to the specification
    spec_path: PathBuf,
    /// Configuration that maps function arguments/return values to input/output streams
    config_file: PathBuf,
    #[clap(long, short = 'n', default_value = "Contract")]
    /// The name of the resulting contract
    contract_name: String,
    #[clap(long, short, value_enum, default_value_t = OptimizationLevel::PartialEval)]
    /// Whether to optimize the IR
    optimize: OptimizationLevel,
    /// Whether a trigger throws revert or emits an event
    #[clap(long, value_enum, default_value_t=TriggerAction::EmitMultiple)]
    trigger_action: TriggerAction,
    /// Whether a function is generated for each trigger or only a single one
    #[clap(long, value_enum, default_value_t=TriggerFunctionMode::Multiple)]
    trigger_function_mode: TriggerFunctionMode,
    #[clap(long)]
    /// The path where the output contract is written to
    output_file: PathBuf,
    /// Whether to overwrite already existing output files
    #[clap(long)]
    overwrite: bool,
    /// Emit the given output streams at the end of each cycle evaluation (for testing purposes)
    #[clap(long)]
    output_streams: Vec<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy, Eq, PartialEq)]
enum OptimizationLevel {
    PartialEval,
    Rewriting,
    Memory,
    All,
}

fn main() -> anyhow::Result<()> {
    let Args {
        spec_path,
        config_file,
        contract_name,
        optimize: optimize_level,
        trigger_action,
        output_file,
        overwrite,
        trigger_function_mode,
        output_streams,
    } = Args::parse();

    if trigger_action == TriggerAction::EmitMultiple
        && trigger_function_mode == TriggerFunctionMode::Single
    {
        anyhow::bail!("Conflicting command line arguments: --trigger-action emit-multiple with --trigger-function-mode single")
    }

    let parser_config = ParserConfig::from_path(spec_path).context("loading specification file")?;
    // let parser_config = if optimize_level == OptimizationLevel::All
    //     || optimize_level == OptimizationLevel::Memory
    // {
    //     parser_config.with_memory_bound_mode(MemoryBoundMode::Static)
    // } else {
    //     parser_config.with_memory_bound_mode(MemoryBoundMode::Static)
    // };

    let ir = parse(&parser_config).context("parsing specification to StreamIR")?;

    let optimized_ir = match optimize_level {
        OptimizationLevel::PartialEval => Ok(ir),
        OptimizationLevel::Rewriting => optimize(
            ir,
            vec![
                Box::new(CombineIf),
                Box::new(SimplifyGuard),
                Box::new(MoveCommonGuardsOutside),
                Box::new(ImpliedGuards),
                Box::new(SimplifyGuard),
                Box::new(RemoveIfs),
                Box::new(CombineSeq),
                Box::new(MoveIfOutside),
                Box::new(IterateAssign),
                Box::new(CombineNestedIf),
                Box::new(CombineIterate),
                Box::new(RemoveIfs),
            ],
        ),
        OptimizationLevel::Memory => optimize(
            ir,
            vec![
                Box::new(RemoveShift),
                Box::new(MemoryOptimizations),
                Box::new(RemoveSpawn),
                Box::new(RemoveClose),
            ],
        ),
        OptimizationLevel::All => optimize(
            ir,
            vec![
                Box::new(CombineIf),
                Box::new(SimplifyGuard),
                Box::new(MoveCommonGuardsOutside),
                Box::new(ImpliedGuards),
                Box::new(SimplifyGuard),
                Box::new(RemoveIfs),
                Box::new(CombineSeq),
                Box::new(MoveIfOutside),
                Box::new(IterateAssign),
                Box::new(CombineNestedIf),
                Box::new(CombineIterate),
                Box::new(RemoveIfs),
                Box::new(RemoveShift),
                Box::new(MemoryOptimizations),
                Box::new(RemoveSpawn),
                Box::new(RemoveClose),
            ],
        ),
    }
    .context("optimizing specification")?;

    let config = std::fs::read_to_string(config_file).context("reading config file")?;
    let config = InterfaceConfig::from_toml(&config).map_err(anyhow::Error::msg)?;

    let output_streams = output_streams
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim())
        .map(|stream_name| {
            optimized_ir
                .sr2memory
                .iter()
                .find_map(|(sr, m)| (m.name == stream_name).then_some(*sr))
                .ok_or_else(|| {
                    anyhow::anyhow!("stream {stream_name} does not exist in the specification")
                })
        })
        .collect::<anyhow::Result<_>>()?;

    let formatter = SolidityFormatter::new(
        &optimized_ir,
        config,
        contract_name,
        trigger_action,
        trigger_function_mode,
        output_file,
        overwrite,
        output_streams,
    );

    translate(optimized_ir, formatter).map_err(anyhow::Error::msg)
}
