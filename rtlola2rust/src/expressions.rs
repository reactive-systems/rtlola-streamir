use crate::windows::instance::ConditionalInstanceAggregationFunction;
use get::GetAccess;
use hold::HoldAccess;
use is_fresh::IsFresh;
use itertools::Itertools;
use offset::OffsetAccess;
use streamir_lib::{
    formatter::{
        expressions::{
            DefaultConstantFormatter, DefaultExprFormatter, DefaultFunctionFormatter,
            DefaultOperatorFormatter, ExprFormatter, FunctionFormatter, OperatorFormatter,
        },
        files::FilesFormatter,
        names::GetStreamName,
        types::TypeFormatter,
    },
    ir::{
        expressions::{Expr, Function, Operator},
        memory::Parameter,
        windows::{InstanceSelection, WindowKind},
        StreamReference, Type, WindowReference,
    },
};
use sync::SyncAccess;

use crate::{
    error::MonitorError, RustFormatter, StreamMemoryStruct, StructDefinition, WindowMemory,
};

pub(crate) mod get;
mod hold;
pub(crate) mod is_fresh;
mod offset;
pub(crate) mod sync;

impl DefaultExprFormatter for RustFormatter {
    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        let parameters: Vec<_> = parameters.into_iter().map(|p| self.expr(p)).collect();
        let sync = SyncAccess(sr);
        format!("{}?", self.call_self_function(sync, &parameters))
    }

    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> String {
        let mut args = parameters
            .into_iter()
            .map(|p| self.expr(p))
            .collect::<Vec<_>>();
        args.insert(0, offset.to_string());
        let default = self.expr(default);
        let offset = OffsetAccess(sr);
        let offset = self.call_self_function(offset, &args);
        format!("{offset}?.unwrap_or({default})")
    }

    fn hold_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String {
        let parameters: Vec<_> = parameters.into_iter().map(|p| self.expr(p)).collect();
        let hold = HoldAccess(sr);
        let default = self.expr(default);
        let hold = self.call_self_function(hold, &parameters);
        format!("{hold}?.unwrap_or({default})")
    }

    fn get_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String {
        let parameters: Vec<_> = parameters.into_iter().map(|p| self.expr(p)).collect();
        let call = self.call_self_function(GetAccess(sr), &parameters);
        let default = self.expr(default);
        format!("{call}?.unwrap_or({default})")
    }

    fn is_fresh(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        let parameters: Vec<_> = parameters.into_iter().map(|p| self.expr(p)).collect();
        format!("{}?", self.call_self_function(IsFresh(sr), &parameters))
    }

    fn sliding_window_access(&self, wref: usize, default: Option<Expr>) -> String {
        let wref = WindowReference::Sliding(wref);
        let window = &self.wref2window[&wref];
        // let WindowKind::Sliding {
        //     duration,
        //     bucket_count,
        //     bucket_duration,
        //     wait,
        // } = &window.kind
        // else {
        //     unreachable!("is sliding window")
        // };
        let v = if self.stream_parameter(window.target).is_empty() {
            format!(
                "self.{}.{}.get_value(self.{})",
                WindowMemory.argument_name(self),
                self.window_name(wref),
                self.time_argument_name()
            )
        } else {
            format!(
                "self.{}.{}.get_window(&{}).unwrap().get_value(self.{})",
                WindowMemory.argument_name(self),
                self.window_name(wref),
                self.method_parameters_variable(window.caller),
                self.time_argument_name()
            )
        };
        if let Some(default) = default {
            if window.op.returns_option() {
                format!("{v}.flatten().unwrap_or_else(||{})", self.expr(default))
            } else {
                format!("{v}.unwrap_or_else(||{})", self.expr(default))
            }
        } else {
            format!("{v}.unwrap()")
        }
    }

    fn discrete_window_access(&self, _wref: usize, _default: Option<Expr>) -> String {
        unimplemented!()
    }

    fn instance_aggregation(&self, wref: usize, default: Option<Expr>) -> String {
        self.add_requirement(ConditionalInstanceAggregationFunction);
        let wref = WindowReference::Instance(wref);
        let window = &self.wref2window[&wref];
        let WindowKind::Instances { selection } = &window.kind else {
            unreachable!("is instance window")
        };
        let (fresh, filter) = match selection {
            InstanceSelection::All => (false, None),
            InstanceSelection::Fresh => (true, None),
            InstanceSelection::FilteredAll { parameters, cond } => {
                (false, Some((parameters, cond)))
            }
            InstanceSelection::FilteredFresh { parameters, cond } => {
                (true, Some((parameters, cond)))
            }
        };
        let filter = filter
            .map(|(param, cond)| {
                let param_list = param
                    .iter()
                    .enumerate()
                    .map(|(i, _)| self.instance_aggregation_parameter_name(i))
                    .join(", ");
                let param_list = if param.len() == 1 {
                    param_list
                } else {
                    format!("({param_list})")
                };
                format!("|{param_list}| {}", self.expr(cond.clone()))
            })
            .unwrap_or_else(|| "|_| true".into());
        let op = self.window_aggregation(wref);
        let mut s = format!(
            "self.{}.{}.cond_aggregate_instances::<{op}>({fresh}, {filter})",
            StreamMemoryStruct.argument_name(self),
            self.stream_name(window.target)
        );
        if let Some(default) = default {
            s.push_str(&format!(
                ".unwrap_or_else(||self.expr({}))",
                self.expr(default)
            ));
        }
        s
    }

    fn parameter_access(&self, sr: StreamReference, p: usize) -> String {
        // expressions are always inside a function with the corresponding parameter arguments
        self.parameter_arguments(sr)[p].0.to_owned()
    }

    fn lambda_parameter_access(&self, _wref: WindowReference, idx: usize) -> String {
        self.instance_aggregation_parameter_name(idx)
    }

    fn cast(&self, ty: Type, expr: Expr) -> String {
        match (&expr.ty, &ty) {
            (Type::UInt(_), Type::UInt(_)) | (Type::Int(_), Type::Int(_)) => {
                let expr_code = self.expr(expr);
                format!("(({expr_code}) as {})", self.ty(ty))
            }
            (Type::UInt(_), Type::Int(_)) | (Type::Int(_), Type::UInt(_)) => {
                let expr_code = self.expr(expr);
                format!("(({expr_code}) as {})", self.ty(ty))
            }
            (Type::Int(_) | Type::UInt(_), Type::Float32 | Type::Float64) => {
                let expr_code = self.expr(expr);
                format!("(({expr_code}) as {})", self.ty(ty))
            }
            (from, to) => panic!("Unsupported cast from: {from} to {to}"),
        }
    }

    fn if_then_else(&self, condition: Expr, consequence: Expr, alternative: Expr) -> String {
        let condition = self.expr(condition);
        let consequence = self.expr(consequence);
        let alternative = self.expr(alternative);
        format!("(if {condition} {{ {consequence} }} else {{ {alternative} }})")
    }

    fn binary(&self, op: Operator, lhs: Expr, rhs: Expr) -> String {
        match op {
            Operator::Pow => match (&lhs.ty, &rhs.ty) {
                (Type::Float32 | Type::Float64, Type::Float32 | Type::Float64) => {
                    format!("({}).powf({})", self.expr(lhs), self.expr(rhs))
                }
                (Type::Float32 | Type::Float64, Type::UInt(_) | Type::Int(_)) => {
                    format!("({}).powi({})", self.expr(lhs), self.expr(rhs))
                }
                (Type::UInt(_) | Type::Int(_), Type::UInt(_) | Type::Int(_)) => {
                    format!("({}).pow({})", self.expr(lhs), self.expr(rhs))
                }
                _ => panic!("disallowed by type checker"),
            },
            _ => format!("({} {} {})", self.expr(lhs), self.op(op), self.expr(rhs)),
        }
    }

    fn function_call(&self, function: Function, args: Vec<Expr>) -> String {
        match function {
            Function::Sqrt
            | Function::Abs
            | Function::Sin
            | Function::Arcsin
            | Function::Cos
            | Function::Arccos
            | Function::Tan
            | Function::Arctan => {
                let [lhs]: [Expr; 1] = args.try_into().unwrap();
                format!("({}).{}()", self.expr(lhs), self.function(function))
            }
            Function::Min | Function::Max => {
                let [lhs, rhs]: [Expr; 2] = args.try_into().unwrap();
                format!(
                    "({}).{}({})",
                    self.expr(lhs),
                    self.function(function),
                    self.expr(rhs)
                )
            }
        }
    }
}

impl DefaultConstantFormatter for RustFormatter {
    fn constant_float32(&self, f: f64) -> String {
        format!("{}f64", f)
    }

    fn constant_float64(&self, f: f64) -> String {
        self.constant_float32(f)
    }

    fn constant_string(&self, s: String) -> String {
        format!("\"{s}\".to_owned()")
    }
}

impl DefaultFunctionFormatter for RustFormatter {
    fn function_arccos(&self) -> String {
        "acos".into()
    }

    fn function_arcsin(&self) -> String {
        "asin".into()
    }

    fn function_arctan(&self) -> String {
        "atan".into()
    }
}

impl DefaultOperatorFormatter for RustFormatter {}

impl RustFormatter {
    pub(crate) fn get_stream_value(&self, sr: StreamReference, offset: &str) -> String {
        let stream = self.stream_name(sr);
        let mut res = format!("self.{}.{}", StreamMemoryStruct.argument_name(self), stream);
        res += &match self.stream_parameter(sr) {
            [] => String::new(),
            [p] => {
                let instance = &p.name;
                format!(
                    ".get_instance(&{instance}).ok_or_else(|| {{ {} }})?",
                    MonitorError::instance_not_found(sr, instance, self)
                )
            }
            p => {
                let instance = p.iter().map(|Parameter { name, ty: _ }| name).join(",");
                format!(
                    ".get_instance(&({instance},)).ok_or_else(|| {{ {} }})?",
                    MonitorError::instance_not_found(sr, &instance, self)
                )
            }
        };
        res += &format!(".get({offset})?.cloned()");
        res
    }

    pub(crate) fn get_stream_value_async(&self, sr: StreamReference, offset: &str) -> String {
        let stream = self.stream_name(sr);
        let mut res = format!("self.{}.{}", StreamMemoryStruct.argument_name(self), stream);
        res += &match self.stream_parameter(sr) {
            [] => format!(".get({offset})?"),
            [p] => {
                let instance = &p.name;
                format!(
                    ".get_instance(&{instance}).map(|i| i.get({offset})).transpose()?.flatten()"
                )
            }
            p => {
                let instance = p.iter().map(|Parameter { name, ty: _ }| name).join(",");
                format!(
                    ".get_instance(&({instance})).map(|i| i.get({offset})).transpose()?.flatten()"
                )
            }
        };
        res += ".cloned()";
        res
    }

    pub(crate) fn get_stream_buffer(&self, sr: StreamReference, parameter: &[&str]) -> String {
        let stream = self.stream_name(sr);
        let mut res = format!("self.{}.{}", StreamMemoryStruct.argument_name(self), stream);

        res += &match parameter {
            [] => String::new(),
            [p] => {
                format!(
                    ".get_instance(&{p}).expect(\"Cannot happen! Checked by RTLola Typechecker\")"
                )
            }
            p => {
                format!(
                    ".get_instance(&({},)).expect(\"Cannot happen! Checked by RTLola Typechecker\")",p.iter().join(",")
                )
            }
        };
        res
    }

    pub(crate) fn get_stream_buffer_mut(&self, sr: StreamReference, parameter: &[&str]) -> String {
        let stream = self.stream_name(sr);
        let mut res = format!("self.{}.{}", StreamMemoryStruct.argument_name(self), stream);

        res += &match parameter {
            [] => String::new(),
            [p] => {
                format!(
                    ".get_instance_mut(&{p}).expect(\"Cannot happen! Checked by RTLola Typechecker\")"
                )
            }
            p => {
                format!(
                    ".get_instance_mut(&({},)).expect(\"Cannot happen! Checked by RTLola Typechecker\")",p.iter().join(",")
                )
            }
        };
        res
    }
}
