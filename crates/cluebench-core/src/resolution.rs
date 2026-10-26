// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeMap;

use crate::{Activation, Constraint, Effect, Hypothesis, Id, Seed, ValidatedProject};

/// An active constraint with its stable effect identity and complete origin.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedConstraint<'a> {
    pub id: Id,
    pub constraint: &'a Constraint,
    pub hypothesis: &'a Hypothesis,
}

/// `None` denotes a manual seed; otherwise the originating hypothesis retains
/// its interpretation, applicability, and optional clue reference.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedSeed<'a> {
    pub seed: &'a Seed,
    pub hypothesis: Option<&'a Hypothesis>,
}

/// A read-only scenario borrowing validated source data. No source entity or
/// string is copied or deduplicated, and no public constructor bypasses checks.
#[derive(Debug)]
pub struct ResolvedModel<'a> {
    pub(crate) active_hypotheses: Vec<&'a Hypothesis>,
    pub(crate) constraints: Vec<ResolvedConstraint<'a>>,
    pub(crate) seeds: Vec<ResolvedSeed<'a>>,
}

impl ValidatedProject {
    /// Resolves selection only; contradictory valid constraints stay present.
    /// Validation has already bounded every index and output-vector size.
    pub fn resolve(&self) -> ResolvedModel<'_> {
        let groups: BTreeMap<_, _> = self
            .raw
            .alternative_groups
            .iter()
            .map(|group| (group.id, group.selected))
            .collect();
        let mut model = ResolvedModel {
            active_hypotheses: Vec::new(),
            constraints: Vec::new(),
            seeds: Vec::new(),
        };
        model
            .seeds
            .extend(self.raw.manual_seeds.iter().map(|seed| ResolvedSeed {
                seed,
                hypothesis: None,
            }));
        for hypothesis in &self.raw.hypotheses {
            let active = match hypothesis.activation {
                Activation::Independent { enabled } => enabled,
                Activation::Grouped { group_id } => {
                    groups.get(&group_id) == Some(&Some(hypothesis.id))
                }
            };
            if !active {
                continue;
            }
            model.active_hypotheses.push(hypothesis);
            for effect in &hypothesis.effects {
                match effect {
                    Effect::Constraint(effect) => model.constraints.push(ResolvedConstraint {
                        id: effect.id,
                        constraint: &effect.constraint,
                        hypothesis,
                    }),
                    Effect::Seed(seed) => model.seeds.push(ResolvedSeed {
                        seed,
                        hypothesis: Some(hypothesis),
                    }),
                }
            }
        }
        model
    }
}

impl<'a> ResolvedModel<'a> {
    pub fn active_hypotheses(&self) -> &[&'a Hypothesis] {
        &self.active_hypotheses
    }

    pub fn constraints(&self) -> &[ResolvedConstraint<'a>] {
        &self.constraints
    }

    /// Manual seeds first, then active hypothesis seed effects, in source order.
    pub fn seeds(&self) -> &[ResolvedSeed<'a>] {
        &self.seeds
    }
}
