use std::collections::HashMap;

use crate::{
    ir::{memory::Memory, Guard, LivetimeEquivalences, StreamReference},
    rewrite_rules::{RemoveIfs, RemoveSkip, SimplifyGuard},
};

use super::{ChangeSet, RewriteRule};

type GuardPredicate = Box<dyn Fn(&Guard) -> Option<bool>>;

/// Allows for partially evaluating the guard conditions.
pub struct EvaluateGuards(GuardPredicate);

impl std::fmt::Debug for EvaluateGuards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EvaluateGuards").finish()
    }
}

impl RewriteRule for EvaluateGuards {
    fn rewrite_guard(
        &self,
        guard: Guard,
        _memory: &HashMap<StreamReference, Memory>,
        _livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(Guard, ChangeSet), super::RewriteError> {
        if matches!(guard, Guard::Constant(_)) {
            return Ok((guard, ChangeSet::default()));
        }
        match self.0(&guard) {
            Some(b) => Ok((Guard::Constant(b), ChangeSet::local_change())),
            None => Ok((guard, ChangeSet::default())),
        }
    }

    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        vec![
            Box::new(SimplifyGuard),
            Box::new(RemoveIfs),
            Box::new(RemoveSkip),
        ]
    }
}

impl EvaluateGuards {
    /// Remove all guards that will be never satisfied for an event-based event.
    pub fn only_event_based() -> Self {
        Self(Box::new(|g| match g {
            Guard::LocalFreq(_) | Guard::GlobalFreq(_) => Some(false),
            _ => None,
        }))
    }

    /// Remove all guards that will be never satisfied for an time-based event.
    pub fn only_time_based() -> Self {
        Self(Box::new(|g| match g {
            Guard::Stream(_) | Guard::FastAnd(_) | Guard::FastOr(_) => Some(false),
            _ => None,
        }))
    }

    /// Partially evaluate the activation conditions for the case where only but all `streams` receive a new value.
    pub fn only_streams(streams: Vec<StreamReference>) -> Self {
        Self(Box::new(move |g| match g {
            Guard::Stream(s) => {
                if streams.contains(s) {
                    Some(true)
                } else {
                    Some(false)
                }
            }
            _ => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::{ir::parse::parse_ir, rewrite_rules::Rewriter};

    use super::EvaluateGuards;

    #[test]
    fn test() {
        let ir = parse_ir(
            "if @0 || Global(1) then
				input 0	
			fi",
        );
        let reference = parse_ir(
            "if @0 then
				input 0	
			fi",
        );
        let rewriter = Rewriter::new(vec![Box::new(EvaluateGuards::only_event_based())]);
        let (res, changed) = rewriter.apply(ir).unwrap();
        assert!(changed);
        assert!(res.stmt.eq(&reference.stmt));
    }
}
