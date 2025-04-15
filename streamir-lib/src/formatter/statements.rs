//! A framework for formatting StreamIR statements in the target language

use itertools::Itertools;

use crate::ir::{
    expressions::Expr, Guard, InputReference, LocalFreqRef, OutputReference, Stmt, StreamReference,
    WindowReference,
};

/// A trait for formatting StreamIR statements in the target language (see [DefaultStmtFormatter] for a
/// convenience trait for formatter returning Strings).
pub trait StmtFormatter {
    /// The return type of the formatter
    type Return;

    /// The representation of [Stmt::Skip] in the target language
    fn skip(&self) -> Self::Return;

    /// The representation of [Stmt::Seq] in the target language
    fn seq(&self, inner: Vec<Stmt>) -> Self::Return;

    /// The representation of [Stmt::Parallel] in the target language
    fn parallel(&self, inner: Vec<Stmt>) -> Self::Return;

    /// The representation of [Stmt::Shift] in the target language
    fn shift(&self, sr: StreamReference) -> Self::Return;

    /// The representation of [Stmt::Input] in the target language
    fn input(&self, sr: InputReference) -> Self::Return;

    /// The representation of [Stmt::Spawn] in the target language
    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Self::Return;

    /// The representation of [Stmt::Eval] in the target language
    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> Self::Return;

    /// The representation of [Stmt::Close] in the target language
    fn close(
        &self,
        sr: OutputReference,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Self::Return;

    /// The representation of [Stmt::If] in the target language
    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> Self::Return;

    /// The representation of [Stmt::Iterate] in the target language
    fn iterate(&self, sr: Vec<OutputReference>, inner: Stmt) -> Self::Return;

    /// The representation of [Stmt::Assign] in the target language
    fn assign(
        &self,
        sr: Vec<OutputReference>,
        parameter_expr: Vec<Expr>,
        inner: Stmt,
    ) -> Self::Return;

    /// The representation of a statement in the target language
    fn stmt(&self, stmt: Stmt) -> Self::Return {
        match stmt {
            Stmt::Skip => self.skip(),
            Stmt::Seq(stmts) => self.seq(stmts),
            Stmt::Parallel(stmts) => self.parallel(stmts),
            Stmt::Shift(sr) => self.shift(sr),
            Stmt::Input(sr) => self.input(sr),
            Stmt::Spawn {
                sr,
                with,
                local_frequencies,
                windows,
            } => self.spawn(sr, with, local_frequencies, windows),
            Stmt::Eval { sr, with, idx } => self.eval(sr, with, idx),
            Stmt::Close {
                sr,
                local_frequencies,
                windows,
            } => self.close(sr, local_frequencies, windows),
            Stmt::If(if_stmt) => {
                let (guard, cons, alt) = if_stmt.destruct();
                self.r#if(guard, cons, alt)
            }
            Stmt::Iterate { sr, stmt } => self.iterate(sr, *stmt),
            Stmt::Assign {
                parameter_expr,
                sr,
                stmt,
            } => self.assign(sr, parameter_expr, *stmt),
        }
    }
}

/// A convenience trait for [StmtFormatter]'s returning a string.
pub trait DefaultStmtFormatter
where
    Self: StmtFormatter<Return = String>,
{
    /// The representation of [Stmt::Skip] in the target language
    fn skip(&self) -> String {
        "".into()
    }

    /// The representation of [Stmt::Shift] in the target language
    fn shift(&self, sr: StreamReference) -> String;

    /// The representation of [Stmt::Input] in the target language
    fn input(&self, sr: InputReference) -> String;

    /// The representation of [Stmt::Spawn] in the target language
    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> String;

    /// The representation of [Stmt::Eval] in the target language
    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> String;

    /// The representation of [Stmt::Close] in the target language
    fn close(
        &self,
        sr: OutputReference,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> String;

    /// The representation of [Stmt::If] in the target language
    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> String;

    /// The representation of [Stmt::Iterate] in the target language
    fn iterate(&self, sr: Vec<OutputReference>, inner: Stmt) -> String;

    /// The representation of [Stmt::Assign] in the target language
    fn assign(&self, sr: Vec<OutputReference>, parameter_expr: Vec<Expr>, inner: Stmt) -> String;

    /// The representation of [Stmt::Seq] in the target language
    fn seq(&self, inner: Vec<Stmt>) -> String {
        inner.into_iter().map(|stmt| self.stmt(stmt)).join("\n")
    }

    /// The representation of [Stmt::Parallel] in the target language
    fn parallel(&self, inner: Vec<Stmt>) -> String {
        <Self as DefaultStmtFormatter>::seq(self, inner)
    }
}

impl<F: DefaultStmtFormatter> StmtFormatter for F {
    type Return = String;

    fn skip(&self) -> Self::Return {
        <Self as DefaultStmtFormatter>::skip(self)
    }

    fn seq(&self, inner: Vec<Stmt>) -> Self::Return {
        <Self as DefaultStmtFormatter>::seq(self, inner)
    }

    fn parallel(&self, inner: Vec<Stmt>) -> Self::Return {
        <Self as DefaultStmtFormatter>::parallel(self, inner)
    }

    fn shift(&self, sr: StreamReference) -> Self::Return {
        <Self as DefaultStmtFormatter>::shift(self, sr)
    }

    fn input(&self, sr: InputReference) -> Self::Return {
        <Self as DefaultStmtFormatter>::input(self, sr)
    }

    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Self::Return {
        <Self as DefaultStmtFormatter>::spawn(self, sr, with, local_frequencies, windows)
    }

    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> Self::Return {
        <Self as DefaultStmtFormatter>::eval(self, sr, with, idx)
    }

    fn close(
        &self,
        sr: OutputReference,
        local_frequencies: Vec<LocalFreqRef>,
        windows: Vec<WindowReference>,
    ) -> Self::Return {
        <Self as DefaultStmtFormatter>::close(self, sr, local_frequencies, windows)
    }

    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> Self::Return {
        <Self as DefaultStmtFormatter>::r#if(self, guard, cons, alt)
    }

    fn iterate(&self, sr: Vec<OutputReference>, inner: Stmt) -> Self::Return {
        <Self as DefaultStmtFormatter>::iterate(self, sr, inner)
    }

    fn assign(
        &self,
        sr: Vec<OutputReference>,
        parameter_expr: Vec<Expr>,
        inner: Stmt,
    ) -> Self::Return {
        <Self as DefaultStmtFormatter>::assign(self, sr, parameter_expr, inner)
    }
}
