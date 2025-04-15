//! A framework for automatically translating the StreamIR to a target language.

use crate::ir::StreamIr;

pub mod expressions;
pub mod files;
pub mod guards;
pub mod names;
pub mod statements;
pub mod types;

/// The main formatter trait to be used with the `translate` method of the library.
pub trait StreamIrFormatter {
    /// The type that is returned when formatting the whole StreamIR.
    type Return;
    /// An identifier string for the given formatter (to be used in file header information etc.)
    fn id(&self) -> String;
    /// Translates the StreamIR into the target type.
    fn format(self, ir: StreamIr) -> Self::Return;
}
