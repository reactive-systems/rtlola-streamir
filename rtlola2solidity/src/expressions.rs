pub(crate) mod get_access;

use get_access::GetAccessFunction;
use std::fmt::Write;
use streamir_lib::{
    formatter::{
        expressions::{
            DefaultConstantFormatter, DefaultExprFormatter, DefaultFunctionFormatter,
            DefaultOperatorFormatter, ExprFormatter,
        },
        files::FilesFormatter,
        types::TypeFormatter,
    },
    ir::{expressions::Expr, OutputReference, StreamReference, Type, WindowReference},
};

use crate::{types::TupleType, SolidityFormatter};

impl DefaultExprFormatter for SolidityFormatter {
    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        match sr {
            StreamReference::In(_) | StreamReference::Out(OutputReference::Unparameterized(_)) => {
                self.name(sr).into()
            }
            StreamReference::Out(OutputReference::Parameterized(_)) => {
                let parameters = parameters.into_iter().fold(String::new(), |mut res, p| {
                    write!(res, "[{}]", self.expr(p)).unwrap();
                    res
                });
                self.access_buffer_value(sr, "0", parameters)
            }
        }
    }

    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> String {
        self.call_function(
            GetAccessFunction { sr },
            parameters
                .into_iter()
                .map(|p| self.expr(p))
                .chain([offset.to_string(), self.expr(default)])
                .collect(),
        )
    }

    fn hold_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String {
        self.call_function(
            GetAccessFunction { sr },
            parameters
                .into_iter()
                .map(|p| self.expr(p))
                .chain(["0".into(), self.expr(default)])
                .collect(),
        )
    }

    fn get_access(&self, _sr: StreamReference, _default: Expr, _parameters: Vec<Expr>) -> String {
        unimplemented!()
    }

    fn is_fresh(&self, _sr: StreamReference, _parameters: Vec<Expr>) -> String {
        unimplemented!()
    }

    fn sliding_window_access(&self, _wref: usize, _default: Option<Expr>) -> String {
        unimplemented!()
    }

    fn discrete_window_access(&self, _wref: usize, _default: Option<Expr>) -> String {
        unimplemented!()
    }

    fn instance_aggregation(&self, _wref: usize, _default: Option<Expr>) -> String {
        unimplemented!()
    }

    fn parameter_access(&self, sr: StreamReference, p: usize) -> String {
        self.stream_parameter(sr).unwrap()[p].name.to_owned()
    }

    fn lambda_parameter_access(&self, _wref: WindowReference, _idx: usize) -> String {
        unimplemented!()
    }

    fn cast(&self, ty: Type, expr: Expr) -> String {
        format!("{}({})", self.ty(ty), self.expr(expr))
    }

    fn tuple(&self, inner: Vec<Expr>) -> String {
        let inner_tys = inner.iter().map(|e| e.ty.clone()).collect();
        let tuple = TupleType(inner_tys);
        let expr = tuple.expr(inner, self);
        self.add_requirement(tuple);
        expr
    }
}

impl DefaultConstantFormatter for SolidityFormatter {
    fn constant_int(&self, i: i64, bits: u16) -> String {
        format!("int{bits}({i})")
    }

    fn constant_uint(&self, i: u64, bits: u16) -> String {
        format!("uint{bits}({i})")
    }
}

impl DefaultFunctionFormatter for SolidityFormatter {}

impl DefaultOperatorFormatter for SolidityFormatter {}
