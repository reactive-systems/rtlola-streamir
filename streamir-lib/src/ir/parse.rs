use std::{collections::HashMap, time::Duration};

use disjoint::DisjointSet;
use winnow::{
    ascii::{dec_int, dec_uint, multispace0, multispace1},
    combinator::{alt, cut_err, delimited, opt, preceded, separated, seq},
    error::{ParserError, StrContext},
    stream::{AsChar, Stream, StreamIsPartial},
    Parser,
};

use super::{
    expressions::{Constant, Expr, ExprKind, Operator},
    memory::{Memory, Parameter, StreamBuffer, StreamMemory},
    Guard, LivetimeEquivalences, LocalFreq, OutputReference, Stmt, StreamIr, StreamReference, Type,
};

fn iws<Input, Output, Error, ParseNext>(parser: ParseNext) -> impl Parser<Input, Output, Error>
where
    Input: Stream + StreamIsPartial,
    <Input as Stream>::Token: AsChar + Clone,
    ParseNext: Parser<Input, Output, Error>,
    Error: ParserError<Input>,
{
    delimited(multispace0, parser, multispace0)
}

fn parse_stmt_prime(input: &mut &str) -> winnow::ModalResult<Stmt> {
    iws(alt((
        preceded(
            iws("seq"),
            cut_err(delimited("{", separated(.., parse_stmt_prime, iws(";")), (opt(iws(";")), "}"))),
        )
        .context(StrContext::Label("seq"))
        .map(Stmt::Seq),
        preceded(
            iws("par"),
            cut_err(delimited("{", separated(.., parse_stmt_prime, iws(";")), (opt(iws(";")), "}"))),
        ).context(StrContext::Label("parallel"))
        .map(Stmt::Parallel),
        preceded("if", cut_err(seq!(_: multispace1, parse_guard, _: "then", parse_stmt_prime, opt(seq!(_: "else", parse_stmt_prime)), _: "fi"))).context(StrContext::Label("if")).map(
            |(guard, cons, alt)| {
                Stmt::If(super::IfStmt {
                    guard,
                    cons: Box::new(cons),
                    alt: Box::new(alt.unwrap_or((Stmt::Skip,)).0),
                })
            },
        ),
        preceded("input", cut_err(seq!(_: multispace1, dec_int))).context(StrContext::Label("input")).map(|(i,): (i32,)| Stmt::Input(i as usize)),
        preceded("eval", cut_err(seq!(_: multispace1, dec_int, _: multispace1, parse_expr))).context(StrContext::Label("eval")).map(|(i, expr): (i32,_)| Stmt::Eval {
            sr: OutputReference::Unparameterized(i as usize),
            with: expr,
            idx: 0
        }),
        preceded("iterate", cut_err(seq!(_: multispace1, dec_int, _: multispace1, parse_stmt_prime))).context(StrContext::Label("iterate")).map(|(i,stmt): (i32,_)| Stmt::Iterate {
            sr: vec![OutputReference::Unparameterized(i as usize)],
            stmt: Box::new(stmt)
        }),
        preceded("assign", cut_err(seq!(_: multispace1, dec_int, _: multispace1, delimited("(", separated(.., parse_expr, ","), ")"), _: multispace1, parse_stmt_prime))).context(StrContext::Label("assign")).map(|(i,expr, stmt): (i32,_, _)| Stmt::Assign {
            sr: vec![OutputReference::Unparameterized(i as usize)],
            parameter_expr: expr,
            stmt: Box::new(stmt)
        }),
    )))
    .parse_next(input)
}

pub(crate) fn parse_guard(input: &mut &str) -> winnow::ModalResult<Guard> {
    let lhs = parse_guard_term.parse_next(input)?;
    let op = opt(iws(alt(("&&", "||")))).parse_next(input)?;

    if let Some(op) = op {
        let rhs = parse_guard.parse_next(input)?;
        match op {
            "&&" => Ok(Guard::And {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }),
            "||" => Ok(Guard::Or {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }),
            _ => unreachable!(),
        }
    } else {
        Ok(lhs)
    }
}

fn parse_guard_term(input: &mut &str) -> winnow::ModalResult<Guard> {
    iws(alt((
        "true".value(Guard::Constant(true)),
        "false".value(Guard::Constant(false)),
        preceded("@", dec_int).map(|i: i32| Guard::Stream(StreamReference::In(i as usize))),
        preceded("?", dec_int).map(|i: i32| Guard::Alive(StreamReference::In(i as usize))),
        delimited("Global(", dec_uint, ")").map(|i| Guard::GlobalFreq(Duration::new(i, 0))),
        delimited("Local(", dec_uint, ")").map(Guard::LocalFreq),
        delimited(
            "FastAnd(",
            separated(.., dec_uint::<_, u32, _>, iws(",")),
            ")",
        )
        .map(|i: Vec<_>| {
            Guard::FastAnd(
                i.into_iter()
                    .map(|i| StreamReference::In(i as usize))
                    .collect(),
            )
        }),
        delimited("FastOr(", separated(.., dec_uint::<_, u32, _>, ","), ")").map(|i: Vec<_>| {
            Guard::FastOr(
                i.into_iter()
                    .map(|i| StreamReference::In(i as usize))
                    .collect(),
            )
        }),
        delimited("Expr(", cut_err(parse_expr), ")")
            .context(StrContext::Label("expr guard"))
            .map(Guard::Dynamic),
        delimited("(", parse_guard, ")"),
    )))
    .parse_next(input)
}

fn parse_expr(input: &mut &str) -> winnow::ModalResult<Expr> {
    let lhs = parse_expr_term.parse_next(input)?;
    let op = opt(iws(alt(("&&", "||", "==")))).parse_next(input)?;

    if let Some(op) = op {
        let rhs = parse_expr.parse_next(input)?;
        match op {
            "&&" => Ok(Expr {
                ty: Type::Bool,
                kind: ExprKind::BinaryOperation(Operator::And, Box::new(lhs), Box::new(rhs)),
            }),
            "||" => Ok(Expr {
                ty: Type::Bool,
                kind: ExprKind::BinaryOperation(Operator::Or, Box::new(lhs), Box::new(rhs)),
            }),
            "==" => Ok(Expr {
                ty: Type::Bool,
                kind: ExprKind::BinaryOperation(Operator::Eq, Box::new(lhs), Box::new(rhs)),
            }),
            _ => unreachable!(),
        }
    } else {
        Ok(lhs)
    }
}

fn parse_expr_term(input: &mut &str) -> winnow::ModalResult<Expr> {
    iws(alt((
        parse_bool,
        seq!(_: "p", dec_uint).map(|(p,): (u64,)| Expr {
            ty: Type::Bool,
            kind: ExprKind::ParameterAccess(StreamReference::In(0), p as usize),
        }),
        seq!(_: alt(("s", "o")), dec_uint).map(|(p,): (u64,)| Expr {
            ty: Type::Bool,
            kind: ExprKind::SyncStreamAccess {
                target: StreamReference::In(p as usize),
                parameters: vec![],
            },
        }),
        delimited("(", parse_expr, ")"),
    )))
    .parse_next(input)
}

fn parse_bool(input: &mut &str) -> winnow::ModalResult<Expr> {
    alt(("true", "false"))
        .map(|b| match b {
            "true" => true,
            "false" => false,
            _ => unreachable!(),
        })
        .map(|v| Expr {
            ty: Type::Bool,
            kind: ExprKind::Constant(Constant::Bool(v)),
        })
        .parse_next(input)
}

pub(crate) fn parse_stmt(mut s: &str) -> Stmt {
    match parse_stmt_prime.parse_next(&mut s) {
        Ok(stmt) => stmt,
        Err(e) => {
            println!("{e}");
            panic!()
        }
    }
}

pub(crate) fn parse_ir(s: &str) -> StreamIr {
    let stmt = parse_stmt(s);
    StreamIr {
        stmt,
        sr2memory: (0..10)
            .map(|i| (StreamReference::In(i), format!("i{i}")))
            .chain((0..10).map(|o| {
                (
                    StreamReference::Out(OutputReference::Unparameterized(o)),
                    format!("o{o}"),
                )
            }))
            .map(|(sr, name)| {
                (
                    sr,
                    Memory {
                        buffer: StreamMemory::Instances {
                            buffer: StreamBuffer::SingleValue,
                            parameter: (0..3)
                                .map(|i| Parameter {
                                    name: format!("p{i}"),
                                    ty: Type::Bool,
                                })
                                .collect(),
                        },
                        ty: Type::Bool,
                        name,
                    },
                )
            })
            .collect(),
        wref2window: HashMap::new(),
        lref2lfreq: (0..10)
            .map(|i| {
                (
                    i as usize,
                    LocalFreq {
                        dur: Duration::from_secs(i),
                        sr: OutputReference::Unparameterized(i as usize),
                        reference: i as usize,
                    },
                )
            })
            .collect(),
        livetime_equivalences: LivetimeEquivalences {
            idx: (0..10)
                .map(|o| (OutputReference::Unparameterized(o), o))
                .collect(),
            sets: DisjointSet::with_len(10),
            input_idx: 10,
        },
        static_schedule: None,
        triggers: HashMap::new(),
        accessed_by: HashMap::new(),
        accesses: HashMap::new(),
    }
}
