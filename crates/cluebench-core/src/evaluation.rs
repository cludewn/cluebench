// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

use crate::limits::{MAX_CANDIDATE_BYTES, MAX_HYPOTHESES, MAX_SOFT_POINTS};
use crate::{Applicability, CharacterClass, Constraint, Effect, Id, ResolvedModel};

// Changing the population limit requires updating the documented score bound.
const _: () = assert!(MAX_HYPOTHESES <= u32::MAX as usize / 3);
const _: () = assert!(MAX_SOFT_POINTS as usize == MAX_HYPOTHESES * 3);

/// Relative assessment of one supported candidate, independent of seed effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    pub hard_eligible: bool,
    /// Matched Soft hypotheses in their original project order.
    pub matched_soft_hypotheses: Vec<Id>,
    /// Relative points, never a calibrated probability.
    pub relative_points: u32,
}

/// Unsupported input, distinct from a supported candidate failing a constraint.
/// No variant retains candidate text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateError {
    Empty,
    TooLong {
        length: usize,
    },
    /// Zero-based byte offset, unlike one-based constraint positions.
    UnsupportedByte {
        index: usize,
    },
}

impl fmt::Display for CandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("candidate must not be empty"),
            Self::TooLong { length } => {
                write!(
                    formatter,
                    "candidate length {length} exceeds {MAX_CANDIDATE_BYTES} bytes"
                )
            }
            Self::UnsupportedByte { index } => {
                write!(formatter, "unsupported candidate byte at offset {index}")
            }
        }
    }
}

impl std::error::Error for CandidateError {}

impl ResolvedModel<'_> {
    /// Evaluate a printable ASCII candidate of 1 through 64 bytes.
    ///
    /// Validation and resolution are the only construction path to this model.
    /// No trimming, case conversion, or Unicode normalization is performed.
    pub fn evaluate(&self, candidate: &str) -> Result<Evaluation, CandidateError> {
        if candidate.is_empty() {
            return Err(CandidateError::Empty);
        }
        if candidate.len() > MAX_CANDIDATE_BYTES {
            return Err(CandidateError::TooLong {
                length: candidate.len(),
            });
        }
        if let Some(index) = candidate
            .bytes()
            .position(|byte| !(0x20..=0x7e).contains(&byte))
        {
            return Err(CandidateError::UnsupportedByte { index });
        }

        let mut result = Evaluation {
            hard_eligible: true,
            matched_soft_hypotheses: Vec::new(),
            relative_points: 0,
        };
        for hypothesis in &self.active_hypotheses {
            let mut has_constraints = false;
            let mut all_match = true;
            for effect in &hypothesis.effects {
                if let Effect::Constraint(effect) = effect {
                    has_constraints = true;
                    all_match &= matches_constraint(&effect.constraint, candidate);
                }
            }
            match hypothesis.applicability {
                Applicability::Hard => result.hard_eligible &= all_match,
                Applicability::Soft if has_constraints && all_match => {
                    result.matched_soft_hypotheses.push(hypothesis.id);
                    // Validation admits at most 1,024 hypotheses, each worth at
                    // most 3 points: the largest total is 3,072, within u32.
                    result.relative_points += hypothesis.confidence.weight();
                }
                Applicability::Soft => {}
            }
        }
        Ok(result)
    }
}

fn matches_constraint(constraint: &Constraint, candidate: &str) -> bool {
    match constraint {
        Constraint::Length { min, max } => (*min..=*max).contains(&candidate.len()),
        Constraint::Position { index, allowed } => candidate
            .as_bytes()
            .get(index - 1)
            .is_some_and(|byte| allowed.as_bytes().contains(byte)),
        Constraint::Contains(literal) => candidate.contains(literal.as_str()),
        Constraint::NotContains(literal) => !candidate.contains(literal.as_str()),
        Constraint::Prefix(literal) => candidate.starts_with(literal),
        Constraint::Suffix(literal) => candidate.ends_with(literal),
        Constraint::CharacterCount { class, min, max } => {
            let count = candidate
                .bytes()
                .filter(|byte| character_class(*byte) == *class)
                .count();
            (*min..=*max).contains(&count)
        }
    }
}

fn character_class(byte: u8) -> CharacterClass {
    match byte {
        b'a'..=b'z' => CharacterClass::Lowercase,
        b'A'..=b'Z' => CharacterClass::Uppercase,
        b'0'..=b'9' => CharacterClass::Digit,
        b' ' => CharacterClass::Space,
        _ => CharacterClass::Symbol,
    }
}
