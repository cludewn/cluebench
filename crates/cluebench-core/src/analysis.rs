// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sound, deliberately incomplete analysis of one validated resolved scenario.

use std::fmt;

use crate::matching::{contains, matches_constraint, starts_with};
use crate::{Applicability, CharacterClass, Constraint, Id, ResolvedConstraint, ResolvedModel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisDomain {
    pub min_byte: u8,
    pub max_byte: u8,
    pub min_length: usize,
    pub max_length: usize,
}

/// Independent of seeds and any future candidate-generation alphabet.
pub const ANALYSIS_DOMAIN: AnalysisDomain = AnalysisDomain {
    min_byte: 0x20,
    max_byte: 0x7e,
    min_length: 1,
    max_length: 64,
};

// Position validation and the fixed analysis arrays share this domain bound.
const _: () = assert!(ANALYSIS_DOMAIN.max_length == crate::limits::MAX_CANDIDATE_BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub work_budget: u64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            work_budget: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisPhase {
    Extraction,
    Preprocessing,
    Deduction,
    WitnessConstruction,
    WitnessVerification,
    /// Reserve result packaging before the final verification or evidence copy.
    /// This is not a cancellation poll after a proof has already completed.
    ResultPreparation,
}

/// Called before reserving work. Returning true cancels; cancellation takes
/// precedence over insufficient budget at the same checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisCheckpoint {
    pub phase: AnalysisPhase,
    pub work_used: u64,
    pub next_cost: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    InferenceLimit,
    WorkBudgetExhausted,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContradictionCategory {
    LengthIntersection,
    PositionBeyondMaximumLength,
    PositionIntersection,
    ContainsNotContains,
    IncompatiblePrefixes,
    IncompatibleSuffixes,
    CharacterClassMinimums,
}

/// Constructed only after every active Hard constraint has been checked.
pub struct VerifiedWitness(String);

impl VerifiedWitness {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for VerifiedWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerifiedWitness([REDACTED])")
    }
}

/// Supporting effects borrow the validated input; no source strings are copied.
/// The enclosing result also borrows the exact resolved scenario.
pub struct Contradiction<'a> {
    category: ContradictionCategory,
    constraints: Vec<ResolvedConstraint<'a>>,
}

impl<'a> Contradiction<'a> {
    pub fn category(&self) -> ContradictionCategory {
        self.category
    }

    pub fn constraints(&self) -> &[ResolvedConstraint<'a>] {
        &self.constraints
    }

    pub fn effect_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.constraints.iter().map(|constraint| constraint.id)
    }
}

impl fmt::Debug for Contradiction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Contradiction")
            .field("category", &self.category)
            .field("effect_ids", &EffectIds(&self.constraints))
            .finish()
    }
}

struct EffectIds<'a, 'b>(&'a [ResolvedConstraint<'b>]);

impl fmt::Debug for EffectIds<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.iter().map(|c| c.id)).finish()
    }
}

#[derive(Debug)]
pub enum AnalysisOutcome<'a> {
    Satisfiable(VerifiedWitness),
    Unsatisfiable(Contradiction<'a>),
    Unknown(UnknownReason),
}

/// A result cannot be constructed or detached from its scenario through public
/// fields. Debug deliberately omits the model, literals, and candidate content.
pub struct AnalysisResult<'model, 'source> {
    model: &'model ResolvedModel<'source>,
    outcome: AnalysisOutcome<'source>,
    work_used: u64,
}

impl<'model, 'source> AnalysisResult<'model, 'source> {
    pub fn model(&self) -> &'model ResolvedModel<'source> {
        self.model
    }

    pub fn domain(&self) -> AnalysisDomain {
        ANALYSIS_DOMAIN
    }

    pub fn outcome(&self) -> &AnalysisOutcome<'source> {
        &self.outcome
    }

    pub fn work_used(&self) -> u64 {
        self.work_used
    }

    /// Read-only presentation traversal, outside analysis execution. Even an
    /// interrupted extraction still refers to the complete original scenario.
    pub fn hard_constraints(&self) -> impl Iterator<Item = ResolvedConstraint<'source>> + '_ {
        self.model
            .constraints()
            .iter()
            .copied()
            .filter(|c| c.hypothesis.applicability == Applicability::Hard)
    }
}

impl fmt::Debug for AnalysisResult<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnalysisResult")
            .field("domain", &ANALYSIS_DOMAIN)
            .field("outcome", &self.outcome)
            .field("work_used", &self.work_used)
            .finish()
    }
}

struct Work<C> {
    budget: u64,
    used: u64,
    cancel: C,
}

impl<C: FnMut(AnalysisCheckpoint) -> bool> Work<C> {
    fn spend(&mut self, phase: AnalysisPhase, cost: u64) -> Result<(), UnknownReason> {
        if (self.cancel)(AnalysisCheckpoint {
            phase,
            work_used: self.used,
            next_cost: cost,
        }) {
            return Err(UnknownReason::Cancelled);
        }
        if cost > self.budget - self.used {
            return Err(UnknownReason::WorkBudgetExhausted);
        }
        self.used += cost;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Prepared<'a> {
    source: ResolvedConstraint<'a>,
    allowed: u128,
}

impl<'source> ResolvedModel<'source> {
    /// Analyze this scenario only. The callback may poll an AtomicBool or cancel
    /// deterministically from checkpoint metadata. It must return promptly.
    /// Result preparation is reserved before final verification/evidence copy.
    /// No cancellation is observed after a proof or verified witness completes.
    pub fn analyze(
        &self,
        config: AnalysisConfig,
        cancel: impl FnMut(AnalysisCheckpoint) -> bool,
    ) -> AnalysisResult<'_, 'source> {
        let mut work = Work {
            budget: config.work_budget,
            used: 0,
            cancel,
        };
        let outcome = analyze(self, &mut work).unwrap_or_else(AnalysisOutcome::Unknown);
        AnalysisResult {
            model: self,
            outcome,
            work_used: work.used,
        }
    }
}

fn analyze<'a>(
    model: &ResolvedModel<'a>,
    work: &mut Work<impl FnMut(AnalysisCheckpoint) -> bool>,
) -> Result<AnalysisOutcome<'a>, UnknownReason> {
    use AnalysisPhase::*;
    // Reserve all slot capacity before allocation: pushes cannot reallocate.
    // Soft effects affect this reservation and scan cost, never logical rules.
    work.spend(Extraction, 1 + model.constraints().len() as u64)?;
    let mut hard = Vec::with_capacity(model.constraints().len());
    for source in model.constraints() {
        work.spend(Extraction, 1)?;
        if source.hypothesis.applicability == Applicability::Hard {
            hard.push(Prepared {
                source: *source,
                allowed: 0,
            });
        }
    }
    for entry in &mut hard {
        work.spend(Preprocessing, 1)?;
        if let Constraint::Position { allowed, .. } = entry.source.constraint {
            for byte in allowed.bytes() {
                work.spend(Preprocessing, 1)?;
                entry.allowed |= 1u128 << (byte - ANALYSIS_DOMAIN.min_byte);
            }
        }
    }

    let mut minimum = ANALYSIS_DOMAIN.min_length;
    let mut maximum = ANALYSIS_DOMAIN.max_length;
    let mut lower = None;
    let mut upper = None;
    for entry in &hard {
        work.spend(Deduction, 1)?;
        if let Constraint::Length { min, max } = entry.source.constraint {
            if *min > minimum {
                minimum = *min;
                lower = Some(entry.source);
            }
            if *max < maximum {
                maximum = *max;
                upper = Some(entry.source);
            }
            if minimum > maximum {
                return contradiction(
                    ContradictionCategory::LengthIntersection,
                    lower.into_iter().chain(upper),
                    2,
                    work,
                );
            }
        }
    }
    for entry in &hard {
        work.spend(Deduction, 1)?;
        if let Constraint::Position { index, .. } = entry.source.constraint
            && *index > maximum
        {
            return contradiction(
                ContradictionCategory::PositionBeyondMaximumLength,
                std::iter::once(entry.source).chain(upper),
                2,
                work,
            );
        }
    }

    work.spend(Deduction, 64)?;
    let mut positions = [u128::MAX; 64];
    for entry in &hard {
        work.spend(Deduction, 1)?;
        if let Constraint::Position { index, .. } = entry.source.constraint {
            positions[index - 1] &= entry.allowed;
            if positions[index - 1] == 0 {
                // All effects at this position are sufficient; preserving their
                // source order and identity also handles a three-way conflict.
                let support = hard
                    .iter()
                    .filter_map(|entry| match entry.source.constraint {
                        Constraint::Position { index: other, .. } if other == index => {
                            Some(entry.source)
                        }
                        _ => None,
                    });
                return contradiction(
                    ContradictionCategory::PositionIntersection,
                    support,
                    hard.len(),
                    work,
                );
            }
        }
    }

    for entry in &hard {
        work.spend(Deduction, 1)?;
        if let Constraint::Contains(required) = entry.source.constraint {
            for other in &hard {
                work.spend(Deduction, 1)?;
                if let Constraint::NotContains(forbidden) = other.source.constraint
                    && contains(required.as_bytes(), forbidden.as_bytes(), &mut |cost| {
                        work.spend(Deduction, cost)
                    })?
                {
                    return contradiction(
                        ContradictionCategory::ContainsNotContains,
                        [entry.source, other.source],
                        2,
                        work,
                    );
                }
            }
        }
    }
    for (index, entry) in hard.iter().enumerate() {
        work.spend(Deduction, 1)?;
        let (literal, suffix) = match entry.source.constraint {
            Constraint::Prefix(value) => (value, false),
            Constraint::Suffix(value) => (value, true),
            _ => continue,
        };
        for other in &hard[index + 1..] {
            work.spend(Deduction, 1)?;
            let value = match other.source.constraint {
                Constraint::Prefix(value) if !suffix => value,
                Constraint::Suffix(value) if suffix => value,
                _ => continue,
            };
            let (short, long) = if literal.len() <= value.len() {
                (literal.as_bytes(), value.as_bytes())
            } else {
                (value.as_bytes(), literal.as_bytes())
            };
            let aligned = if suffix {
                &long[long.len() - short.len()..]
            } else {
                long
            };
            if !starts_with(aligned, short, &mut |cost| work.spend(Deduction, cost))? {
                let category = if suffix {
                    ContradictionCategory::IncompatibleSuffixes
                } else {
                    ContradictionCategory::IncompatiblePrefixes
                };
                return contradiction(category, [entry.source, other.source], 2, work);
            }
        }
    }

    work.spend(Deduction, 5)?;
    let mut classes: [Option<(usize, ResolvedConstraint<'a>)>; 5] = [None; 5];
    for entry in &hard {
        work.spend(Deduction, 1)?;
        if let Constraint::CharacterCount { class, min, .. } = entry.source.constraint {
            let slot = &mut classes[class_index(*class)];
            if *min > slot.map_or(0, |(count, _)| count) {
                *slot = Some((*min, entry.source));
            }
        }
    }
    let mut total = 0;
    for class in &classes {
        work.spend(Deduction, 1)?;
        if let Some((minimum, _)) = class {
            total += minimum;
        }
    }
    if total > maximum {
        return contradiction(
            ContradictionCategory::CharacterClassMinimums,
            classes
                .into_iter()
                .flatten()
                .map(|(_, source)| source)
                .chain(upper),
            6,
            work,
        );
    }

    work.spend(WitnessConstruction, 1 + minimum as u64)?;
    let mut candidate = String::with_capacity(minimum);
    for _ in 0..minimum {
        work.spend(WitnessConstruction, 1)?;
        candidate.push(' ');
    }
    // The fixed construction establishes the domain. Verification still scans
    // every byte and every Hard effect under the same budget/cancellation guard.
    work.spend(ResultPreparation, 1)?;
    work.spend(WitnessVerification, 1)?;
    let mut valid = (1..=64).contains(&candidate.len());
    for byte in candidate.bytes() {
        work.spend(WitnessVerification, 1)?;
        valid &= (0x20..=0x7e).contains(&byte);
    }
    for entry in &hard {
        work.spend(WitnessVerification, 1)?;
        valid &= matches_constraint(entry.source.constraint, &candidate, &mut |cost| {
            work.spend(WitnessVerification, cost)
        })?;
    }
    Ok(if valid {
        AnalysisOutcome::Satisfiable(VerifiedWitness(candidate))
    } else {
        AnalysisOutcome::Unknown(UnknownReason::InferenceLimit)
    })
}

fn class_index(class: CharacterClass) -> usize {
    match class {
        CharacterClass::Lowercase => 0,
        CharacterClass::Uppercase => 1,
        CharacterClass::Digit => 2,
        CharacterClass::Space => 3,
        CharacterClass::Symbol => 4,
    }
}

fn contradiction<'a>(
    category: ContradictionCategory,
    support: impl IntoIterator<Item = ResolvedConstraint<'a>>,
    capacity: usize,
    work: &mut Work<impl FnMut(AnalysisCheckpoint) -> bool>,
) -> Result<AnalysisOutcome<'a>, UnknownReason> {
    // Reserve both storage and iterator traversal before any lazy filtering.
    // At most `capacity` source entries can be examined or copied here.
    work.spend(AnalysisPhase::Deduction, 1 + 2 * capacity as u64)?;
    let mut constraints = Vec::with_capacity(capacity);
    work.spend(AnalysisPhase::ResultPreparation, 1)?;
    for source in support {
        constraints.push(source);
    }
    Ok(AnalysisOutcome::Unsatisfiable(Contradiction {
        category,
        constraints,
    }))
}
