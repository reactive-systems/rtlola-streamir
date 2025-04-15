use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, Div, DivAssign, Mul,
    MulAssign, Neg, Not, Rem, RemAssign, Shl, Shr, Sub, SubAssign,
};

use ordered_float::NotNan;
use streamir_lib::ir::Type;
use thiserror::Error;
use Value::*;

use crate::csv::ParseValueFn;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Value {
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Float(NotNan<f64>),
    Str(String),
    Tuple(Vec<Value>),
    None,
}

impl Value {
    pub(crate) fn as_bool(&self) -> bool {
        match self {
            Bool(b) => *b,
            _ => unreachable!(),
        }
    }

    pub(crate) fn unwrap_or_else(self, mut f: impl FnMut() -> Value) -> Value {
        match self {
            None => f(),
            other => other,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Bool(b) => write!(f, "{}", *b),
            Unsigned(u) => write!(f, "{}", *u),
            Signed(s) => write!(f, "{}", *s),
            Float(fl) => write!(f, "{}", *fl),
            Tuple(t) => {
                write!(f, "(")?;
                if let Some(e) = t.first() {
                    write!(f, "{}", e)?;
                    for b in &t[1..] {
                        write!(f, ", {}", b)?;
                    }
                }
                write!(f, ")")
            }
            Str(str) => write!(f, "{}", *str),
            None => write!(f, "None"),
        }
    }
}

impl Add for Value {
    type Output = Value;

    fn add(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1 + v2),
            (Signed(v1), Signed(v2)) => Signed(v1 + v2),
            (Float(v1), Float(v2)) => Float(v1 + v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl AddAssign for Value {
    fn add_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Unsigned(v1), Unsigned(v2)) => v1.add_assign(v2),
            (Signed(v1), Signed(v2)) => v1.add_assign(v2),
            (Float(v1), Float(v2)) => v1.add_assign(v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1 - v2),
            (Signed(v1), Signed(v2)) => Signed(v1 - v2),
            (Float(v1), Float(v2)) => Float(v1 - v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl SubAssign for Value {
    fn sub_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Unsigned(v1), Unsigned(v2)) => v1.sub_assign(v2),
            (Signed(v1), Signed(v2)) => v1.sub_assign(v2),
            (Float(v1), Float(v2)) => v1.sub_assign(v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1 * v2),
            (Signed(v1), Signed(v2)) => Signed(v1 * v2),
            (Float(v1), Float(v2)) => Float(v1 * v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl MulAssign for Value {
    fn mul_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Unsigned(v1), Unsigned(v2)) => v1.mul_assign(v2),
            (Signed(v1), Signed(v2)) => v1.mul_assign(v2),
            (Float(v1), Float(v2)) => v1.mul_assign(v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Div for Value {
    type Output = Value;

    fn div(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1 / v2),
            (Signed(v1), Signed(v2)) => Signed(v1 / v2),
            (Float(v1), Float(v2)) => Float(v1 / v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl DivAssign for Value {
    fn div_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Unsigned(v1), Unsigned(v2)) => v1.div_assign(v2),
            (Signed(v1), Signed(v2)) => v1.div_assign(v2),
            (Float(v1), Float(v2)) => v1.div_assign(v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Rem for Value {
    type Output = Value;

    fn rem(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1 % v2),
            (Signed(v1), Signed(v2)) => Signed(v1 % v2),
            (Float(v1), Float(v2)) => Float(v1 % v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl RemAssign for Value {
    fn rem_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Unsigned(v1), Unsigned(v2)) => v1.rem_assign(v2),
            (Signed(v1), Signed(v2)) => v1.rem_assign(v2),
            (Float(v1), Float(v2)) => v1.rem_assign(v2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Value {
    /// Returns the powered value of a value type, with:
    /// # Arguments:
    /// * 'exp' - The exponent given as a 'Value'
    pub(crate) fn pow(self, exp: Value) -> Value {
        match (self, exp) {
            (Unsigned(v1), Unsigned(v2)) => Unsigned(v1.pow(v2 as u32)),
            (Signed(v1), Signed(v2)) => Signed(v1.pow(v2 as u32)),
            (Float(v1), Float(v2)) => Value::try_from(v1.powf(v2.into())).unwrap(),
            (Float(v1), Signed(v2)) => Value::try_from(v1.powi(v2 as i32)).unwrap(),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl BitAnd for Value {
    type Output = Value;

    fn bitand(self, other: Value) -> Value {
        match (self, other) {
            (Bool(v1), Bool(v2)) => Bool(v1 && v2),
            (Unsigned(u1), Unsigned(u2)) => Unsigned(u1 & u2),
            (Signed(s1), Signed(s2)) => Signed(s1 & s2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl BitAndAssign for Value {
    fn bitand_assign(&mut self, other: Self) {
        match (self, other) {
            (Bool(v1), Bool(v2)) => v1.bitand_assign(v2),
            (Unsigned(u1), Unsigned(u2)) => u1.bitand_assign(u2),
            (Signed(s1), Signed(s2)) => s1.bitand_assign(s2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl BitOr for Value {
    type Output = Value;

    fn bitor(self, other: Value) -> Value {
        match (self, other) {
            (Bool(v1), Bool(v2)) => Bool(v1 || v2),
            (Unsigned(u1), Unsigned(u2)) => Unsigned(u1 | u2),
            (Signed(s1), Signed(s2)) => Signed(s1 | s2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl BitOrAssign for Value {
    fn bitor_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Bool(v1), Bool(v2)) => v1.bitor_assign(v2),
            (Unsigned(u1), Unsigned(u2)) => u1.bitor_assign(u2),
            (Signed(s1), Signed(s2)) => s1.bitor_assign(s2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl BitXor for Value {
    type Output = Value;

    fn bitxor(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(u1), Unsigned(u2)) => Unsigned(u1 ^ u2),
            (Signed(s1), Signed(s2)) => Signed(s1 ^ s2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Shl for Value {
    type Output = Value;

    fn shl(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(u1), Unsigned(u2)) => Unsigned(u1 << u2),
            (Signed(s1), Unsigned(u)) => Signed(s1 << u),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Shr for Value {
    type Output = Value;

    fn shr(self, other: Value) -> Value {
        match (self, other) {
            (Unsigned(u1), Unsigned(u2)) => Unsigned(u1 >> u2),
            (Signed(s1), Unsigned(u)) => Signed(s1 >> u),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl Not for Value {
    type Output = Value;

    fn not(self) -> Value {
        match self {
            Bool(v) => Bool(!v),
            Unsigned(u) => Unsigned(!u),
            Signed(s) => Signed(!s),
            a => panic!("Incompatible type: {:?}", a),
        }
    }
}

impl Neg for Value {
    type Output = Value;

    fn neg(self) -> Value {
        match self {
            Signed(v) => Signed(-v), // TODO Check
            a => panic!("Incompatible type: {:?}", a),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Unsigned(u1), Unsigned(u2)) => u1.cmp(u2),
            (Signed(i1), Signed(i2)) => i1.cmp(i2),
            (Float(i1), Float(i2)) => i1.cmp(i2),
            (a, b) => panic!("Incompatible types: ({:?},{:?})", a, b),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Bool(b)
    }
}

impl From<i16> for Value {
    fn from(i: i16) -> Self {
        Signed(i as i64)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Signed(i as i64)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Signed(i)
    }
}

impl From<u16> for Value {
    fn from(u: u16) -> Self {
        Unsigned(u as u64)
    }
}

impl From<u32> for Value {
    fn from(u: u32) -> Self {
        Unsigned(u as u64)
    }
}

impl From<u64> for Value {
    fn from(u: u64) -> Self {
        Unsigned(u)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Str(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::from(value.to_string())
    }
}

impl TryFrom<f64> for Value {
    type Error = ValueConvertError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let val = NotNan::try_from(value).map_err(|_| ValueConvertError::FloatIsNan)?;
        Ok(Float(val))
    }
}

impl TryFrom<f32> for Value {
    type Error = ValueConvertError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        let val = NotNan::try_from(value as f64).map_err(|_| ValueConvertError::FloatIsNan)?;
        Ok(Float(val))
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Unsigned(value as u64)
    }
}

impl TryInto<bool> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<bool, Self::Error> {
        if let Bool(b) = self {
            Ok(b)
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

impl TryInto<u64> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<u64, Self::Error> {
        if let Unsigned(v) = self {
            Ok(v)
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

impl TryInto<i64> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<i64, Self::Error> {
        if let Signed(v) = self {
            Ok(v)
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

impl TryInto<f64> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<f64, Self::Error> {
        if let Float(v) = self {
            Ok(v.into_inner())
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

impl TryInto<Vec<Value>> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<Vec<Value>, Self::Error> {
        if let Tuple(v) = self {
            Ok(v.to_vec())
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

impl TryInto<String> for Value {
    type Error = ValueConvertError;

    fn try_into(self) -> Result<String, Self::Error> {
        if let Str(v) = self {
            Ok(v.to_string())
        } else {
            Err(ValueConvertError::TypeMismatch(self))
        }
    }
}

#[derive(Debug, Error)]
/// Describes an error occurring when converting from or into a value.
pub enum ValueConvertError {
    #[error("Failed to vonvert value: {0}")]
    TypeMismatch(Value),
    #[error("The given float is not a number (NaN)")]
    FloatIsNan,
    #[error("UTF-8 decoding failed for bytes: {0:?}")]
    NotUtf8(Vec<u8>),
    #[error("Failed to parse value of type {0} from: {1}")]
    ParseError(Type, String),
}

impl Value {
    /// Returns the interpreted values of an byte representation, if possible:
    /// # Arguments
    /// * 'source' - A byte slice that holds the value
    /// * 'ty' - the type of the interpretation
    pub fn bytes_parser(ty: Type) -> ParseValueFn {
        macro_rules! str {
            ($s:expr) => {{
                let s = match std::str::from_utf8($s) {
                    Ok(s) => s,
                    Err(_) => return Err(ValueConvertError::NotUtf8($s.to_vec())),
                };
                if s == "#" {
                    return Ok(Option::None);
                }
                s
            }};
        }
        match ty {
            Type::Bool => Box::new(move |source: &[u8]| {
                let source = str!(source);
                source
                    .parse::<bool>()
                    .map(|v| Some(Bool(v)))
                    .map_err(|_| ValueConvertError::ParseError(ty.clone(), source.to_string()))
            }),
            Type::Int(_) => Box::new(move |source: &[u8]| {
                let source = str!(source);
                source
                    .parse::<i64>()
                    .map(|v| Some(Signed(v)))
                    .map_err(|_| ValueConvertError::ParseError(ty.clone(), source.to_string()))
            }),
            Type::UInt(_) => Box::new(move |source: &[u8]| {
                let source = str!(source);
                if source == "0.0" {
                    Ok(Some(Unsigned(0)))
                } else {
                    source
                        .parse::<u64>()
                        .map(|v| Some(Unsigned(v)))
                        .map_err(|_| ValueConvertError::ParseError(ty.clone(), source.to_string()))
                }
            }),
            Type::Float64 | Type::Float32 => Box::new(move |source: &[u8]| {
                let source = str!(source);
                source
                    .parse::<f64>()
                    .map_err(|_| ValueConvertError::ParseError(ty.clone(), source.to_string()))
                    .and_then(Value::try_from)
                    .map(Some)
            }),
            Type::String => Box::new(|source: &[u8]| {
                let source = str!(source);
                Ok(Some(source.into()))
            }),
            Type::Tuple(inner) => {
                if inner.is_empty() {
                    Box::new(move |source: &[u8]| {
                        let source = str!(source);
                        (source == "()")
                            .then_some(Tuple(Vec::new()))
                            .ok_or_else(|| {
                                ValueConvertError::ParseError(
                                    Type::Tuple(inner.clone()),
                                    source.to_string(),
                                )
                            })
                            .map(Some)
                    })
                } else {
                    unimplemented!()
                }
            }
            Type::Option(_) => unreachable!(),
            Type::Fixed(_) | Type::UFixed(_) | Type::Bytes => unimplemented!(),
        }
    }
}
