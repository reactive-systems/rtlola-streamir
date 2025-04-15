use std::{collections::HashMap, fmt::Write, time::Duration};

use itertools::Itertools;
use rtlola_frontend::mir::InputReference;

use crate::formatter::{
    expressions::{
        DefaultConstantFormatter, DefaultExprFormatter, DefaultFunctionFormatter,
        DefaultOperatorFormatter, ExprFormatter,
    },
    guards::{DefaultGuardFormatter, GuardFormatter},
    statements::{DefaultStmtFormatter, StmtFormatter},
    types::TypeFormatter,
    StreamIrFormatter,
};

use super::{
    Expr, Guard, LocalFreq, LocalFreqRef, OutputReference, Stmt, StreamIr, StreamReference, Type,
    WindowReference,
};

/// A [StreamIrFormatter] trait implementation to display the StreamIR in a nicely formatted string
#[derive(Debug, Clone)]
pub struct DebugFormatter {
    sr2name: HashMap<StreamReference, String>,
    sr2parameter: HashMap<StreamReference, Vec<String>>,
    window_targets: HashMap<WindowReference, StreamReference>,
    lref2lfreq: HashMap<LocalFreqRef, LocalFreq>,
}

impl DebugFormatter {
    /// Creates a new debug formatter
    pub fn new(ir: &StreamIr) -> Self {
        let sr2name = ir
            .sr2memory
            .iter()
            .map(|(sr, mem)| (*sr, mem.name.clone()))
            .collect();
        let sr2parameter = ir
            .sr2memory
            .iter()
            .map(|(sr, mem)| {
                (
                    *sr,
                    mem.parameters()
                        .map(|p| p.iter().map(|p| p.name.clone()).collect())
                        .unwrap_or_else(Vec::new),
                )
            })
            .collect();
        let window_targets = ir
            .wref2window
            .iter()
            .map(|(wref, memory)| (*wref, memory.target))
            .collect();
        let lref2lfreq = ir.lref2lfreq.clone();
        Self {
            sr2name,
            sr2parameter,
            window_targets,
            lref2lfreq,
        }
    }
}

impl StreamIrFormatter for DebugFormatter {
    type Return = String;

    fn id(&self) -> String {
        "DebugPrinter".into()
    }

    fn format(self, ir: StreamIr) -> Self::Return {
        StreamIrPrinter::new(
            &self.sr2name,
            &self.sr2parameter,
            &self.window_targets,
            &self.lref2lfreq,
        )
        .stmt(ir.stmt)
    }
}

struct StreamIrPrinter<'a> {
    whitespace_counter: usize,
    sr2name: &'a HashMap<StreamReference, String>,
    sr2parameter: &'a HashMap<StreamReference, Vec<String>>,
    window_targets: &'a HashMap<WindowReference, StreamReference>,
    lref2lfreq: &'a HashMap<LocalFreqRef, LocalFreq>,
}

impl<'a> StreamIrPrinter<'a> {
    fn new(
        sr2name: &'a HashMap<StreamReference, String>,
        sr2parameter: &'a HashMap<StreamReference, Vec<String>>,
        window_targets: &'a HashMap<WindowReference, StreamReference>,
        lref2lfreq: &'a HashMap<LocalFreqRef, LocalFreq>,
    ) -> Self {
        Self {
            whitespace_counter: 0,
            sr2name,
            sr2parameter,
            window_targets,
            lref2lfreq,
        }
    }

    fn indent(&self) -> Self {
        Self {
            whitespace_counter: self.whitespace_counter + 1,
            sr2name: self.sr2name,
            sr2parameter: self.sr2parameter,
            window_targets: self.window_targets,
            lref2lfreq: self.lref2lfreq,
        }
    }

    fn name(&self, sr: StreamReference) -> &str {
        &self.sr2name[&sr]
    }

    fn whitespace(&self, s: &str) -> String {
        let ws = (0..self.whitespace_counter * 4)
            .map(|_| " ")
            .collect::<String>();
        format!("{ws}{}", s)
    }

    fn parameter(&self, sr: StreamReference) -> String {
        self.sr2parameter[&sr].iter().join(" ,")
    }

    fn stream_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        format!(
            "{}({})",
            self.name(sr),
            parameters.into_iter().map(|p| self.expr(p)).join(",")
        )
    }
}

impl DefaultStmtFormatter for StreamIrPrinter<'_> {
    fn skip(&self) -> String {
        self.whitespace("skip")
    }

    fn shift(&self, sr: StreamReference) -> String {
        self.whitespace(&format!("shift {}", self.name(sr)))
    }

    fn input(&self, sr: InputReference) -> String {
        self.whitespace(&format!("input {}", self.name(StreamReference::In(sr))))
    }

    fn spawn(
        &self,
        sr: OutputReference,
        with: Option<Vec<Expr>>,
        _local_frequencies: Vec<LocalFreqRef>,
        _windows: Vec<WindowReference>,
    ) -> String {
        let with = with
            .map(|with| {
                format!(
                    "with ({})",
                    with.into_iter().map(|with| self.expr(with)).join(",")
                )
            })
            .unwrap_or_default();
        self.whitespace(&format!("spawn {} {with}", self.name(sr.sr())))
    }

    fn eval(&self, sr: OutputReference, with: Expr, idx: usize) -> String {
        self.whitespace(&format!(
            "eval_{idx} {} with {}",
            self.name(sr.sr()),
            self.expr(with)
        ))
    }

    fn close(
        &self,
        sr: OutputReference,
        _local_frequencies: Vec<LocalFreqRef>,
        _windows: Vec<WindowReference>,
    ) -> String {
        self.whitespace(&format!("close {}", self.name(sr.sr())))
    }

    fn r#if(&self, guard: Guard, cons: Stmt, alt: Option<Stmt>) -> String {
        let inner_formater = self.indent();
        let guard = self.guard(guard);
        let cons = inner_formater.stmt(cons);
        let alt = alt.map(|alt| inner_formater.stmt(alt));
        let cons = self.whitespace(&format!("if {guard} then\n{cons}"));
        if let Some(alt) = alt {
            cons + &self.whitespace(&format!("\nelse\n{alt}"))
        } else {
            cons
        }
    }

    fn iterate(&self, srs: Vec<OutputReference>, inner: Stmt) -> String {
        let sr = srs[0].sr();
        let inner_formatter = self.indent();
        let inner = inner_formatter.stmt(inner);
        let parameter = self.parameter(sr);
        let names = srs.into_iter().map(|s| self.name(s.sr())).join(", ");
        self.whitespace(&format!("({parameter}) <- iterate {names}\n{inner}",))
    }

    fn assign(&self, sr: Vec<OutputReference>, parameter_expr: Vec<Expr>, inner: Stmt) -> String {
        let sr = sr[0].sr();
        let inner_formatter = self.indent();
        let inner = &inner_formatter.stmt(inner);
        let parameter = self.parameter(sr);
        let parameter_expr = parameter_expr.into_iter().map(|p| self.expr(p)).join(",");
        self.whitespace(&format!(
            "({parameter}) <- assign {parameter_expr}\n{inner}",
        ))
    }

    fn seq(&self, inner: Vec<Stmt>) -> String {
        let inner_strings = inner
            .into_iter()
            .map(|stmt| self.stmt(stmt))
            .collect::<Vec<_>>();
        let longest_line = inner_strings
            .iter()
            .map(|s| s.lines().map(|line| line.len()).max().unwrap())
            .max()
            .unwrap();
        let leading_whitespace = inner_strings
            .iter()
            .map(|s| s.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap();
        let separator =
            " ".repeat(leading_whitespace) + &"-".repeat(longest_line - leading_whitespace);
        let separator = format!("\n{separator}\n");
        inner_strings.join(&separator)
    }

    fn parallel(&self, inner: Vec<Stmt>) -> String {
        let inner_strings = inner
            .into_iter()
            .map(|stmt| self.stmt(stmt))
            .collect::<Vec<_>>();
        let leading_whitespace = inner_strings
            .iter()
            .map(|s| {
                s.lines()
                    .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
                    .min()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut inner_lines = inner_strings
            .iter()
            .map(|stmt| stmt.lines())
            .collect::<Vec<_>>();
        let longest_line_per_stmt = inner_strings
            .iter()
            .map(|s| s.lines().map(|line| line.len()).max().unwrap())
            .collect::<Vec<_>>();

        let mut res = String::new();
        for i in 0.. {
            let current_lines = inner_lines
                .iter_mut()
                .map(|lines| lines.next())
                .collect::<Vec<_>>();
            if current_lines.iter().all(|line| line.is_none()) {
                break;
            }

            if i != 0 {
                writeln!(res).unwrap();
            }

            for (i, line) in current_lines.into_iter().enumerate() {
                write!(
                    &mut res,
                    "{:width$}",
                    if i == 0 {
                        line.unwrap_or("")
                    } else {
                        line.map(|l| &l[leading_whitespace[i]..]).unwrap_or("")
                    },
                    width = if i == 0 {
                        longest_line_per_stmt[i]
                    } else {
                        longest_line_per_stmt[i] - leading_whitespace[i]
                    }
                )
                .expect("can't fail with strings");
                if i != inner_lines.len() - 1 {
                    write!(&mut res, " | ").unwrap();
                }
            }
        }

        res
    }
}

impl DefaultGuardFormatter for StreamIrPrinter<'_> {
    fn stream(&self, sr: StreamReference) -> String {
        format!("@{}", self.name(sr))
    }

    fn alive(&self, sr: StreamReference) -> String {
        format!("Alive({})", self.name(sr))
    }

    fn dynamic(&self, expr: Expr) -> String {
        format!("( {} )", self.expr(expr))
    }

    fn global_freq(&self, duration: Duration) -> String {
        format!("@Global({}s)", duration.as_secs_f64())
    }

    fn local_freq(&self, lref: LocalFreqRef) -> String {
        let LocalFreq {
            dur,
            sr,
            reference: _,
        } = self.lref2lfreq[&lref];
        format!("@Local({}s, {})", dur.as_secs_f64(), self.name(sr.sr()))
    }

    fn constant(&self, b: bool) -> String {
        if b {
            "⊤".into()
        } else {
            "⊥".into()
        }
    }

    fn fast_and(&self, inner: Vec<StreamReference>) -> String {
        let s = inner.into_iter().map(|s| self.name(s)).join("&&");
        format!("@({s})")
    }

    fn fast_or(&self, inner: Vec<StreamReference>) -> String {
        let s = inner.into_iter().map(|s| self.name(s)).join("||");
        format!("@({s})")
    }
}

impl DefaultExprFormatter for StreamIrPrinter<'_> {
    fn sync_access(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        self.stream_access(sr, parameters)
    }

    fn offset_access(
        &self,
        sr: StreamReference,
        offset: u32,
        default: Expr,
        parameters: Vec<Expr>,
    ) -> String {
        let sr = self.stream_access(sr, parameters);
        let dft = self.expr(default);
        format!("{sr}.offset(by: -{offset}).defaults(to: {dft})")
    }

    fn hold_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String {
        let sr = self.stream_access(sr, parameters);
        let dft = self.expr(default);
        format!("{sr}.hold().defaults(to: {dft})")
    }

    fn get_access(&self, sr: StreamReference, default: Expr, parameters: Vec<Expr>) -> String {
        let sr = self.stream_access(sr, parameters);
        let dft = self.expr(default);
        format!("{sr}.get().defaults(to: {dft})")
    }

    fn is_fresh(&self, sr: StreamReference, parameters: Vec<Expr>) -> String {
        let sr = self.stream_access(sr, parameters);
        format!("{sr}.is_fresh()")
    }

    fn sliding_window_access(&self, window_idx: usize, default: Option<Expr>) -> String {
        let target_name = self.name(self.window_targets[&WindowReference::Sliding(window_idx)]);
        let c = format!("{target_name}.aggregate_sliding({window_idx})");
        if let Some(default) = default {
            format!("{c}.defaults(to: {})", self.expr(default))
        } else {
            c
        }
    }

    fn discrete_window_access(&self, window_idx: usize, default: Option<Expr>) -> String {
        let target_name = self.name(self.window_targets[&WindowReference::Discrete(window_idx)]);
        let c = format!("{target_name}.aggregate_discrete({window_idx})");
        if let Some(default) = default {
            format!("{c}.defaults(to: {})", self.expr(default))
        } else {
            c
        }
    }

    fn instance_aggregation(&self, window_idx: usize, default: Option<Expr>) -> String {
        let target_name = self.name(self.window_targets[&WindowReference::Instance(window_idx)]);
        let c = format!("{target_name}.aggregate_instances({window_idx})");
        if let Some(default) = default {
            format!("{c}.defaults(to: {})", self.expr(default))
        } else {
            c
        }
    }

    fn parameter_access(&self, sr: StreamReference, p: usize) -> String {
        self.sr2parameter[&sr][p].clone()
    }

    fn lambda_parameter_access(&self, _wref: WindowReference, _idx: usize) -> String {
        unreachable!("is never printed as we only print window references")
    }

    fn cast(&self, ty: Type, expr: Expr) -> String {
        format!(
            "cast<{},{}>({})",
            self.ty(expr.ty.clone()),
            self.ty(ty),
            self.expr(expr)
        )
    }
}

impl DefaultFunctionFormatter for StreamIrPrinter<'_> {}

impl DefaultOperatorFormatter for StreamIrPrinter<'_> {}

impl DefaultConstantFormatter for StreamIrPrinter<'_> {}

impl TypeFormatter for StreamIrPrinter<'_> {
    type Return = String;

    fn type_int(&self, bits: u16) -> Self::Return {
        format!("Int{bits}")
    }

    fn type_uint(&self, bits: u16) -> Self::Return {
        format!("UInt{bits}")
    }

    fn type_bool(&self) -> Self::Return {
        "Bool".into()
    }

    fn type_string(&self) -> Self::Return {
        "String".into()
    }

    fn type_float32(&self) -> Self::Return {
        "Float32".into()
    }

    fn type_float64(&self) -> Self::Return {
        "Float64".into()
    }

    fn type_option(&self, inner: Type) -> Self::Return {
        format!("{}?", self.ty(inner))
    }

    fn type_tuple(&self, inner: Vec<Type>) -> Self::Return {
        let inners = inner.into_iter().map(|inner| self.ty(inner)).join(", ");
        format!("({inners})")
    }

    fn type_fixed(&self, bits: u16) -> Self::Return {
        format!("Fixed{bits}")
    }

    fn type_ufixed(&self, bits: u16) -> Self::Return {
        format!("UFixed{bits}")
    }

    fn type_bytes(&self) -> Self::Return {
        "Bytes".into()
    }
}

impl StreamIr {
    /// Display the StreamIR using a debugging formatter
    pub fn display(self) -> String {
        DebugFormatter::new(&self).format(self)
    }
}

#[cfg(test)]
mod tests {
    use rtlola_frontend::{parse, ParserConfig};

    use crate::ir::StreamIr;

    #[test]
    fn test() {
        let spec = "input a : UInt64
        input c: UInt64
        output b(p)
            spawn with a eval when p == a with b(p).last(or: 0) + 1";
        let mir = parse(&ParserConfig::for_string(spec.into())).unwrap();
        let streamir: StreamIr = mir.try_into().unwrap();
        println!("{}", streamir.display());
    }
}
