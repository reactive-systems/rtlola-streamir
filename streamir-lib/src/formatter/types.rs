//! A framework for representing types in the target language

use crate::ir::Type;

/// A trait defining how types are represented in the target language
pub trait TypeFormatter {
    /// The return type of the formatter
    type Return;

    /// Defines how an integer type is represented in the target language
    fn type_int(&self, bits: u16) -> Self::Return;

    /// Defines how an unsigned integer type is represented in the target language
    fn type_uint(&self, bits: u16) -> Self::Return;

    /// Defines how a boolean type is represented in the target language
    fn type_bool(&self) -> Self::Return;

    /// Defines how a string type is represented in the target language
    fn type_string(&self) -> Self::Return;

    /// Defines how an 32-bit float type is represented in the target language
    fn type_float32(&self) -> Self::Return;

    /// Defines how an 64-bit float type is represented in the target language
    fn type_float64(&self) -> Self::Return;

    /// Defines how an optional type is represented in the target language
    fn type_option(&self, inner: Type) -> Self::Return;

    /// Defines how a tuple type is represented in the target language
    fn type_tuple(&self, inner: Vec<Type>) -> Self::Return;

    /// Defines how a signed fixed point number is represented in the target language
    fn type_fixed(&self, bits: u16) -> Self::Return;

    /// Defines how an unsigned fixed point number is represented in the target language
    fn type_ufixed(&self, bits: u16) -> Self::Return;

    /// Defines how the bytes type is represented in the target language
    fn type_bytes(&self) -> Self::Return;

    /// Defines how a type is represented in the target language.
    fn ty(&self, ty: Type) -> Self::Return {
        match ty {
            Type::Int(b) => self.type_int(b),
            Type::UInt(b) => self.type_uint(b),
            Type::Bool => self.type_bool(),
            Type::String => self.type_string(),
            Type::Float32 => self.type_float32(),
            Type::Float64 => self.type_float64(),
            Type::Option(inner) => self.type_option(*inner),
            Type::Tuple(items) => self.type_tuple(items),
            Type::Fixed(b) => self.type_fixed(b),
            Type::UFixed(b) => self.type_ufixed(b),
            Type::Bytes => self.type_bytes(),
        }
    }
}
