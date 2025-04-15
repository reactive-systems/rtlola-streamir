use streamir_lib::{
    formatter::{
        files::{FilesFormatter, Requirement},
        types::TypeFormatter,
    },
    ir::{windows::WindowOperation, Type},
};
use tera::Context;

use crate::{
    constructs::{RequirementKey, WindowsKey},
    RustFormatter,
};

use super::AggregationTrait;

pub(crate) struct SumAggregation;

impl SumAggregation {
    pub(crate) fn ty(f: &RustFormatter, ty: Type) -> String {
        f.add_requirement(Self);
        format!("SumAggregation<{}>", f.ty(ty.inner_ty().clone()))
    }
}

impl Requirement<RustFormatter> for SumAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Sum))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/sum.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct AverageAggregation;

impl AverageAggregation {
    pub(crate) fn ty(f: &RustFormatter, ty: Type) -> String {
        f.add_requirement(Self);
        let Type::Option(inner) = ty else {
            unreachable!("ensured by type checker")
        };
        format!("AvgAggregation<{}>", f.ty(*inner))
    }
}

impl Requirement<RustFormatter> for AverageAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Average))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.import("std::ops::Div", self.file(formatter));
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/average.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct CountAggregation;

impl CountAggregation {
    pub(crate) fn ty(f: &RustFormatter, target_ty: Type, ty: Type) -> String {
        f.add_requirement(Self);
        format!(
            "CountAggregation<{}, {}>",
            f.ty(target_ty),
            f.ty(ty.inner_ty().clone())
        )
    }
}

impl Requirement<RustFormatter> for CountAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Count))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/count.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct MinAggregation;

impl MinAggregation {
    pub(crate) fn ty(f: &RustFormatter, ty: Type) -> String {
        f.add_requirement(Self);
        let Type::Option(inner) = ty else {
            unreachable!("ensured by type checker")
        };
        format!("MinAggregation<{}>", f.ty(*inner))
    }
}

impl Requirement<RustFormatter> for MinAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Min))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/min.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct MaxAggregation;

impl MaxAggregation {
    pub(crate) fn ty(f: &RustFormatter, ty: Type) -> String {
        f.add_requirement(Self);
        let Type::Option(inner) = ty else {
            unreachable!("ensured by type checker")
        };
        format!("MaxAggregation<{}>", f.ty(*inner))
    }
}

impl Requirement<RustFormatter> for MaxAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Max))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/max.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct ConjunctionAggregation;

impl ConjunctionAggregation {
    pub(crate) fn ty(f: &RustFormatter) -> String {
        f.add_requirement(Self);
        "ConjunctionAggregation".into()
    }
}

impl Requirement<RustFormatter> for ConjunctionAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Conjunction))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/conjunction.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct DisjunctionAggregation;

impl DisjunctionAggregation {
    pub(crate) fn ty(f: &RustFormatter) -> String {
        f.add_requirement(Self);
        "DisjunctionAggregation".into()
    }
}

impl Requirement<RustFormatter> for DisjunctionAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Disjunction))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter
            .tera
            .render("windows/disjunction.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct VarianceAggregation;

impl VarianceAggregation {
    pub(crate) fn ty(f: &RustFormatter, ty: Type) -> String {
        f.add_requirement(Self);
        let Type::Option(inner) = ty else {
            unreachable!("ensured by type checker")
        };
        format!("VarianceAggregation<{}>", f.ty(*inner))
    }
}

impl Requirement<RustFormatter> for VarianceAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Variance))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter.import("std::ops::Div", self.file(formatter));
        formatter
            .tera
            .render("windows/variance.rs", &Context::new())
            .unwrap()
    }
}

pub(crate) struct IntegralAggregation;

impl IntegralAggregation {
    pub(crate) fn ty(f: &RustFormatter, target_type: Type, ty: Type) -> String {
        f.add_requirement(Self);
        format!(
            "IntegralAggregation<{}, {}>",
            f.ty(target_type),
            f.ty(ty.inner_ty().clone())
        )
    }
}

impl Requirement<RustFormatter> for IntegralAggregation {
    fn key(&self) -> RequirementKey {
        RequirementKey::Windows(WindowsKey::Aggregation(WindowOperation::Integral))
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        formatter.import("std::ops::Mul", self.file(formatter));
        formatter.import("std::ops::Div", self.file(formatter));
        formatter
            .tera
            .render("windows/integral.rs", &Context::new())
            .unwrap()
    }
}
