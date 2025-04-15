use itertools::Itertools;

use super::Type;

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int(i) => write!(f, "Int({i})"),
            Type::UInt(u) => write!(f, "UInt({u})"),
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Float32 => write!(f, "Float(32)"),
            Type::Float64 => write!(f, "Float(64)"),
            Type::Option(inner) => write!(f, "Option<{inner}>"),
            Type::Tuple(items) => write!(f, "({})", items.iter().map(|t| t.to_string()).join(",")),
            Type::Fixed(i) => write!(f, "Fixed{i}"),
            Type::UFixed(i) => write!(f, "UFixed{i}"),
            Type::Bytes => write!(f, "Bytes"),
        }
    }
}
