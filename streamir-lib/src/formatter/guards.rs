//! A framework for formatting guards in the target language.

use std::time::Duration;

use crate::ir::{expressions::Expr, Guard, LocalFreqRef, StreamReference};

/// A trait defining how guards are represented in the target language.
pub trait GuardFormatter {
    /// The return type of the formatter.
    type Return;
    /// The representation of a [Guard::Stream] in the target language.
    fn stream(&self, sr: StreamReference) -> Self::Return;

    /// The representation of a [Guard::Alive] in the target language.
    fn alive(&self, sr: StreamReference) -> Self::Return;

    /// The representation of a [Guard::Dynamic] in the target language.
    fn dynamic(&self, expr: Expr) -> Self::Return;

    /// The representation of a [Guard::GlobalFreq] in the target language.
    fn global_freq(&self, duration: Duration) -> Self::Return;

    /// The representation of a [Guard::LocalFreq] in the target language.
    fn local_freq(&self, freq_ref: LocalFreqRef) -> Self::Return;

    /// The representation of a [Guard::And] in the target language.
    fn and(&self, lhs: Guard, rhs: Guard) -> Self::Return;

    /// The representation of a [Guard::Or] in the target language.
    fn or(&self, lhs: Guard, rhs: Guard) -> Self::Return;

    /// The representation of a [Guard::Constant] in the target language.
    fn constant(&self, b: bool) -> Self::Return;

    /// The representation of a [Guard::FastAnd] in the target language.
    fn fast_and(&self, inner: Vec<StreamReference>) -> Self::Return {
        self.guard(fast_to_normal_guard(inner, |lhs, rhs| Guard::And {
            lhs,
            rhs,
        }))
    }

    /// The representation of a [Guard::FastOr] in the target language.
    fn fast_or(&self, inner: Vec<StreamReference>) -> Self::Return {
        self.guard(fast_to_normal_guard(inner, |lhs, rhs| Guard::Or {
            lhs,
            rhs,
        }))
    }

    /// The representation of the guard in the target language.
    fn guard(&self, g: Guard) -> Self::Return {
        match g {
            Guard::Stream(sr) => self.stream(sr),
            Guard::Alive(sr) => self.alive(sr),
            Guard::Dynamic(expr) => self.dynamic(expr),
            Guard::GlobalFreq(duration) => self.global_freq(duration),
            Guard::LocalFreq(freq_ref) => self.local_freq(freq_ref),
            Guard::And { lhs, rhs } => self.and(*lhs, *rhs),
            Guard::Or { lhs, rhs } => self.or(*lhs, *rhs),
            Guard::Constant(b) => self.constant(b),
            Guard::FastAnd(inner) => self.fast_and(inner),
            Guard::FastOr(inner) => self.fast_or(inner),
        }
    }
}

fn fast_to_normal_guard(
    inner: Vec<StreamReference>,
    f: impl Fn(Box<Guard>, Box<Guard>) -> Guard,
) -> Guard {
    inner
        .into_iter()
        .map(Guard::Stream)
        .reduce(|a, b| f(Box::new(a), Box::new(b)))
        .unwrap()
}

/// A convenience trait for [GuardFormatter]'s that return strings.
pub trait DefaultGuardFormatter
where
    Self: GuardFormatter<Return = String>,
{
    /// The representation of a [Guard::Stream] in the target language.
    fn stream(&self, sr: StreamReference) -> String;

    /// The representation of a [Guard::Alive] in the target language.
    fn alive(&self, sr: StreamReference) -> String;

    /// The representation of a [Guard::Dynamic] in the target language.
    fn dynamic(&self, expr: Expr) -> String;

    /// The representation of a [Guard::GlobalFreq] in the target language.
    fn global_freq(&self, duration: Duration) -> String;

    /// The representation of a [Guard::LocalFreq] in the target language.
    fn local_freq(&self, freq_ref: LocalFreqRef) -> String;

    /// The representation of a [Guard::Constant] in the target language.
    fn constant(&self, b: bool) -> String;

    /// The representation of a [Guard::And] in the target language.
    fn and(&self, lhs: Guard, rhs: Guard) -> String {
        format!("({} && {})", self.guard(lhs), self.guard(rhs))
    }

    /// The representation of a [Guard::Or] in the target language.
    fn or(&self, lhs: Guard, rhs: Guard) -> String {
        format!("({} || {})", self.guard(lhs), self.guard(rhs))
    }

    /// The representation of a [Guard::FastAnd] in the target language.
    fn fast_and(&self, inner: Vec<StreamReference>) -> String {
        self.guard(fast_to_normal_guard(inner, |lhs, rhs| Guard::And {
            lhs,
            rhs,
        }))
    }

    /// The representation of a [Guard::FastOr] in the target language.
    fn fast_or(&self, inner: Vec<StreamReference>) -> String {
        self.guard(fast_to_normal_guard(inner, |lhs, rhs| Guard::Or {
            lhs,
            rhs,
        }))
    }
}

impl<F: DefaultGuardFormatter> GuardFormatter for F {
    type Return = String;

    fn stream(&self, sr: StreamReference) -> Self::Return {
        <Self as DefaultGuardFormatter>::stream(self, sr)
    }

    fn alive(&self, sr: StreamReference) -> Self::Return {
        <Self as DefaultGuardFormatter>::alive(self, sr)
    }

    fn dynamic(&self, expr: Expr) -> Self::Return {
        <Self as DefaultGuardFormatter>::dynamic(self, expr)
    }

    fn global_freq(&self, duration: Duration) -> Self::Return {
        <Self as DefaultGuardFormatter>::global_freq(self, duration)
    }

    fn local_freq(&self, freq_ref: LocalFreqRef) -> Self::Return {
        <Self as DefaultGuardFormatter>::local_freq(self, freq_ref)
    }

    fn and(&self, lhs: Guard, rhs: Guard) -> Self::Return {
        <Self as DefaultGuardFormatter>::and(self, lhs, rhs)
    }

    fn or(&self, lhs: Guard, rhs: Guard) -> Self::Return {
        <Self as DefaultGuardFormatter>::or(self, lhs, rhs)
    }

    fn constant(&self, b: bool) -> Self::Return {
        <Self as DefaultGuardFormatter>::constant(self, b)
    }

    fn fast_and(&self, inner: Vec<StreamReference>) -> Self::Return {
        <Self as DefaultGuardFormatter>::fast_and(self, inner)
    }

    fn fast_or(&self, inner: Vec<StreamReference>) -> Self::Return {
        <Self as DefaultGuardFormatter>::fast_or(self, inner)
    }
}
