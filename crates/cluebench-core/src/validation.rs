// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::limits::*;
use crate::{Activation, Constraint, Effect, HypothesisOrigin, Id, RawProject};

/// Diagnostic payloads deliberately exclude clue, seed, and literal text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub entity: Option<Id>,
    pub field: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticCode {
    InvalidId,
    DuplicateId,
    MissingClue,
    MissingGroup,
    InvalidSelection,
    InvalidConstraint,
    InvalidSeed,
    EntityLimit,
    HypothesisLimit,
    EffectLimit,
    ReferenceLimit,
    StringLimit,
    TotalStringLimit,
    ArithmeticOverflow,
}

/// At most `MAX_DIAGNOSTICS` entries; `is_truncated` signals omitted errors.
#[derive(Debug)]
pub struct ValidationErrors {
    diagnostics: Vec<Diagnostic>,
    truncated: bool,
}

impl ValidationErrors {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            truncated: false,
        }
    }

    fn push(&mut self, code: DiagnosticCode, entity: Option<Id>, field: &'static str) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic {
                code,
                entity,
                field,
            });
        } else {
            self.truncated = true;
        }
    }

    fn single(code: DiagnosticCode, entity: Option<Id>, field: &'static str) -> Self {
        let mut errors = Self::new();
        errors.push(code, entity, field);
        errors
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "project validation failed ({} diagnostics",
            self.diagnostics.len()
        )?;
        if self.truncated {
            write!(f, ", further diagnostics omitted")?;
        }
        write!(f, ")")
    }
}

impl std::error::Error for ValidationErrors {}

/// Owns the complete source project with no mutable public access.
#[derive(Debug)]
pub struct ValidatedProject {
    pub(crate) raw: RawProject,
}

impl ValidatedProject {
    pub fn raw(&self) -> &RawProject {
        &self.raw
    }
}

impl RawProject {
    /// Consumes caller-owned input without copying strings. Preflight size checks
    /// run before any validation indexes are allocated.
    pub fn validate(self) -> Result<ValidatedProject, ValidationErrors> {
        preflight(&self)?;
        let mut errors = ValidationErrors::new();
        let mut ids = BTreeSet::new();
        let mut clues = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut hypotheses = BTreeMap::new();

        for clue in &self.clues {
            check_id(clue.id, &mut ids, &mut errors);
            clues.insert(clue.id);
        }
        for group in &self.alternative_groups {
            check_id(group.id, &mut ids, &mut errors);
            groups.insert(group.id);
        }
        for hypothesis in &self.hypotheses {
            check_id(hypothesis.id, &mut ids, &mut errors);
            hypotheses.insert(hypothesis.id, hypothesis);
            for effect in &hypothesis.effects {
                match effect {
                    Effect::Constraint(effect) => {
                        check_id(effect.id, &mut ids, &mut errors);
                        if !valid_constraint(&effect.constraint) {
                            errors.push(
                                DiagnosticCode::InvalidConstraint,
                                Some(effect.id),
                                "constraint",
                            );
                        }
                    }
                    Effect::Seed(seed) => {
                        check_id(seed.id, &mut ids, &mut errors);
                        if !valid_text(&seed.text, MAX_CANDIDATE_BYTES) {
                            errors.push(DiagnosticCode::InvalidSeed, Some(seed.id), "seed.text");
                        }
                    }
                }
            }
        }
        for seed in &self.manual_seeds {
            check_id(seed.id, &mut ids, &mut errors);
            if !valid_text(&seed.text, MAX_CANDIDATE_BYTES) {
                errors.push(DiagnosticCode::InvalidSeed, Some(seed.id), "seed.text");
            }
        }
        for hypothesis in &self.hypotheses {
            if let HypothesisOrigin::Clue(id) = hypothesis.origin
                && !clues.contains(&id)
            {
                errors.push(
                    DiagnosticCode::MissingClue,
                    Some(hypothesis.id),
                    "hypothesis.origin",
                );
            }
            if let Activation::Grouped { group_id } = hypothesis.activation
                && !groups.contains(&group_id)
            {
                errors.push(
                    DiagnosticCode::MissingGroup,
                    Some(hypothesis.id),
                    "hypothesis.activation",
                );
            }
        }
        for group in &self.alternative_groups {
            if let Some(selected) = group.selected {
                let valid = hypotheses.get(&selected).is_some_and(|hypothesis| {
                    hypothesis.activation == Activation::Grouped { group_id: group.id }
                });
                if !valid {
                    errors.push(
                        DiagnosticCode::InvalidSelection,
                        Some(group.id),
                        "group.selected",
                    );
                }
            }
        }
        if errors.diagnostics.is_empty() {
            Ok(ValidatedProject { raw: self })
        } else {
            Err(errors)
        }
    }
}

fn check_id(id: Id, ids: &mut BTreeSet<Id>, errors: &mut ValidationErrors) {
    if id.0 == 0 {
        errors.push(DiagnosticCode::InvalidId, Some(id), "id");
    }
    if !ids.insert(id) {
        errors.push(DiagnosticCode::DuplicateId, Some(id), "id");
    }
}

fn bounded_add(
    total: &mut usize,
    amount: usize,
    max: usize,
    code: DiagnosticCode,
    field: &'static str,
) -> Result<(), ValidationErrors> {
    let sum = total
        .checked_add(amount)
        .ok_or_else(|| ValidationErrors::single(DiagnosticCode::ArithmeticOverflow, None, field))?;
    if sum > max {
        return Err(ValidationErrors::single(code, None, field));
    }
    *total = sum;
    Ok(())
}

fn preflight(raw: &RawProject) -> Result<(), ValidationErrors> {
    let mut entities = 0;
    for count in [
        raw.clues.len(),
        raw.hypotheses.len(),
        raw.alternative_groups.len(),
        raw.manual_seeds.len(),
    ] {
        bounded_add(
            &mut entities,
            count,
            MAX_ENTITIES,
            DiagnosticCode::EntityLimit,
            "entities",
        )?;
    }
    if raw.hypotheses.len() > MAX_HYPOTHESES {
        return Err(ValidationErrors::single(
            DiagnosticCode::HypothesisLimit,
            None,
            "hypotheses",
        ));
    }
    let mut effects = 0;
    let mut edges = 0;
    for hypothesis in &raw.hypotheses {
        bounded_add(
            &mut effects,
            hypothesis.effects.len(),
            MAX_EFFECTS,
            DiagnosticCode::EffectLimit,
            "effects",
        )?;
        bounded_add(
            &mut entities,
            hypothesis.effects.len(),
            MAX_ENTITIES,
            DiagnosticCode::EntityLimit,
            "entities",
        )?;
        bounded_add(
            &mut edges,
            hypothesis.effects.len(),
            MAX_REFERENCE_EDGES,
            DiagnosticCode::ReferenceLimit,
            "references",
        )?;
        for has_edge in [
            matches!(hypothesis.origin, HypothesisOrigin::Clue(_)),
            matches!(hypothesis.activation, Activation::Grouped { .. }),
        ] {
            bounded_add(
                &mut edges,
                usize::from(has_edge),
                MAX_REFERENCE_EDGES,
                DiagnosticCode::ReferenceLimit,
                "references",
            )?;
        }
    }
    for group in &raw.alternative_groups {
        bounded_add(
            &mut edges,
            usize::from(group.selected.is_some()),
            MAX_REFERENCE_EDGES,
            DiagnosticCode::ReferenceLimit,
            "references",
        )?;
    }

    // String lengths are O(1); no text is scanned or cloned before its bound.
    let mut bytes = 0;
    let mut string = |value: &str, id: Id, field| {
        if value.len() > MAX_STRING_BYTES {
            return Err(ValidationErrors::single(
                DiagnosticCode::StringLimit,
                Some(id),
                field,
            ));
        }
        bounded_add(
            &mut bytes,
            value.len(),
            MAX_TOTAL_STRING_BYTES,
            DiagnosticCode::TotalStringLimit,
            "strings",
        )
    };
    for clue in &raw.clues {
        string(&clue.text, clue.id, "clue.text")?;
        string(&clue.source, clue.id, "clue.source")?;
    }
    for hypothesis in &raw.hypotheses {
        string(
            &hypothesis.interpretation,
            hypothesis.id,
            "hypothesis.interpretation",
        )?;
        for effect in &hypothesis.effects {
            match effect {
                Effect::Seed(seed) => string(&seed.text, seed.id, "seed.text")?,
                Effect::Constraint(effect) => match &effect.constraint {
                    Constraint::Position { allowed, .. } => {
                        string(allowed, effect.id, "position.allowed")?
                    }
                    Constraint::Contains(literal)
                    | Constraint::NotContains(literal)
                    | Constraint::Prefix(literal)
                    | Constraint::Suffix(literal) => {
                        string(literal, effect.id, "constraint.literal")?
                    }
                    Constraint::Length { .. } | Constraint::CharacterCount { .. } => {}
                },
            }
        }
    }
    for seed in &raw.manual_seeds {
        string(&seed.text, seed.id, "seed.text")?;
    }
    Ok(())
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|byte| (b' '..=b'~').contains(&byte))
}

fn valid_constraint(constraint: &Constraint) -> bool {
    match constraint {
        Constraint::Length { min, max } => 1 <= *min && min <= max && *max <= MAX_CANDIDATE_BYTES,
        Constraint::Position { index, allowed } => {
            (1..=MAX_CANDIDATE_BYTES).contains(index) && valid_text(allowed, MAX_STRING_BYTES)
        }
        Constraint::Contains(literal)
        | Constraint::NotContains(literal)
        | Constraint::Prefix(literal)
        | Constraint::Suffix(literal) => valid_text(literal, MAX_CANDIDATE_BYTES),
        Constraint::CharacterCount { min, max, .. } => min <= max && *max <= MAX_CANDIDATE_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_resource_arithmetic_reports_overflow() {
        let mut count = usize::MAX;
        let errors = bounded_add(
            &mut count,
            1,
            usize::MAX,
            DiagnosticCode::EntityLimit,
            "entities",
        )
        .unwrap_err();
        assert_eq!(
            errors.diagnostics()[0].code,
            DiagnosticCode::ArithmeticOverflow
        );
        assert_eq!(count, usize::MAX);
    }
}
