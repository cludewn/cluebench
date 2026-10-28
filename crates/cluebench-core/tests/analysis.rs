// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cluebench_core::*;
use std::collections::BTreeSet;

fn hypothesis(id: u32, constraints: Vec<(u32, Constraint)>) -> Hypothesis {
    Hypothesis {
        id: Id(id),
        origin: HypothesisOrigin::Manual,
        interpretation: "Synthetic analysis fixture".into(),
        confidence: Confidence::Medium,
        applicability: Applicability::Hard,
        activation: Activation::Independent { enabled: true },
        effects: constraints
            .into_iter()
            .map(|(id, constraint)| {
                Effect::Constraint(ConstraintEffect {
                    id: Id(id),
                    constraint,
                })
            })
            .collect(),
    }
}

fn project(constraints: Vec<Constraint>) -> RawProject {
    RawProject {
        hypotheses: vec![hypothesis(
            1,
            constraints
                .into_iter()
                .enumerate()
                .map(|(index, constraint)| (100 + index as u32, constraint))
                .collect(),
        )],
        ..RawProject::default()
    }
}

fn length(min: usize, max: usize) -> Constraint {
    Constraint::Length { min, max }
}

fn position(index: usize, allowed: &str) -> Constraint {
    Constraint::Position {
        index,
        allowed: allowed.into(),
    }
}

fn count(class: CharacterClass, min: usize) -> Constraint {
    Constraint::CharacterCount {
        class,
        min,
        max: 64,
    }
}

fn ids(values: impl IntoIterator<Item = Id>) -> BTreeSet<Id> {
    values.into_iter().collect()
}

// Expected values below come from the contract and explicit candidate examples,
// never from production deduction helpers.
fn analyze<'m, 's>(model: &'m ResolvedModel<'s>) -> AnalysisResult<'m, 's> {
    model.analyze(
        AnalysisConfig {
            work_budget: 1_000_000,
        },
        |_| false,
    )
}

fn assert_sat(model: &ResolvedModel<'_>, expected_length: usize) {
    let result = analyze(model);
    let AnalysisOutcome::Satisfiable(witness) = result.outcome() else {
        panic!("expected verified SAT, got {result:?}");
    };
    assert_eq!(witness.as_str().len(), expected_length);
    assert!(model.evaluate(witness.as_str()).unwrap().hard_eligible);
    assert!(std::ptr::eq(result.model(), model));
    let domain = result.domain();
    assert_eq!((domain.min_byte, domain.max_byte), (0x20, 0x7e));
    assert_eq!((domain.min_length, domain.max_length), (1, 64));
}

fn assert_unsat(constraints: Vec<Constraint>, category: ContradictionCategory, support: &[u32]) {
    let validated = project(constraints).validate().unwrap();
    let model = validated.resolve();
    let result = analyze(&model);
    let AnalysisOutcome::Unsatisfiable(contradiction) = result.outcome() else {
        panic!("expected sound UNSAT, got {result:?}");
    };
    assert_eq!(contradiction.category(), category);
    assert_eq!(
        ids(contradiction.effect_ids()),
        ids(support.iter().copied().map(Id))
    );
    for evidence in contradiction.constraints() {
        assert_eq!(evidence.hypothesis.applicability, Applicability::Hard);
        let source = model
            .constraints()
            .iter()
            .find(|source| source.id == evidence.id)
            .unwrap();
        assert!(std::ptr::eq(source.constraint, evidence.constraint));
        assert!(std::ptr::eq(source.hypothesis, evidence.hypothesis));
    }
}

fn assert_compatible(constraints: Vec<Constraint>, candidate: &str) {
    let validated = project(constraints).validate().unwrap();
    let model = validated.resolve();
    assert!(model.evaluate(candidate).unwrap().hard_eligible);
    let result = analyze(&model);
    assert!(
        !matches!(result.outcome(), AnalysisOutcome::Unsatisfiable(_)),
        "{result:?}"
    );
    if let AnalysisOutcome::Satisfiable(witness) = result.outcome() {
        assert!(model.evaluate(witness.as_str()).unwrap().hard_eligible);
    }
}

#[test]
fn no_hard_and_length_only_return_independently_verified_boundary_witnesses() {
    let empty = RawProject::default().validate().unwrap();
    let empty_model = empty.resolve();
    assert_sat(&empty_model, 1);
    // Independent sum from the documented work table: extraction1,
    // position initialization64, classes5+5, witness2+1, preparation1,
    // verification1+1. This is deliberately not computed by a helper.
    assert_eq!(analyze(&empty_model).work_used(), 81);
    for (constraints, expected) in [
        (vec![length(1, 1)], 1),
        (vec![length(64, 64)], 64),
        (vec![length(64, 64); limits::MAX_EFFECTS], 64),
        (vec![length(2, 8), length(4, 6), length(3, 64)], 4),
    ] {
        let validated = project(constraints).validate().unwrap();
        assert_sat(&validated.resolve(), expected);
    }
}

#[test]
fn incompatible_length_intervals_preserve_the_actual_support() {
    assert_unsat(
        vec![length(1, 2), length(3, 64)],
        ContradictionCategory::LengthIntersection,
        &[100, 101],
    );
    assert_compatible(vec![length(1, 3), length(3, 64)], "abc");
}

#[test]
fn position_above_maximum_length_requires_the_length_support() {
    assert_unsat(
        vec![position(4, "a"), length(1, 3)],
        ContradictionCategory::PositionBeyondMaximumLength,
        &[100, 101],
    );
    assert_compatible(vec![position(3, "a"), length(1, 3)], "  a");
    assert_compatible(vec![position(64, "~")], &"~".repeat(64));
}

#[test]
fn position_intersections_handle_pairs_and_three_way_empty_sets() {
    assert_unsat(
        vec![position(1, "ab"), position(1, "cd")],
        ContradictionCategory::PositionIntersection,
        &[100, 101],
    );
    assert_unsat(
        vec![position(2, "ab"), position(2, "bc"), position(2, "ac")],
        ContradictionCategory::PositionIntersection,
        &[100, 101, 102],
    );
    for (first, second, candidate) in [("ab", "bc", " b"), ("ab", "ac", " a"), ("bc", "ac", " c")] {
        assert_compatible(vec![position(2, first), position(2, second)], candidate);
    }
    assert_compatible(vec![position(1, "a"), position(2, "b")], "ab");
}

#[test]
fn position_normalization_scans_the_full_4096_bytes_and_ignores_order_and_duplicates() {
    let allowed = format!("{}z", "a".repeat(4095));
    assert_compatible(vec![position(1, &allowed), position(1, "z")], "z");
    assert_compatible(vec![position(1, "~aa ~~"), position(1, " ~a")], " ");
    let printable: String = (b' '..=b'~').map(char::from).collect();
    assert_compatible(vec![position(1, &printable), position(1, "~")], "~");
}

#[test]
fn contains_not_contains_uses_the_required_to_forbidden_direction_and_case() {
    for (required, forbidden) in [("ab", "a"), ("cab", "ab"), ("aba", "aba")] {
        assert_unsat(
            vec![
                Constraint::Contains(required.into()),
                Constraint::NotContains(forbidden.into()),
            ],
            ContradictionCategory::ContainsNotContains,
            &[100, 101],
        );
    }
    for (required, forbidden, candidate) in [
        ("ab", "cab", "ab"),
        ("AB", "a", "AB"),
        ("aba", "bab", "aba"),
    ] {
        assert_compatible(
            vec![
                Constraint::Contains(required.into()),
                Constraint::NotContains(forbidden.into()),
            ],
            candidate,
        );
    }
}

#[test]
fn prefix_and_suffix_compatibility_accepts_nested_values_and_overlap() {
    assert_unsat(
        vec![
            Constraint::Prefix("ab".into()),
            Constraint::Prefix("ac".into()),
        ],
        ContradictionCategory::IncompatiblePrefixes,
        &[100, 101],
    );
    assert_unsat(
        vec![
            Constraint::Suffix("ab".into()),
            Constraint::Suffix("cb".into()),
        ],
        ContradictionCategory::IncompatibleSuffixes,
        &[100, 101],
    );
    assert_compatible(
        vec![
            Constraint::Prefix("ab".into()),
            Constraint::Prefix("abc".into()),
        ],
        "abc",
    );
    assert_compatible(
        vec![
            Constraint::Suffix("bc".into()),
            Constraint::Suffix("abc".into()),
        ],
        "abc",
    );
    assert_compatible(
        vec![
            length(3, 3),
            Constraint::Prefix("aba".into()),
            Constraint::Suffix("aba".into()),
        ],
        "aba",
    );
    assert_compatible(
        vec![
            length(5, 5),
            Constraint::Contains("aba".into()),
            Constraint::Contains("bab".into()),
            Constraint::Prefix("aba".into()),
            Constraint::Suffix("aba".into()),
        ],
        "ababa",
    );
}

#[test]
fn character_class_minima_use_the_maximum_per_class_and_keep_its_support() {
    use CharacterClass::*;
    assert_unsat(
        vec![
            count(Lowercase, 1),
            count(Lowercase, 3),
            count(Digit, 2),
            length(1, 4),
        ],
        ContradictionCategory::CharacterClassMinimums,
        &[101, 102, 103],
    );
    assert_compatible(
        vec![
            count(Lowercase, 2),
            count(Lowercase, 2),
            count(Digit, 1),
            length(3, 3),
        ],
        "aa0",
    );
    assert_compatible(
        vec![
            count(Lowercase, 1),
            count(Uppercase, 1),
            count(Digit, 1),
            count(Space, 1),
            count(Symbol, 1),
            length(5, 5),
        ],
        "aA0 !",
    );
}

#[test]
fn class_minimum_conflicts_include_domain_bounds_and_distinguish_space_from_symbol() {
    use CharacterClass::*;
    assert_unsat(
        vec![count(Lowercase, 32), count(Digit, 33)],
        ContradictionCategory::CharacterClassMinimums,
        &[100, 101],
    );
    assert_unsat(
        vec![count(Space, 1), count(Symbol, 1), length(1, 1)],
        ContradictionCategory::CharacterClassMinimums,
        &[100, 101, 102],
    );
    assert_compatible(vec![count(Space, 1), count(Symbol, 1), length(2, 2)], " !");
    assert_compatible(
        vec![count(Lowercase, 32), count(Digit, 32)],
        &format!("{}{}", "a".repeat(32), "0".repeat(32)),
    );
}

#[test]
fn unsupported_interactions_and_unsuccessful_fixed_witnesses_are_unknown() {
    for constraints in [
        vec![length(1, 1), Constraint::Contains("ab".into())],
        vec![position(1, "a"), Constraint::Prefix("b".into())],
        vec![Constraint::Contains("a".into())],
    ] {
        let validated = project(constraints).validate().unwrap();
        assert!(matches!(
            analyze(&validated.resolve()).outcome(),
            AnalysisOutcome::Unknown(UnknownReason::InferenceLimit)
        ));
    }
}

#[test]
fn every_active_hard_constraint_is_checked_before_returning_a_witness() {
    let good = project(vec![
        length(2, 2),
        position(1, " "),
        Constraint::Contains(" ".into()),
        Constraint::NotContains("x".into()),
        Constraint::Prefix(" ".into()),
        Constraint::Suffix(" ".into()),
        Constraint::CharacterCount {
            class: CharacterClass::Space,
            min: 2,
            max: 2,
        },
    ])
    .validate()
    .unwrap();
    assert_sat(&good.resolve(), 2);
    for constraint in [
        position(1, "x"),
        Constraint::Contains("x".into()),
        Constraint::NotContains(" ".into()),
        Constraint::Prefix("x".into()),
        Constraint::Suffix("x".into()),
        Constraint::CharacterCount {
            class: CharacterClass::Space,
            min: 0,
            max: 0,
        },
    ] {
        let validated = project(vec![length(2, 2), constraint]).validate().unwrap();
        assert!(matches!(
            analyze(&validated.resolve()).outcome(),
            AnalysisOutcome::Unknown(UnknownReason::InferenceLimit)
        ));
    }
}

#[test]
fn soft_constraints_and_seed_text_do_not_change_hard_logic_or_witness_domain() {
    let mut soft = hypothesis(
        2,
        vec![
            (200, length(64, 64)),
            (201, Constraint::Contains("x".into())),
            (202, Constraint::NotContains("x".into())),
        ],
    );
    soft.applicability = Applicability::Soft;
    soft.effects.push(Effect::Seed(Seed {
        id: Id(203),
        text: "X".into(),
    }));
    let mut hard = hypothesis(1, vec![(100, length(1, 1))]);
    hard.effects.push(Effect::Seed(Seed {
        id: Id(101),
        text: "Y".into(),
    }));
    let validated = RawProject {
        hypotheses: vec![hard, soft],
        manual_seeds: vec![Seed {
            id: Id(300),
            text: "Z".into(),
        }],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = validated.resolve();
    assert_sat(&model, 1);
    let result = analyze(&model);
    assert_eq!(
        result
            .hard_constraints()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [Id(100)]
    );
    let AnalysisOutcome::Satisfiable(witness) = result.outcome() else {
        unreachable!()
    };
    assert_eq!(witness.as_str(), " ");
}

#[test]
fn alternative_a_b_none_and_inactive_constraints_use_only_resolution() {
    for (selected, expected_ids, is_unsat) in [
        (Some(Id(2)), vec![Id(100), Id(200)], false),
        (Some(Id(3)), vec![Id(100), Id(300)], true),
        (None, vec![Id(100)], false),
    ] {
        let independent = hypothesis(1, vec![(100, length(1, 1))]);
        let mut a = hypothesis(2, vec![(200, length(1, 1))]);
        let mut b = hypothesis(3, vec![(300, length(2, 2))]);
        for item in [&mut a, &mut b] {
            item.activation = Activation::Grouped { group_id: Id(10) };
        }
        let mut inactive = hypothesis(
            4,
            vec![
                (400, Constraint::Contains("x".into())),
                (401, Constraint::NotContains("x".into())),
            ],
        );
        inactive.activation = Activation::Independent { enabled: false };
        let validated = RawProject {
            hypotheses: vec![independent, a, b, inactive],
            alternative_groups: vec![AlternativeGroup {
                id: Id(10),
                selected,
            }],
            ..RawProject::default()
        }
        .validate()
        .unwrap();
        let model = validated.resolve();
        let result = analyze(&model);
        assert_eq!(
            result
                .hard_constraints()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert_eq!(
            matches!(result.outcome(), AnalysisOutcome::Unsatisfiable(_)),
            is_unsat
        );
        if !is_unsat {
            assert_sat(&model, 1);
        }
    }
}

#[test]
fn equal_effect_values_retain_distinct_ids_and_clue_or_manual_origins() {
    let mut from_clue = hypothesis(1, vec![(100, length(2, 2))]);
    from_clue.origin = HypothesisOrigin::Clue(Id(10));
    let manual = hypothesis(2, vec![(200, length(2, 2)), (201, length(3, 3))]);
    let validated = RawProject {
        clues: vec![Clue {
            id: Id(10),
            text: "Synthetic source clue".into(),
            source: "Synthetic source".into(),
        }],
        hypotheses: vec![from_clue, manual],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = validated.resolve();
    let result = analyze(&model);
    let hard: Vec<_> = result.hard_constraints().collect();
    assert_eq!(
        hard.iter().map(|item| item.id).collect::<Vec<_>>(),
        [Id(100), Id(200), Id(201)]
    );
    assert_eq!(hard[0].hypothesis.origin, HypothesisOrigin::Clue(Id(10)));
    assert_eq!(hard[1].hypothesis.origin, HypothesisOrigin::Manual);
    assert_eq!(hard[0].constraint, hard[1].constraint);
    let AnalysisOutcome::Unsatisfiable(evidence) = result.outcome() else {
        panic!("expected length proof")
    };
    assert!(
        evidence
            .constraints()
            .iter()
            .any(|item| item.hypothesis.origin == HypothesisOrigin::Clue(Id(10)))
    );
    assert!(
        evidence
            .constraints()
            .iter()
            .any(|item| item.hypothesis.origin == HypothesisOrigin::Manual)
    );
    assert!(std::ptr::eq(result.model(), &model));
}

#[test]
fn zero_minimal_exact_and_one_short_work_budgets_are_enforced() {
    let validated = RawProject::default().validate().unwrap();
    let model = validated.resolve();
    let full = analyze(&model);
    let needed = full.work_used();
    assert!(needed > 1);
    for budget in [0, 1, needed - 1] {
        let result = model.analyze(
            AnalysisConfig {
                work_budget: budget,
            },
            |_| false,
        );
        assert!(matches!(
            result.outcome(),
            AnalysisOutcome::Unknown(UnknownReason::WorkBudgetExhausted)
        ));
        assert!(result.work_used() <= budget);
    }
    let exact = model.analyze(
        AnalysisConfig {
            work_budget: needed,
        },
        |_| false,
    );
    assert!(matches!(exact.outcome(), AnalysisOutcome::Satisfiable(_)));
    assert_eq!(exact.work_used(), needed);
}

fn checkpoint_fixture() -> ValidatedProject {
    project(vec![
        length(4, 4),
        position(1, &" ".repeat(4096)),
        Constraint::Prefix(" ".into()),
        Constraint::Prefix("  ".into()),
        Constraint::NotContains("x".into()),
    ])
    .validate()
    .unwrap()
}

#[test]
fn every_analysis_phase_reserves_work_before_execution() {
    let validated = checkpoint_fixture();
    let model = validated.resolve();
    let mut trace = Vec::new();
    let full = model.analyze(
        AnalysisConfig {
            work_budget: 1_000_000,
        },
        |checkpoint| {
            trace.push(checkpoint);
            false
        },
    );
    assert!(matches!(full.outcome(), AnalysisOutcome::Satisfiable(_)));
    for phase in [
        AnalysisPhase::Extraction,
        AnalysisPhase::Preprocessing,
        AnalysisPhase::Deduction,
        AnalysisPhase::WitnessConstruction,
        AnalysisPhase::WitnessVerification,
        AnalysisPhase::ResultPreparation,
    ] {
        let checkpoint = trace
            .iter()
            .find(|checkpoint| checkpoint.phase == phase)
            .expect("required charged phase");
        assert!(checkpoint.next_cost > 0);
        let budget = checkpoint.work_used + checkpoint.next_cost - 1;
        let result = model.analyze(
            AnalysisConfig {
                work_budget: budget,
            },
            |_| false,
        );
        assert!(
            matches!(
                result.outcome(),
                AnalysisOutcome::Unknown(UnknownReason::WorkBudgetExhausted)
            ),
            "phase {phase:?}: {result:?}"
        );
        assert_eq!(result.work_used(), checkpoint.work_used);
        assert!(result.work_used() <= budget);
    }
    for pair in trace.windows(2) {
        assert_eq!(pair[0].work_used + pair[0].next_cost, pair[1].work_used);
    }
    let last = trace.last().unwrap();
    assert_eq!(last.work_used + last.next_cost, full.work_used());
    let preprocessing: Vec<_> = trace
        .iter()
        .filter(|checkpoint| checkpoint.phase == AnalysisPhase::Preprocessing)
        .collect();
    assert!(
        preprocessing.len() >= 4096,
        "allowed bytes must have bounded checkpoints"
    );
}

#[test]
fn cancellation_before_start_wins_over_simultaneous_budget_exhaustion() {
    let validated = RawProject::default().validate().unwrap();
    let model = validated.resolve();
    let result = model.analyze(AnalysisConfig { work_budget: 0 }, |_| true);
    assert!(matches!(
        result.outcome(),
        AnalysisOutcome::Unknown(UnknownReason::Cancelled)
    ));
    assert_eq!(result.work_used(), 0);
}

#[test]
fn cancellation_is_deterministic_inside_preprocessing_deduction_and_witness_work() {
    let validated = checkpoint_fixture();
    let model = validated.resolve();
    for phase in [
        AnalysisPhase::Preprocessing,
        AnalysisPhase::Deduction,
        AnalysisPhase::WitnessConstruction,
        AnalysisPhase::WitnessVerification,
    ] {
        let mut observed = 0;
        let result = model.analyze(
            AnalysisConfig {
                work_budget: 1_000_000,
            },
            |checkpoint| {
                if checkpoint.phase == phase {
                    observed += 1;
                }
                checkpoint.phase == phase && observed == 2
            },
        );
        assert_eq!(observed, 2, "fixture must enter phase {phase:?}");
        assert!(
            matches!(
                result.outcome(),
                AnalysisOutcome::Unknown(UnknownReason::Cancelled)
            ),
            "{phase:?}: {result:?}"
        );
        let mut repeated = 0;
        let again = model.analyze(
            AnalysisConfig {
                work_budget: 1_000_000,
            },
            |checkpoint| {
                if checkpoint.phase == phase {
                    repeated += 1;
                }
                checkpoint.phase == phase && repeated == 2
            },
        );
        assert_eq!(again.work_used(), result.work_used());
        assert_eq!(
            format!("{:?}", again.outcome()),
            format!("{:?}", result.outcome())
        );
    }
}

#[test]
fn cancellation_after_the_last_reserved_operation_does_not_replace_a_completed_result() {
    for constraints in [vec![length(2, 2)], vec![length(1, 1), length(2, 2)]] {
        let validated = project(constraints).validate().unwrap();
        let model = validated.resolve();
        let mut full_trace = Vec::new();
        let baseline = model.analyze(
            AnalysisConfig {
                work_budget: 1_000_000,
            },
            |checkpoint| {
                full_trace.push(checkpoint);
                false
            },
        );
        let preparation = model.analyze(
            AnalysisConfig {
                work_budget: 1_000_000,
            },
            |checkpoint| checkpoint.phase == AnalysisPhase::ResultPreparation,
        );
        assert!(matches!(
            preparation.outcome(),
            AnalysisOutcome::Unknown(UnknownReason::Cancelled)
        ));
        let mut polls = 0;
        let mut cancellation_now_pending = false;
        let completed = model.analyze(
            AnalysisConfig {
                work_budget: 1_000_000,
            },
            |_| {
                polls += 1;
                if cancellation_now_pending {
                    return true;
                }
                // A GUI flag may change just after a checkpoint observes false.
                // The final reserved operation must complete without a new poll
                // invalidating its verified witness or assembled proof.
                cancellation_now_pending = polls == full_trace.len();
                false
            },
        );
        assert!(cancellation_now_pending);
        assert_eq!(polls, full_trace.len());
        assert_eq!(completed.work_used(), baseline.work_used());
        assert_eq!(
            format!("{:?}", completed.outcome()),
            format!("{:?}", baseline.outcome())
        );
    }
}

#[test]
fn identical_input_and_configuration_reproduce_evidence_witness_and_work() {
    for constraints in [
        vec![length(3, 3)],
        vec![position(1, "ab"), position(1, "bc"), position(1, "ac")],
    ] {
        let validated = project(constraints).validate().unwrap();
        let model = validated.resolve();
        let a = analyze(&model);
        let b = analyze(&model);
        assert_eq!(a.work_used(), b.work_used());
        assert_eq!(format!("{:?}", a.outcome()), format!("{:?}", b.outcome()));
        match (a.outcome(), b.outcome()) {
            (AnalysisOutcome::Satisfiable(a), AnalysisOutcome::Satisfiable(b)) => {
                assert_eq!(a.as_str(), b.as_str())
            }
            (AnalysisOutcome::Unsatisfiable(a), AnalysisOutcome::Unsatisfiable(b)) => assert_eq!(
                a.effect_ids().collect::<Vec<_>>(),
                b.effect_ids().collect::<Vec<_>>()
            ),
            _ => panic!("fixture must complete a proof or witness"),
        }
    }
}

#[test]
fn diagnostic_debug_does_not_include_witness_literals_clues_or_seeds() {
    let mut raw = project(vec![
        Constraint::Contains("SyntheticSensitiveLiteral".into()),
        Constraint::NotContains("SyntheticSensitiveLiteral".into()),
    ]);
    raw.clues.push(Clue {
        id: Id(10),
        text: "SyntheticSensitiveClue".into(),
        source: "SyntheticSensitiveSource".into(),
    });
    raw.hypotheses[0].origin = HypothesisOrigin::Clue(Id(10));
    raw.hypotheses[0].interpretation = "SyntheticSensitiveInterpretation".into();
    raw.manual_seeds.push(Seed {
        id: Id(11),
        text: "SyntheticSensitiveSeed".into(),
    });
    let validated = raw.validate().unwrap();
    let model = validated.resolve();
    let result = analyze(&model);
    let diagnostic = format!("{result:?} {:?}", result.outcome());
    for text in [
        "SyntheticSensitiveLiteral",
        "SyntheticSensitiveClue",
        "SyntheticSensitiveSource",
        "SyntheticSensitiveInterpretation",
        "SyntheticSensitiveSeed",
    ] {
        assert!(!diagnostic.contains(text));
    }
    let validated = project(vec![length(64, 64)]).validate().unwrap();
    let model = validated.resolve();
    let result = analyze(&model);
    let AnalysisOutcome::Satisfiable(witness) = result.outcome() else {
        unreachable!()
    };
    assert!(!format!("{result:?} {witness:?}").contains(witness.as_str()));
}

#[test]
fn complete_one_byte_ascii_reference_space_checks_conflicts_and_counterexamples() {
    // This enumerates the *entire* 95-character alphabet at the explicitly
    // constrained length 1. Expected matching bytes are written independently.
    let cases = [
        (vec![position(1, "ab"), position(1, "bc")], "b"),
        (
            vec![position(1, "ab"), position(1, "bc"), position(1, "ac")],
            "",
        ),
        (vec![position(2, "a")], ""),
        (
            vec![
                Constraint::Contains("a".into()),
                Constraint::NotContains("A".into()),
            ],
            "a",
        ),
        (
            vec![
                Constraint::Contains("a".into()),
                Constraint::NotContains("a".into()),
            ],
            "",
        ),
        (
            vec![
                Constraint::Prefix("a".into()),
                Constraint::Prefix("b".into()),
            ],
            "",
        ),
        (
            vec![
                Constraint::Suffix("a".into()),
                Constraint::Suffix("b".into()),
            ],
            "",
        ),
        (
            vec![
                Constraint::Prefix("a".into()),
                Constraint::Suffix("a".into()),
            ],
            "a",
        ),
        (
            vec![
                count(CharacterClass::Space, 1),
                count(CharacterClass::Symbol, 1),
            ],
            "",
        ),
        (
            vec![
                count(CharacterClass::Lowercase, 1),
                count(CharacterClass::Lowercase, 1),
            ],
            "abcdefghijklmnopqrstuvwxyz",
        ),
    ];
    for (mut constraints, expected) in cases {
        constraints.insert(0, length(1, 1));
        let validated = project(constraints).validate().unwrap();
        let model = validated.resolve();
        let actual: String = (b' '..=b'~')
            .filter(|byte| {
                model
                    .evaluate(&char::from(*byte).to_string())
                    .unwrap()
                    .hard_eligible
            })
            .map(char::from)
            .collect();
        assert_eq!(actual, expected);
        let result = analyze(&model);
        if expected.is_empty() {
            assert!(
                matches!(result.outcome(), AnalysisOutcome::Unsatisfiable(_)),
                "{result:?}"
            );
        } else {
            assert!(
                !matches!(result.outcome(), AnalysisOutcome::Unsatisfiable(_)),
                "{result:?}"
            );
        }
    }
}

#[test]
fn each_reserved_checkpoint_can_stop_without_leaking_a_partial_result() {
    let validated = project(vec![
        length(2, 2),
        position(1, " "),
        Constraint::Prefix(" ".into()),
        Constraint::Prefix("  ".into()),
    ])
    .validate()
    .unwrap();
    let model = validated.resolve();
    let mut trace = Vec::new();
    let full = model.analyze(
        AnalysisConfig {
            work_budget: 1_000_000,
        },
        |checkpoint| {
            trace.push(checkpoint);
            false
        },
    );
    assert!(matches!(full.outcome(), AnalysisOutcome::Satisfiable(_)));
    for stop in trace {
        let one_short = stop.work_used + stop.next_cost - 1;
        let exhausted = model.analyze(
            AnalysisConfig {
                work_budget: one_short,
            },
            |_| false,
        );
        assert!(matches!(
            exhausted.outcome(),
            AnalysisOutcome::Unknown(UnknownReason::WorkBudgetExhausted)
        ));
        assert_eq!(exhausted.work_used(), stop.work_used);
        // At the exact same checkpoint, cancellation takes precedence even
        // when there are no remaining work units for the reserved operation.
        let cancelled = model.analyze(
            AnalysisConfig {
                work_budget: stop.work_used,
            },
            |checkpoint| checkpoint.work_used == stop.work_used,
        );
        assert!(matches!(
            cancelled.outcome(),
            AnalysisOutcome::Unknown(UnknownReason::Cancelled)
        ));
        assert_eq!(cancelled.work_used(), stop.work_used);
    }
}

#[test]
fn maximum_position_evidence_is_reserved_before_scanning_or_copying_support() {
    let mut constraints = vec![position(1, "a"); 2047];
    constraints.push(position(1, "b"));
    let validated = project(constraints).validate().unwrap();
    let model = validated.resolve();
    let mut reservation = None;
    let result = model.analyze(AnalysisConfig::default(), |checkpoint| {
        if checkpoint.phase == AnalysisPhase::Deduction && checkpoint.next_cost == 4097 {
            reservation = Some(checkpoint);
        }
        false
    });
    let AnalysisOutcome::Unsatisfiable(proof) = result.outcome() else {
        panic!("expected maximum-size Position evidence")
    };
    assert_eq!(
        proof.category(),
        ContradictionCategory::PositionIntersection
    );
    assert_eq!(
        proof.effect_ids().collect::<Vec<_>>(),
        (100..2148).map(Id).collect::<Vec<_>>()
    );
    let reservation = reservation.expect("reserve 1 + 2 * 2048 units before support traversal");
    for (budget, cancel, expected) in [
        (
            reservation.work_used + 4096,
            false,
            UnknownReason::WorkBudgetExhausted,
        ),
        (reservation.work_used, true, UnknownReason::Cancelled),
    ] {
        let stopped = model.analyze(
            AnalysisConfig {
                work_budget: budget,
            },
            |checkpoint| cancel && checkpoint == reservation,
        );
        assert!(
            matches!(stopped.outcome(), AnalysisOutcome::Unknown(reason) if *reason == expected)
        );
        assert_eq!(stopped.work_used(), reservation.work_used);
    }
}
