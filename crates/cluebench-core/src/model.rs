// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/// A stable, project-local identifier. Validation requires nonzero, globally
/// unique values across clues, hypotheses, groups, constraints, and seeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u32);

/// Caller-owned editable data. Every stored entity is checked by `validate`,
/// including disabled hypotheses and unselected alternatives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawProject {
    pub clues: Vec<Clue>,
    pub hypotheses: Vec<Hypothesis>,
    pub alternative_groups: Vec<AlternativeGroup>,
    pub manual_seeds: Vec<Seed>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clue {
    pub id: Id,
    pub text: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    pub id: Id,
    pub origin: HypothesisOrigin,
    pub interpretation: String,
    pub confidence: Confidence,
    pub applicability: Applicability,
    pub activation: Activation,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypothesisOrigin {
    Manual,
    Clue(Id),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    /// Relative points per fully matched Soft hypothesis, never a probability.
    pub const fn weight(self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    Hard,
    Soft,
}

/// Group membership and an independent enabled flag are mutually exclusive.
/// Membership has a single source of truth here, not a second member list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    Independent { enabled: bool },
    Grouped { group_id: Id },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeGroup {
    pub id: Id,
    /// Scenario selection, not a logical OR. `None` activates no member.
    pub selected: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Constraint(ConstraintEffect),
    Seed(Seed),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintEffect {
    pub id: Id,
    pub constraint: Constraint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub id: Id,
    pub text: String,
}

/// Editable constraint values. Only a validated, resolved model can match them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    Length {
        min: usize,
        max: usize,
    },
    Position {
        index: usize,
        allowed: String,
    },
    Contains(String),
    NotContains(String),
    Prefix(String),
    Suffix(String),
    CharacterCount {
        class: CharacterClass,
        min: usize,
        max: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterClass {
    Lowercase,
    Uppercase,
    Digit,
    Space,
    Symbol,
}
