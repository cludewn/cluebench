// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cluebench_core::{
    Activation, Applicability, CandidateError, Confidence, Constraint, ConstraintEffect, Effect,
    Evaluation, Hypothesis, HypothesisOrigin, Id, RawProject, ValidationErrors,
};
use serde::Serialize;

/// The IPC boundary exposes no candidate strings, seeds, or domain objects.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreProbeResponse {
    accepted_case_eligible: bool,
    rejected_case_eligible: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) enum CoreProbeError {
    #[serde(rename = "CORE_VALIDATION_FAILED")]
    ValidationFailed,
    #[serde(rename = "CORE_EVALUATION_FAILED")]
    EvaluationFailed,
}

impl From<ValidationErrors> for CoreProbeError {
    fn from(_: ValidationErrors) -> Self {
        Self::ValidationFailed
    }
}

impl From<CandidateError> for CoreProbeError {
    fn from(_: CandidateError) -> Self {
        Self::EvaluationFailed
    }
}

fn synthetic_project() -> RawProject {
    RawProject {
        hypotheses: vec![Hypothesis {
            id: Id(1),
            origin: HypothesisOrigin::Manual,
            interpretation: "Synthetic desktop connectivity probe".into(),
            confidence: Confidence::Medium,
            applicability: Applicability::Hard,
            activation: Activation::Independent { enabled: true },
            effects: vec![Effect::Constraint(ConstraintEffect {
                id: Id(2),
                constraint: Constraint::Length { min: 2, max: 4 },
            })],
        }],
        ..RawProject::default()
    }
}

fn response(accepted: Evaluation, rejected: Evaluation) -> CoreProbeResponse {
    CoreProbeResponse {
        accepted_case_eligible: accepted.hard_eligible,
        rejected_case_eligible: rejected.hard_eligible,
    }
}

/// No frontend input: this exercises the real validation/resolution/evaluation
/// path on a bounded, synthetic fixture without exposing project data.
#[tauri::command]
pub(crate) fn core_probe() -> Result<CoreProbeResponse, CoreProbeError> {
    let project = synthetic_project().validate()?;
    let model = project.resolve();
    Ok(response(model.evaluate("abc")?, model.evaluate("a")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_validates_and_resolves_the_hard_manual_length_fixture() {
        let project = synthetic_project().validate().unwrap();
        assert_eq!(project.raw().hypotheses.len(), 1);
        let hypothesis = &project.raw().hypotheses[0];
        assert_eq!(hypothesis.origin, HypothesisOrigin::Manual);
        assert_eq!(hypothesis.applicability, Applicability::Hard);
        let model = project.resolve();
        assert!(model.evaluate("abc").unwrap().hard_eligible);
        assert!(!model.evaluate("a").unwrap().hard_eligible);
        assert_eq!(
            core_probe().unwrap(),
            CoreProbeResponse {
                accepted_case_eligible: true,
                rejected_case_eligible: false,
            }
        );
    }

    #[test]
    fn response_fields_follow_actual_evaluations_even_when_cases_are_reversed() {
        let project = synthetic_project().validate().unwrap();
        let model = project.resolve();
        assert_eq!(
            response(model.evaluate("a").unwrap(), model.evaluate("abc").unwrap()),
            CoreProbeResponse {
                accepted_case_eligible: false,
                rejected_case_eligible: true
            }
        );
    }

    #[test]
    fn serialized_response_contains_exactly_two_boolean_fields() {
        assert_eq!(
            serde_json::to_value(core_probe().unwrap()).unwrap(),
            serde_json::json!({"acceptedCaseEligible": true, "rejectedCaseEligible": false})
        );
    }

    #[test]
    fn actual_validation_and_evaluation_errors_become_fixed_codes() {
        let mut invalid = synthetic_project();
        invalid.hypotheses[0].id = Id(0);
        let validation = CoreProbeError::from(invalid.validate().unwrap_err());
        assert_eq!(
            serde_json::to_value(validation).unwrap(),
            "CORE_VALIDATION_FAILED"
        );

        let project = synthetic_project().validate().unwrap();
        let model = project.resolve();
        for candidate in ["", "\n", &"a".repeat(65)] {
            let error = CoreProbeError::from(model.evaluate(candidate).unwrap_err());
            assert_eq!(
                serde_json::to_value(error).unwrap(),
                "CORE_EVALUATION_FAILED"
            );
        }
    }
}
