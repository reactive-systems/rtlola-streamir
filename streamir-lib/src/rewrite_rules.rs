//! A framework for optimizing the StreamIR through rewriting rules
//! Applies the rewriting rules alternating until no rule does any changes anymore.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::{Add, AddAssign},
};

use thiserror::Error;

use crate::ir::{
    memory::{Memory, StreamBuffer, StreamMemory},
    Guard, IfStmt, LivetimeEquivalences, Stmt, StreamIr, StreamReference,
};
mod common_guards_outside;
pub use common_guards_outside::MoveCommonGuardsOutside;
mod remove_close;
pub use remove_close::RemoveClose;
mod remove_spawn;
pub use remove_spawn::RemoveSpawn;
mod assign;
pub use assign::IterateAssign;
mod combine_if;
pub use combine_if::CombineIf;
mod combine_iterate;
pub use combine_iterate::CombineIterate;
mod combine_seq;
pub use combine_seq::CombineSeq;
mod fast_guards;
pub use fast_guards::FastGuards;
mod if_outside;
pub use if_outside::MoveIfOutside;
mod implied_guards;
pub use implied_guards::ImpliedGuards;
mod memory_optimizations;
pub use memory_optimizations::MemoryOptimizations;
mod nested_ifs;
pub use nested_ifs::CombineNestedIf;
mod partial_evaluation;
pub use partial_evaluation::EvaluateGuards;
mod remove_ifs;
pub use remove_ifs::RemoveIfs;
mod remove_shift;
pub use remove_shift::RemoveShift;
mod simplify_guard;
pub use simplify_guard::SimplifyGuard;
mod skip;
pub use skip::RemoveSkip;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A global change that can be a side effect from a rewriting rule
enum GlobalChangeInstruction {
    /// A change to memory
    #[allow(dead_code)]
    ReplaceMemory(StreamReference, Memory),
}

impl GlobalChangeInstruction {
    fn apply(self, ir: &mut StreamIr) {
        match self {
            GlobalChangeInstruction::ReplaceMemory(sr, memory) => {
                *ir.sr2memory.get_mut(&sr).unwrap() = memory
            }
        }
    }
}

type GlobalChangeSet = HashSet<GlobalChangeInstruction>;

#[derive(Debug, Clone, Default)]
/// The result from applying a rewriting rule.
///
/// Does indicate whether the rewriting rule changed something and holds the global changes
pub struct ChangeSet {
    local_change: bool,
    global_instructions: GlobalChangeSet,
}

impl ChangeSet {
    fn local_change() -> ChangeSet {
        ChangeSet {
            local_change: true,
            global_instructions: HashSet::new(),
        }
    }
}

impl Add<ChangeSet> for ChangeSet {
    type Output = ChangeSet;

    fn add(self, rhs: ChangeSet) -> Self::Output {
        let global_instructions = self
            .global_instructions
            .union(&rhs.global_instructions)
            .cloned()
            .collect();
        Self {
            local_change: self.local_change || rhs.local_change,
            global_instructions,
        }
    }
}

impl AddAssign for ChangeSet {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: Self) {
        let ChangeSet {
            local_change,
            global_instructions,
        } = rhs;
        self.global_instructions.extend(global_instructions);
        self.local_change |= local_change;
    }
}

#[derive(Error, Debug)]
/// An error that can occour during rewriting
pub enum RewriteError {
    #[error("other error: {0}")]
    /// An error that does not fit any of the other categories
    Other(String),
}

/// A trait representing a rewriting rule
/// Desribes rewriting to different parts of the StreamIR.
pub trait RewriteRule: std::fmt::Debug {
    /// Rewrite a statement.
    /// Is called recursively for all children automatically.
    fn rewrite_stmt(
        &self,
        stmt: Stmt,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        Ok((stmt, ChangeSet::default()))
    }

    /// Rewrite a guard.
    /// Is called recursively for all children automatically.
    fn rewrite_guard(
        &self,
        guard: Guard,
        _memory: &HashMap<StreamReference, Memory>,
        _liveness_equivalences: &LivetimeEquivalences,
    ) -> Result<(Guard, ChangeSet), RewriteError> {
        Ok((guard, ChangeSet::default()))
    }

    /// Rewrites the memory of a stream.
    fn rewrite_memory(
        &self,
        _sr: StreamReference,
        memory: StreamMemory,
    ) -> Result<(StreamMemory, ChangeSet), RewriteError> {
        Ok((memory, ChangeSet::default()))
    }

    /// Rewrites the buffer of the memory of a stream.
    fn rewrite_buffer(
        &self,
        _sr: StreamReference,
        memory: StreamBuffer,
    ) -> Result<(StreamBuffer, ChangeSet), RewriteError> {
        Ok((memory, ChangeSet::default()))
    }

    /// Rewrites the top level statement.
    /// Is NOT called automatically for all children.
    fn apply_stmt(
        &self,
        mut stmt: Stmt,
        memory: &HashMap<StreamReference, Memory>,
        livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(Stmt, ChangeSet), RewriteError> {
        let mut cs = ChangeSet::default();
        stmt = match stmt {
            old @ (Stmt::Skip
            | Stmt::Input(_)
            | Stmt::Shift(_)
            | Stmt::Spawn { .. }
            | Stmt::Eval { .. }
            | Stmt::Close { .. }) => old,
            Stmt::Seq(stmts) => {
                let inner = stmts
                    .into_iter()
                    .map(|stmt| {
                        let (stmt, c) = self.apply_stmt(stmt, memory, livetime_equivalences)?;
                        cs += c;
                        Ok(stmt)
                    })
                    .collect::<Result<_, _>>()?;
                Stmt::Seq(inner)
            }
            Stmt::Parallel(stmts) => {
                let inner = stmts
                    .into_iter()
                    .map(|stmt| {
                        let (stmt, c) = self.apply_stmt(stmt, memory, livetime_equivalences)?;
                        cs += c;
                        Ok(stmt)
                    })
                    .collect::<Result<_, _>>()?;
                Stmt::Parallel(inner)
            }
            Stmt::If(IfStmt { guard, cons, alt }) => {
                let (guard, guard_cs) = self.apply_guard(guard, memory, livetime_equivalences)?;
                let (cons, cons_cs) = self.apply_stmt(*cons, memory, livetime_equivalences)?;
                let (alt, alt_cs) = self.apply_stmt(*alt, memory, livetime_equivalences)?;
                cs += guard_cs + cons_cs + alt_cs;
                Stmt::If(IfStmt {
                    guard,
                    cons: Box::new(cons),
                    alt: Box::new(alt),
                })
            }
            Stmt::Iterate { sr, stmt } => {
                let (stmt, c) = self.apply_stmt(*stmt, memory, livetime_equivalences)?;
                cs += c;
                Stmt::Iterate {
                    sr,
                    stmt: Box::new(stmt),
                }
            }
            Stmt::Assign {
                parameter_expr,
                sr,
                stmt,
            } => {
                let (stmt, c) = self.apply_stmt(*stmt, memory, livetime_equivalences)?;
                cs += c;
                Stmt::Assign {
                    parameter_expr,
                    sr,
                    stmt: Box::new(stmt),
                }
            }
        };
        let (new_stmt, cur_cs) = self.rewrite_stmt(stmt, memory, livetime_equivalences)?;
        stmt = new_stmt;
        cs += cur_cs;
        Ok((stmt, cs))
    }

    /// Rewrites the top level guard.
    /// Is NOT called automatically for all children.
    fn apply_guard(
        &self,
        guard: Guard,
        memory: &HashMap<StreamReference, Memory>,
        livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(Guard, ChangeSet), RewriteError> {
        let mut cs = ChangeSet::default();
        let mut guard = match guard {
            Guard::And { lhs, rhs } => {
                let (lhs, cs1) = self.apply_guard(*lhs, memory, livetime_equivalences)?;
                let (rhs, cs2) = self.apply_guard(*rhs, memory, livetime_equivalences)?;
                cs += cs1 + cs2;
                Guard::And {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            Guard::Or { lhs, rhs } => {
                let (lhs, cs1) = self.apply_guard(*lhs, memory, livetime_equivalences)?;
                let (rhs, cs2) = self.apply_guard(*rhs, memory, livetime_equivalences)?;
                cs += cs1 + cs2;
                Guard::Or {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            other => other,
        };

        let (new_guard, cur_cs) = self.rewrite_guard(guard, memory, livetime_equivalences)?;
        guard = new_guard;
        cs += cur_cs;

        Ok((guard, cs))
    }

    /// Rewrites the whole memory.
    fn apply_memory(
        &self,
        memory: HashMap<StreamReference, Memory>,
        _livetime_equivalences: &LivetimeEquivalences,
    ) -> Result<(HashMap<StreamReference, Memory>, ChangeSet), RewriteError> {
        let mut cs = ChangeSet::default();
        let new_memory = memory
            .into_iter()
            .map(|(sr, Memory { buffer, ty, name })| {
                let (new_buffer, cur_cs) = self.rewrite_memory(sr, buffer)?;
                let new_buffer = match new_buffer {
                    StreamMemory::NoMemory => StreamMemory::NoMemory,
                    StreamMemory::Static(buffer) => {
                        let (new_buffer, cur_cs) = self.rewrite_buffer(sr, buffer)?;
                        cs += cur_cs;
                        StreamMemory::Static(new_buffer)
                    }
                    StreamMemory::Dynamic {
                        buffer,
                        has_spawn,
                        has_close,
                    } => {
                        let (new_buffer, cur_cs) = self.rewrite_buffer(sr, buffer)?;
                        cs += cur_cs;
                        StreamMemory::Dynamic {
                            buffer: new_buffer,
                            has_spawn,
                            has_close,
                        }
                    }
                    StreamMemory::Instances { buffer, parameter } => {
                        let (new_buffer, cur_cs) = self.rewrite_buffer(sr, buffer)?;
                        cs += cur_cs;
                        StreamMemory::Instances {
                            buffer: new_buffer,
                            parameter,
                        }
                    }
                };
                cs += cur_cs;
                Ok((
                    sr,
                    Memory {
                        buffer: new_buffer,
                        ty,
                        name,
                    },
                ))
            })
            .collect::<Result<_, _>>()?;
        Ok((new_memory, cs))
    }

    /// A set of rewrite rules that are applied directly after the given rewrite rules
    fn cleanup_rules(&self) -> Vec<Box<dyn RewriteRule>> {
        Vec::new()
    }
}

#[derive(Debug)]
/// A rewriter that holds a set of rewriting rules and can apply them to StreamIR's.
pub struct Rewriter {
    rules: Vec<Box<dyn RewriteRule>>,
}

impl Rewriter {
    /// Construct a new Rewriter with the given rules.
    pub fn new(rules: Vec<Box<dyn RewriteRule>>) -> Self {
        let original_length = rules.len();
        let mut stack = rules
            .into_iter()
            .rev()
            .collect::<VecDeque<Box<dyn RewriteRule>>>();
        let mut rules = Vec::new();
        while let Some(rule) = stack.pop_back() {
            let cleanup = rule.cleanup_rules();
            rules.push(rule);
            stack.extend(cleanup);
            if rules.len() > original_length * 10 {
                panic!("possible infinite loop in rewrite rule expansion")
            }
        }
        Self { rules }
    }

    /// Run the rewriting on the given StreamIR until a fixedpoint is reached.
    pub fn run(&self, mut ir: StreamIr) -> Result<StreamIr, RewriteError> {
        let mut changed = true;
        while changed {
            (ir, changed) = self.apply(ir)?;
        }
        Ok(ir)
    }

    /// Apply all rewriting rules once. Returns the resulting StreamIR and a boolean indicating
    /// whether something changed for any of the rules.
    fn apply(&self, mut ir: StreamIr) -> Result<(StreamIr, bool), RewriteError> {
        let mut changed = false;
        for rule in &self.rules {
            let StreamIr {
                stmt,
                sr2memory,
                wref2window,
                lref2lfreq,
                livetime_equivalences,
                static_schedule,
                triggers,
                accesses,
                accessed_by,
            } = ir;

            let (sr2memory, cs_memory) = rule.apply_memory(sr2memory, &livetime_equivalences)?;
            let (stmt, cs_stmt) = rule.apply_stmt(stmt, &sr2memory, &livetime_equivalences)?;

            ir = StreamIr {
                stmt,
                sr2memory,
                wref2window,
                lref2lfreq,
                livetime_equivalences,
                static_schedule,
                triggers,
                accesses,
                accessed_by,
            };
            let ChangeSet {
                local_change,
                global_instructions,
            } = cs_memory + cs_stmt;
            changed |= local_change || !global_instructions.is_empty();
            for i in global_instructions {
                i.apply(&mut ir);
            }
        }
        Ok((ir, changed))
    }
}
