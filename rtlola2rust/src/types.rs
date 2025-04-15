use itertools::Itertools;
use streamir_lib::formatter::types::TypeFormatter;

use crate::RustFormatter;

impl TypeFormatter for RustFormatter {
    type Return = String;

    fn type_int(&self, bits: u16) -> Self::Return {
        assert!(bits <= 64, "only ints up to 64 bits are supported in rust");
        format!("i{bits}")
    }

    fn type_uint(&self, bits: u16) -> Self::Return {
        assert!(bits <= 64, "only uints up to 64 bits are supported in rust");
        format!("u{bits}")
    }

    fn type_bool(&self) -> Self::Return {
        "bool".into()
    }

    fn type_string(&self) -> Self::Return {
        "String".into()
    }

    fn type_float32(&self) -> Self::Return {
        "f32".into()
    }

    fn type_float64(&self) -> Self::Return {
        "f64".into()
    }

    fn type_option(&self, inner: streamir_lib::ir::Type) -> Self::Return {
        format!("Option<{}>", self.ty(inner))
    }

    fn type_tuple(&self, inner: Vec<streamir_lib::ir::Type>) -> Self::Return {
        format!("({})", inner.into_iter().map(|ty| self.ty(ty)).join(", "))
    }

    fn type_bytes(&self) -> Self::Return {
        "Vec<u8>".into()
    }

    fn type_fixed(&self, _bits: u16) -> Self::Return {
        unimplemented!()
    }

    fn type_ufixed(&self, _bits: u16) -> Self::Return {
        unimplemented!()
    }
}
