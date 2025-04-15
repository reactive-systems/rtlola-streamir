//! Convenient macro to quickly define a list of name formattings for streams and windows

use crate::ir::{InputReference, OutputReference, StreamReference, WindowReference};

/// Defines how the names of streams and windows are represented in the target language
pub trait GetStreamName {
    /// Returns the name of the given stream in the target language
    fn stream_name(&self, sr: StreamReference) -> String;

    /// Returns the name of the given input stream in the target language
    fn input_name(&self, sr: InputReference) -> String {
        self.stream_name(StreamReference::In(sr))
    }

    /// Returns the name of the given output stream in the target language
    fn output_name(&self, sr: OutputReference) -> String {
        self.stream_name(sr.sr())
    }

    /// Returns the name of the given window in the target language
    fn window_name(&self, sref: WindowReference) -> String;
}

#[macro_export]
/// Mapping of argument names to references
macro_rules! arg_ty {
    (window) => {
        $crate::ir::WindowReference
    };
    (stream) => {
        $crate::ir::StreamReference
    };
    (num) => {
        usize
    };
}

#[macro_export]
/// Returns the text representation of the reference
macro_rules! arg_text {
    (stream, $self:expr, $stream:expr) => {
        $self.stream_name($stream)
    };
    (window, $self:expr, $window:expr) => {
        $self.window_name($window)
    };
    (num, $self:expr, $num:expr) => {
        $num.to_string()
    };
}

#[macro_export]
/// Allows for easy definition of function names based on streams and windows
macro_rules! function_names {
	($self:ty, $($(#[$($attrss:tt)*])* $name:ident($($arg:ident),*): $format_str:literal),*) => {
        use $crate::{arg_text, arg_ty};
		impl $self {
			$(
				$(#[$($attrss)*])*
				pub(crate) fn $name(&self, $($arg: arg_ty!($arg)),*) -> String {
                    use $crate::formatter::names::GetStreamName;
					$(
                        let $arg = arg_text!($arg, self, $arg);
                    )*
					format!($format_str, $($arg=$arg),*)
				}
			)*
		}
    }
}
