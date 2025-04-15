use itertools::Itertools;
use std::fmt::Write;
use streamir_lib::{
    formatter::{
        expressions::ExprFormatter,
        files::{FilesFormatter, Requirement},
        types::TypeFormatter,
    },
    ir::{expressions::Expr, Type},
};

use crate::{RequirementKey, SolidityFormatter};

impl TypeFormatter for SolidityFormatter {
    type Return = String;

    fn type_int(&self, bits: u16) -> Self::Return {
        format!("int{bits}")
    }

    fn type_uint(&self, bits: u16) -> Self::Return {
        format!("uint{bits}")
    }

    fn type_bool(&self) -> Self::Return {
        "bool".into()
    }

    fn type_string(&self) -> Self::Return {
        "string".into()
    }

    fn type_float32(&self) -> Self::Return {
        panic!("not supported in solidity")
    }

    fn type_float64(&self) -> Self::Return {
        panic!("not supported in solidity")
    }

    fn type_option(&self, _inner: Type) -> Self::Return {
        panic!("not supported in solidity")
    }

    fn type_tuple(&self, inner: Vec<Type>) -> Self::Return {
        let tuple = TupleType(inner);
        let ty = tuple.ty(self);
        self.add_requirement(tuple);
        ty
    }

    fn type_fixed(&self, _bits: u16) -> Self::Return {
        unimplemented!()
    }

    fn type_ufixed(&self, _bits: u16) -> Self::Return {
        unimplemented!()
    }

    fn type_bytes(&self) -> Self::Return {
        unimplemented!()
    }
}

pub(crate) struct TupleType(pub Vec<Type>);

impl TupleType {
    fn ty(&self, f: &SolidityFormatter) -> String {
        if self.0.is_empty() {
            "UNIT".into()
        } else {
            self.0.iter().cloned().map(|ty| f.ty(ty)).join("__")
        }
    }

    pub(crate) fn expr(&self, exprs: Vec<Expr>, f: &SolidityFormatter) -> String {
        format!(
            "{}({{{}}})",
            self.ty(f),
            exprs
                .into_iter()
                .enumerate()
                .map(|(i, exp)| format!("_{i}: {}", f.expr(exp)))
                .join(", ")
        )
    }
}

impl Requirement<SolidityFormatter> for TupleType {
    fn key(&self) -> <SolidityFormatter as FilesFormatter>::Key {
        RequirementKey::TupleDefinition(self.0.clone())
    }

    fn file(&self, formatter: &SolidityFormatter) -> std::path::PathBuf {
        formatter.file().into()
    }

    fn format(self, f: &SolidityFormatter) -> String {
        let name = self.ty(f);
        let fields = self.0.iter().cloned().map(|ty| f.ty(ty)).enumerate().fold(
            String::new(),
            |mut res, (i, ty)| {
                writeln!(res, "{ty} _{i};").unwrap();
                res
            },
        );
        format!("struct {name} {{\n{fields}\n}}")
    }
}

impl SolidityFormatter {
    pub(crate) fn type_with_storage(&self, ty: Type) -> String {
        match ty {
            ty @ Type::Tuple(_) => format!("{} memory", self.ty(ty)),
            other => self.ty(other),
        }
    }
}
