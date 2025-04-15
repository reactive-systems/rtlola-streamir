use std::collections::HashMap;

use crate::{
    ir::{memory::Memory, Guard, LivetimeEquivalences, StreamReference},
    rewrite_rules::SimplifyGuard,
};

use super::{ChangeSet, RewriteRule};

#[derive(Debug, Clone, Copy)]
/// A rewriting rule that allows for optimized implementations of guards by replacing
/// conjunctions (or disjunctions) of streams with the Guard::FastAnd (or Guard::FastOr) variant.
pub struct FastGuards;

impl RewriteRule for FastGuards {
    fn rewrite_guard(
        &self,
        guard: Guard,
        _memory: &HashMap<StreamReference, Memory>,
        _livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(Guard, ChangeSet), super::RewriteError> {
        let (guard, changed) = match guard {
            Guard::And { lhs, rhs } => match (*lhs, *rhs) {
                (Guard::Stream(sr1), Guard::Stream(sr2)) => (Guard::FastAnd(vec![sr1, sr2]), true),
                (Guard::FastAnd(mut other), Guard::Stream(sr))
                | (Guard::Stream(sr), Guard::FastAnd(mut other)) => {
                    other.push(sr);
                    (Guard::FastAnd(other), true)
                }
                (Guard::FastAnd(mut lhs), Guard::FastAnd(rhs)) => {
                    lhs.extend(rhs);
                    (Guard::FastAnd(lhs), true)
                }
                (lhs, rhs) => (
                    Guard::And {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    false,
                ),
            },
            Guard::Or { lhs, rhs } => match (*lhs, *rhs) {
                (Guard::Stream(sr1), Guard::Stream(sr2)) => (Guard::FastOr(vec![sr1, sr2]), true),
                (Guard::FastOr(mut other), Guard::Stream(sr))
                | (Guard::Stream(sr), Guard::FastOr(mut other)) => {
                    other.push(sr);
                    (Guard::FastOr(other), true)
                }
                (Guard::FastOr(mut lhs), Guard::FastOr(rhs)) => {
                    lhs.extend(rhs);
                    (Guard::FastOr(lhs), true)
                }
                (lhs, rhs) => (
                    Guard::Or {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    false,
                ),
            },
            other => (other, false),
        };
        if changed {
            Ok((guard, ChangeSet::local_change()))
        } else {
            Ok((guard, ChangeSet::default()))
        }
    }

    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        vec![Box::new(SimplifyGuard)]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        ir::{parse::parse_guard, LivetimeEquivalences},
        rewrite_rules::{fast_guards::FastGuards, RewriteRule},
        SimplifyGuard,
    };

    #[test]
    fn simple() {
        let guard = parse_guard(&mut "@0 && (@1 && @2) && @3").unwrap();
        let reference = parse_guard(&mut "FastAnd(0, 1, 2, 3)").unwrap();
        let (guard, cs1) = FastGuards {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        let (res, cs2) = SimplifyGuard {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        assert_eq!(res, reference);
        assert!((cs1 + cs2).local_change);
    }

    #[test]
    fn simple2() {
        let guard = parse_guard(&mut "Expr(s0) && (@1 && @2) && @3").unwrap();
        let reference = parse_guard(&mut "Expr(s0) && FastAnd(1,2,3)").unwrap();
        let (guard, cs1) = FastGuards {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        let (res, cs2) = SimplifyGuard {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        assert_eq!(res, reference);
        assert!((cs1 + cs2).local_change);
    }

    #[test]
    fn simple3() {
        let guard = parse_guard(&mut "@0 && (@1 || @2)").unwrap();
        let reference = parse_guard(&mut "@0 && FastOr(1,2)").unwrap();
        let (guard, cs1) = FastGuards {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        let (res, cs2) = SimplifyGuard {}
            .apply_guard(guard, &HashMap::new(), &LivetimeEquivalences::default())
            .unwrap();
        assert_eq!(res, reference);
        assert!((cs1 + cs2).local_change);
    }
}
