use std::collections::HashMap;

use disjoint::DisjointSet;
use rtlola_frontend::mir::{self, Close, OutputStream, Spawn, Stream};

use crate::ir::{OutputReference, StreamReference};

/// Contains information about the equivalences of stream livetimes
#[derive(Default, Clone)]
pub struct LivetimeEquivalences {
    pub(crate) idx: HashMap<OutputReference, usize>,
    pub(crate) sets: DisjointSet,
    pub(crate) input_idx: usize,
}

impl LivetimeEquivalences {
    pub(super) fn new(
        outputs: &[OutputStream],
        sr2sr: &HashMap<mir::StreamReference, StreamReference>,
    ) -> Self {
        let idx = outputs
            .iter()
            .enumerate()
            .map(|(i, o)| (sr2sr[&o.reference].out_idx(), i))
            .collect();

        let input_idx = outputs.len();
        let mut sets = DisjointSet::with_len(outputs.len() + 1);
        for (i, output) in outputs.iter().enumerate() {
            if !output.is_spawned() && !output.is_closed() {
                sets.join(i, input_idx);
                continue;
            }
            let Spawn {
                expression: i_spawn_expr,
                pacing: i_spawn_pacing,
                condition: i_spawn_condition,
            } = &output.spawn;
            let Close {
                condition: i_close_condition,
                pacing: i_close_pacing,
                has_self_reference: _,
            } = &output.close;
            for (j, other_output) in outputs.iter().enumerate().take(i) {
                let Spawn {
                    expression: j_spawn_expr,
                    pacing: j_spawn_pacing,
                    condition: j_spawn_condition,
                } = &other_output.spawn;
                let Close {
                    condition: j_close_condition,
                    pacing: j_close_pacing,
                    has_self_reference: _,
                } = &other_output.close;
                if compare_expr_option(j_spawn_expr.as_ref(), i_spawn_expr.as_ref())
                    && compare_expr_option(j_spawn_condition.as_ref(), i_spawn_condition.as_ref())
                    && j_spawn_pacing == i_spawn_pacing
                    && compare_expr_option(j_close_condition.as_ref(), i_close_condition.as_ref())
                    && j_close_pacing == i_close_pacing
                {
                    sets.join(i, j);
                    break;
                }
            }
        }

        Self {
            idx,
            sets,
            input_idx,
        }
    }

    /// Returns whether two stream's livetimes are equivalent
    pub fn is_equivalent(&self, sr1: StreamReference, sr2: StreamReference) -> bool {
        match (sr1, sr2) {
            (StreamReference::In(_), StreamReference::In(_)) => true,
            (StreamReference::Out(output_reference), StreamReference::In(_))
            | (StreamReference::In(_), StreamReference::Out(output_reference)) => self
                .sets
                .is_joined(self.input_idx, self.idx[&output_reference]),
            (StreamReference::Out(sr1), StreamReference::Out(sr2)) => {
                self.is_equivalent_outputs(sr1, sr2)
            }
        }
    }

    /// Returns whether two output's livetimes are equivalent
    pub fn is_equivalent_outputs(&self, sr1: OutputReference, sr2: OutputReference) -> bool {
        let i1 = self.idx[&sr1];
        let i2 = self.idx[&sr2];
        self.sets.is_joined(i1, i2)
    }

    /// Returns whether the given stream is static, i.e. lives for the whole runtime of the program
    pub fn is_static(&self, sr: StreamReference) -> bool {
        match sr {
            StreamReference::In(_) => true,
            StreamReference::Out(sr) => self.sets.is_joined(self.idx[&sr], self.input_idx),
        }
    }
}

impl std::fmt::Debug for LivetimeEquivalences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sets = self.sets.sets();
        let sets = sets
            .into_iter()
            .map(|set| {
                set.into_iter()
                    .map(|s| {
                        self.idx
                            .iter()
                            .find_map(|(k, v)| (*v == s).then_some(k))
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        sets.fmt(f)
    }
}

fn compare_expr_option(e1: Option<&mir::Expression>, e2: Option<&mir::Expression>) -> bool {
    match (e1, e2) {
        (Some(e1), Some(e2)) => compare_expr(e1, e2),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn compare_expr(e1: &mir::Expression, e2: &mir::Expression) -> bool {
    match (&e1.kind, &e2.kind) {
        (mir::ExpressionKind::LoadConstant(c1), mir::ExpressionKind::LoadConstant(c2)) => c1 == c2,
        (mir::ExpressionKind::ArithLog(op1, e1), mir::ExpressionKind::ArithLog(op2, e2)) => {
            op1 == op2
                && e1
                    .iter()
                    .zip(e2.iter())
                    .all(|(e1, e2)| compare_expr(e1, e2))
        }
        (
            mir::ExpressionKind::StreamAccess {
                target: t1,
                parameters: e1,
                access_kind: ak1,
            },
            mir::ExpressionKind::StreamAccess {
                target: t2,
                parameters: e2,
                access_kind: ak2,
            },
        ) => {
            t1 == t2
                && ak1 == ak2
                && e1
                    .iter()
                    .zip(e2.iter())
                    .all(|(e1, e2)| compare_expr(e1, e2))
        }
        (
            mir::ExpressionKind::ParameterAccess(_, i1),
            mir::ExpressionKind::ParameterAccess(_, i2),
        ) => i1 == i2, // purposely ignore stream reference,
        (
            mir::ExpressionKind::LambdaParameterAccess { wref: w1, pref: p1 },
            mir::ExpressionKind::LambdaParameterAccess { wref: w2, pref: p2 },
        ) => w1 == w2 && p1 == p2,
        (
            mir::ExpressionKind::Ite {
                condition: cond1,
                consequence: cons1,
                alternative: alt1,
            },
            mir::ExpressionKind::Ite {
                condition: cond2,
                consequence: cons2,
                alternative: alt2,
            },
        ) => compare_expr(cond1, cond2) && compare_expr(cons1, cons2) && compare_expr(alt1, alt2),
        (mir::ExpressionKind::Tuple(e1), mir::ExpressionKind::Tuple(e2)) => e1
            .iter()
            .zip(e2.iter())
            .all(|(e1, e2)| compare_expr(e1, e2)),
        (
            mir::ExpressionKind::TupleAccess(e1, idx1),
            mir::ExpressionKind::TupleAccess(e2, idx2),
        ) => compare_expr(e1, e2) && idx1 == idx2,
        (mir::ExpressionKind::Function(f1, e1), mir::ExpressionKind::Function(f2, e2)) => {
            f1 == f2
                && e1
                    .iter()
                    .zip(e2.iter())
                    .all(|(e1, e2)| compare_expr(e1, e2))
        }
        (mir::ExpressionKind::Convert { expr: e1 }, mir::ExpressionKind::Convert { expr: e2 }) => {
            compare_expr(e1, e2)
        }
        (
            mir::ExpressionKind::Default {
                expr: e1,
                default: d1,
            },
            mir::ExpressionKind::Default {
                expr: e2,
                default: d2,
            },
        ) => compare_expr(e1, e2) && compare_expr(d1, d2),
        _ => false,
    }
}
