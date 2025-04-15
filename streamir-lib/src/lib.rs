//! A common library for bringing the RTLolaMir, the output of the Frontend, into the StreamIR representation.
//! It offers functionality for optimizing the StreamIR through rewriting rules and a framework for formatting the StreamIR in a target language.

#![forbid(unused_must_use)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

use formatter::StreamIrFormatter;
use ir::{LoweringError, StreamIr};
use rewrite_rules::{
    CombineIf, CombineIterate, CombineNestedIf, CombineSeq, FastGuards, ImpliedGuards,
    IterateAssign, MemoryOptimizations, MoveIfOutside, RemoveClose, RemoveIfs, RemoveShift,
    RemoveSkip, RewriteError, RewriteRule, Rewriter, SimplifyGuard,
};
use rewrite_rules::{MoveCommonGuardsOutside, RemoveSpawn};
pub use rtlola_frontend::ParserConfig;
use rtlola_frontend::RtLolaError;
pub use rtlola_frontend::{FrontendConfig, MemoryBoundMode, ParserConfigExt};
use thiserror::Error;

pub mod formatter;
pub mod ir;
pub mod rewrite_rules;

#[derive(Clone, Debug, Error)]
/// An error that can happen when parsing a specification into the StreamIR
pub enum ParseError {
    #[error("Parser error: {0}")]
    /// The error happened when parsing the specification into the MIR
    FrontendError(#[from] RtLolaError),
    #[error("Lowering error: {0}")]
    /// The error happened when lowering the MIR into the StreamIR
    LoweringError(#[from] LoweringError),
}

/// Parse a specification directly into the StreamIR representation.
///
/// See the RTLola Frontend on more details about the `config` argument.
/// The resulting StreamIR does not have any optimizations applied.
pub fn parse<'a>(config: impl Into<FrontendConfig<'a>>) -> Result<StreamIr, ParseError> {
    let mir = rtlola_frontend::parse(config)?;
    let streamir: StreamIr = mir.try_into()?;
    Ok(optimize(streamir, vec![Box::new(RemoveSkip)]).unwrap())
}

/// Applies all general optimizations to the given StreamIR.
pub fn optimize_all(ir: StreamIr) -> Result<StreamIr, RewriteError> {
    let ir = optimize(
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
    )?;
    optimize(ir, vec![Box::new(FastGuards)])
}

/// Applies a given list of rewriting rules to the StreamIR.
pub fn optimize(ir: StreamIr, rules: Vec<Box<dyn RewriteRule>>) -> Result<StreamIr, RewriteError> {
    let rewriter = Rewriter::new(rules);
    rewriter.run(ir)
}

/// Uses the given formatter for translating the StreamIR to the target language.
pub fn translate<F: StreamIrFormatter>(ir: StreamIr, f: F) -> F::Return {
    f.format(ir)
}
