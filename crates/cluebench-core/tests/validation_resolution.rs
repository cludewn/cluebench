// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cluebench_core::limits::*;
use cluebench_core::*;

fn hypothesis(id: u32) -> Hypothesis {
    Hypothesis {
        id: Id(id),
        origin: HypothesisOrigin::Manual,
        interpretation: String::new(),
        confidence: Confidence::Medium,
        applicability: Applicability::Hard,
        activation: Activation::Independent { enabled: true },
        effects: Vec::new(),
    }
}

fn constraint(id: u32, constraint: Constraint) -> Effect {
    Effect::Constraint(ConstraintEffect {
        id: Id(id),
        constraint,
    })
}

fn seed(id: u32, text: &str) -> Seed {
    Seed {
        id: Id(id),
        text: text.to_owned(),
    }
}

fn project_with_constraint(value: Constraint) -> RawProject {
    let mut item = hypothesis(1);
    item.effects.push(constraint(2, value));
    RawProject {
        hypotheses: vec![item],
        ..RawProject::default()
    }
}

fn assert_code(project: RawProject, code: DiagnosticCode) {
    let errors = project.validate().expect_err("input must be rejected");
    assert!(
        errors.diagnostics().iter().any(|item| item.code == code),
        "expected {code:?}, received {errors:?}"
    );
}

#[test]
fn empty_project_is_valid_and_has_no_effects() {
    let validated = RawProject::default().validate().unwrap();
    let resolved = validated.resolve();
    assert!(resolved.active_hypotheses().is_empty());
    assert!(resolved.constraints().is_empty());
    assert!(resolved.seeds().is_empty());
}

#[test]
fn manual_constraint_needs_no_clue() {
    let validated = project_with_constraint(Constraint::Length { min: 2, max: 4 })
        .validate()
        .unwrap();
    let resolved = validated.resolve();
    assert_eq!(resolved.constraints().len(), 1);
    assert_eq!(resolved.constraints()[0].id, Id(2));
    assert_eq!(
        resolved.constraints()[0].hypothesis.origin,
        HypothesisOrigin::Manual
    );
    assert!(validated.raw().clues.is_empty());
}

#[test]
fn unicode_evidence_and_interpretation_are_preserved_verbatim() {
    let mut item = hypothesis(2);
    item.origin = HypothesisOrigin::Clue(Id(1));
    item.interpretation = " Fictional interpretation: café \u{301} ".to_owned();
    let project = RawProject {
        clues: vec![Clue {
            id: Id(1),
            text: " Fictional clue: 雪 ".to_owned(),
            source: "Synthetic source №1".to_owned(),
        }],
        hypotheses: vec![item],
        ..RawProject::default()
    };
    let expected = project.clone();
    let validated = project.validate().unwrap();
    assert_eq!(validated.raw(), &expected);
    let resolved = validated.resolve();
    assert_eq!(
        resolved.active_hypotheses()[0].interpretation,
        expected.hypotheses[0].interpretation
    );
}

#[test]
fn global_ids_reject_zero_in_every_entity_kind() {
    let cases = [
        RawProject {
            clues: vec![Clue {
                id: Id(0),
                text: String::new(),
                source: String::new(),
            }],
            ..RawProject::default()
        },
        RawProject {
            hypotheses: vec![hypothesis(0)],
            ..RawProject::default()
        },
        RawProject {
            alternative_groups: vec![AlternativeGroup {
                id: Id(0),
                selected: None,
            }],
            ..RawProject::default()
        },
        RawProject {
            manual_seeds: vec![seed(0, "x")],
            ..RawProject::default()
        },
        {
            let mut project = project_with_constraint(Constraint::Length { min: 1, max: 64 });
            project.hypotheses[0].effects =
                vec![constraint(0, Constraint::Length { min: 1, max: 64 })];
            project
        },
        {
            let mut item = hypothesis(1);
            item.effects.push(Effect::Seed(seed(0, "x")));
            RawProject {
                hypotheses: vec![item],
                ..RawProject::default()
            }
        },
    ];
    for project in cases {
        assert_code(project, DiagnosticCode::InvalidId);
    }
}

#[test]
fn duplicate_ids_are_rejected_within_and_across_entity_kinds() {
    let cases = [
        RawProject {
            hypotheses: vec![hypothesis(1), hypothesis(1)],
            ..RawProject::default()
        },
        RawProject {
            manual_seeds: vec![seed(1, "x"), seed(1, "y")],
            ..RawProject::default()
        },
        RawProject {
            clues: vec![Clue {
                id: Id(1),
                text: String::new(),
                source: String::new(),
            }],
            hypotheses: vec![hypothesis(1)],
            ..RawProject::default()
        },
        {
            let mut item = hypothesis(1);
            item.effects = vec![
                constraint(2, Constraint::Contains("x".to_owned())),
                Effect::Seed(seed(2, "x")),
            ];
            RawProject {
                hypotheses: vec![item],
                ..RawProject::default()
            }
        },
        {
            let mut item = hypothesis(1);
            item.effects.push(Effect::Seed(seed(2, "x")));
            RawProject {
                hypotheses: vec![item],
                manual_seeds: vec![seed(2, "x")],
                ..RawProject::default()
            }
        },
        RawProject {
            alternative_groups: vec![
                AlternativeGroup {
                    id: Id(1),
                    selected: None,
                },
                AlternativeGroup {
                    id: Id(1),
                    selected: None,
                },
            ],
            ..RawProject::default()
        },
    ];
    for project in cases {
        assert_code(project, DiagnosticCode::DuplicateId);
    }
}

#[test]
fn origin_must_reference_a_clue_even_for_disabled_hypotheses() {
    for enabled in [false, true] {
        let mut item = hypothesis(1);
        item.activation = Activation::Independent { enabled };
        item.origin = HypothesisOrigin::Clue(Id(2));
        assert_code(
            RawProject {
                hypotheses: vec![item],
                manual_seeds: vec![seed(2, "x")],
                ..RawProject::default()
            },
            DiagnosticCode::MissingClue,
        );
    }
}

#[test]
fn grouped_hypothesis_must_reference_a_group() {
    let mut item = hypothesis(1);
    item.activation = Activation::Grouped { group_id: Id(2) };
    assert_code(
        RawProject {
            hypotheses: vec![item],
            manual_seeds: vec![seed(2, "x")],
            ..RawProject::default()
        },
        DiagnosticCode::MissingGroup,
    );
}

#[test]
fn alternative_selection_rejects_missing_independent_and_other_group_members() {
    for selected in [Id(99), Id(1), Id(2)] {
        let independent = hypothesis(1);
        let mut other_group_member = hypothesis(2);
        other_group_member.activation = Activation::Grouped { group_id: Id(20) };
        assert_code(
            RawProject {
                hypotheses: vec![independent, other_group_member],
                alternative_groups: vec![
                    AlternativeGroup {
                        id: Id(10),
                        selected: Some(selected),
                    },
                    AlternativeGroup {
                        id: Id(20),
                        selected: None,
                    },
                ],
                ..RawProject::default()
            },
            DiagnosticCode::InvalidSelection,
        );
    }
}

fn alternatives(selected: Option<Id>) -> RawProject {
    let mut a = hypothesis(2);
    a.activation = Activation::Grouped { group_id: Id(1) };
    a.effects = vec![
        constraint(4, Constraint::Prefix("A".to_owned())),
        Effect::Seed(seed(5, "A-seed")),
    ];
    let mut b = hypothesis(3);
    b.activation = Activation::Grouped { group_id: Id(1) };
    b.effects = vec![
        constraint(6, Constraint::Suffix("B".to_owned())),
        Effect::Seed(seed(7, "B-seed")),
    ];
    RawProject {
        hypotheses: vec![a, b],
        alternative_groups: vec![AlternativeGroup {
            id: Id(1),
            selected,
        }],
        ..RawProject::default()
    }
}

#[test]
fn alternative_a_b_none_select_exact_effects_without_mutating_sources() {
    for (selected, hypothesis_ids, constraint_ids, seed_ids) in [
        (Some(Id(2)), vec![Id(2)], vec![Id(4)], vec![Id(5)]),
        (Some(Id(3)), vec![Id(3)], vec![Id(6)], vec![Id(7)]),
        (None, vec![], vec![], vec![]),
    ] {
        let raw = alternatives(selected);
        let saved = raw.clone();
        let validated = raw.validate().unwrap();
        let resolved = validated.resolve();
        assert_eq!(
            resolved
                .active_hypotheses()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            hypothesis_ids
        );
        assert_eq!(
            resolved
                .constraints()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            constraint_ids
        );
        assert_eq!(
            resolved
                .seeds()
                .iter()
                .map(|item| item.seed.id)
                .collect::<Vec<_>>(),
            seed_ids
        );
        assert_eq!(validated.raw(), &saved);
        assert_eq!(validated.raw().hypotheses.len(), 2);
    }
}

#[test]
fn disabled_independent_hypothesis_contributes_nothing_but_manual_seed_survives() {
    let mut item = hypothesis(1);
    item.activation = Activation::Independent { enabled: false };
    item.effects = vec![
        constraint(2, Constraint::Contains("x".to_owned())),
        Effect::Seed(seed(3, "x")),
    ];
    let validated = RawProject {
        hypotheses: vec![item],
        manual_seeds: vec![seed(4, "x")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    assert!(resolved.active_hypotheses().is_empty());
    assert!(resolved.constraints().is_empty());
    assert_eq!(resolved.seeds().len(), 1);
    assert_eq!(resolved.seeds()[0].seed.id, Id(4));
    assert!(resolved.seeds()[0].hypothesis.is_none());
}

#[test]
fn identical_values_from_distinct_clues_and_manual_sources_keep_identity() {
    let mut first = hypothesis(3);
    first.origin = HypothesisOrigin::Clue(Id(1));
    first.effects = vec![
        constraint(5, Constraint::Contains("x".to_owned())),
        Effect::Seed(seed(6, "x")),
    ];
    let mut second = hypothesis(4);
    second.origin = HypothesisOrigin::Clue(Id(2));
    second.effects = vec![
        constraint(7, Constraint::Contains("x".to_owned())),
        Effect::Seed(seed(8, "x")),
    ];
    let validated = RawProject {
        clues: vec![
            Clue {
                id: Id(1),
                text: "Synthetic clue A".to_owned(),
                source: "A".to_owned(),
            },
            Clue {
                id: Id(2),
                text: "Synthetic clue B".to_owned(),
                source: "B".to_owned(),
            },
        ],
        hypotheses: vec![first, second],
        manual_seeds: vec![seed(9, "x")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    assert_eq!(resolved.constraints().len(), 2);
    assert_eq!(resolved.constraints()[0].id, Id(5));
    assert_eq!(resolved.constraints()[1].id, Id(7));
    assert_eq!(
        resolved.constraints()[0].hypothesis.origin,
        HypothesisOrigin::Clue(Id(1))
    );
    assert_eq!(
        resolved.constraints()[1].hypothesis.origin,
        HypothesisOrigin::Clue(Id(2))
    );
    assert_eq!(resolved.seeds().len(), 3);
    let mut origins = resolved
        .seeds()
        .iter()
        .map(|item| {
            (
                item.seed.id,
                item.hypothesis.map(|hypothesis| hypothesis.id),
            )
        })
        .collect::<Vec<_>>();
    origins.sort_by_key(|item| item.0.0);
    assert_eq!(
        origins,
        vec![(Id(6), Some(Id(3))), (Id(8), Some(Id(4))), (Id(9), None)]
    );
}

#[test]
fn all_invalid_constraints_are_rejected_even_when_inactive_or_unselected() {
    let invalid = vec![
        Constraint::Length { min: 0, max: 64 },
        Constraint::Length { min: 1, max: 65 },
        Constraint::Length { min: 3, max: 2 },
        Constraint::Position {
            index: 0,
            allowed: "x".to_owned(),
        },
        Constraint::Position {
            index: 65,
            allowed: "x".to_owned(),
        },
        Constraint::Position {
            index: 1,
            allowed: String::new(),
        },
        Constraint::Position {
            index: 1,
            allowed: "\n".to_owned(),
        },
        Constraint::Position {
            index: 1,
            allowed: "é".to_owned(),
        },
        Constraint::Contains(String::new()),
        Constraint::NotContains("x".repeat(65)),
        Constraint::Prefix("\u{7f}".to_owned()),
        Constraint::Suffix("é".to_owned()),
        Constraint::CharacterCount {
            class: CharacterClass::Digit,
            min: 3,
            max: 2,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Space,
            min: 0,
            max: 65,
        },
    ];
    for value in invalid {
        for activation in [
            Activation::Independent { enabled: false },
            Activation::Grouped { group_id: Id(3) },
        ] {
            let mut raw = project_with_constraint(value.clone());
            raw.hypotheses[0].activation = activation;
            raw.alternative_groups.push(AlternativeGroup {
                id: Id(3),
                selected: None,
            });
            assert_code(raw, DiagnosticCode::InvalidConstraint);
        }
    }
}

#[test]
fn usize_extremes_are_invalid_values_without_panics() {
    for value in [
        Constraint::Length {
            min: usize::MAX,
            max: usize::MAX,
        },
        Constraint::Position {
            index: usize::MAX,
            allowed: "x".to_owned(),
        },
        Constraint::CharacterCount {
            class: CharacterClass::Symbol,
            min: 0,
            max: usize::MAX,
        },
    ] {
        assert_code(
            project_with_constraint(value),
            DiagnosticCode::InvalidConstraint,
        );
    }
}

#[test]
fn constraint_range_boundaries_and_all_printable_position_choices_validate() {
    let printable: String = (b' '..=b'~').map(char::from).collect();
    for value in [
        Constraint::Length { min: 1, max: 1 },
        Constraint::Length { min: 64, max: 64 },
        Constraint::Position {
            index: 1,
            allowed: printable.clone(),
        },
        Constraint::Position {
            index: 64,
            allowed: printable,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Lowercase,
            min: 0,
            max: 0,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Uppercase,
            min: 64,
            max: 64,
        },
        Constraint::Contains("x".repeat(64)),
        Constraint::NotContains(" ".repeat(64)),
        Constraint::Prefix("~".repeat(64)),
        Constraint::Suffix("A".repeat(64)),
    ] {
        project_with_constraint(value).validate().unwrap();
    }
}

#[test]
fn position_duplicates_are_allowed_up_to_the_general_string_limit() {
    project_with_constraint(Constraint::Position {
        index: 1,
        allowed: "x".repeat(MAX_STRING_BYTES),
    })
    .validate()
    .unwrap();
    assert_code(
        project_with_constraint(Constraint::Position {
            index: 1,
            allowed: "x".repeat(MAX_STRING_BYTES + 1),
        }),
        DiagnosticCode::StringLimit,
    );
}

#[test]
fn every_literal_constraint_rejects_empty_control_non_ascii_and_overlong_values() {
    for text in [
        String::new(),
        "\0".to_owned(),
        "\u{1f}".to_owned(),
        "\u{7f}".to_owned(),
        "雪".to_owned(),
        "x".repeat(65),
    ] {
        for value in [
            Constraint::Contains(text.clone()),
            Constraint::NotContains(text.clone()),
            Constraint::Prefix(text.clone()),
            Constraint::Suffix(text.clone()),
        ] {
            assert_code(
                project_with_constraint(value),
                DiagnosticCode::InvalidConstraint,
            );
        }
    }
}

#[test]
fn manual_and_inactive_hypothesis_seeds_have_the_same_domain_restrictions() {
    for text in [
        String::new(),
        "x".repeat(65),
        "\n".to_owned(),
        "\u{7f}".to_owned(),
        "é".to_owned(),
    ] {
        assert_code(
            RawProject {
                manual_seeds: vec![seed(1, &text)],
                ..RawProject::default()
            },
            DiagnosticCode::InvalidSeed,
        );
        let mut item = hypothesis(1);
        item.activation = Activation::Independent { enabled: false };
        item.effects = vec![Effect::Seed(seed(2, &text))];
        assert_code(
            RawProject {
                hypotheses: vec![item],
                ..RawProject::default()
            },
            DiagnosticCode::InvalidSeed,
        );
    }
    for text in [" ".to_owned(), "~".to_owned(), "x".repeat(64)] {
        RawProject {
            manual_seeds: vec![seed(1, &text)],
            ..RawProject::default()
        }
        .validate()
        .unwrap();
    }
}

#[test]
fn individually_valid_contradictory_constraints_are_not_rejected() {
    let mut item = hypothesis(1);
    item.effects = vec![
        constraint(2, Constraint::Length { min: 1, max: 1 }),
        constraint(3, Constraint::Length { min: 64, max: 64 }),
        constraint(4, Constraint::Contains("x".to_owned())),
        constraint(5, Constraint::NotContains("x".to_owned())),
    ];
    let validated = RawProject {
        hypotheses: vec![item],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    assert_eq!(validated.resolve().constraints().len(), 4);
}

#[test]
fn entity_count_at_limit_succeeds_and_one_more_fails() {
    let at_limit = RawProject {
        manual_seeds: (1..=MAX_ENTITIES).map(|id| seed(id as u32, "x")).collect(),
        ..RawProject::default()
    };
    at_limit.clone().validate().unwrap();
    let mut over = at_limit;
    over.manual_seeds.push(seed((MAX_ENTITIES + 1) as u32, "x"));
    assert_code(over, DiagnosticCode::EntityLimit);
}

#[test]
fn entity_budget_counts_every_kind_including_both_effect_kinds() {
    let mut item = hypothesis(3);
    item.effects = vec![
        constraint(4, Constraint::Contains("x".to_owned())),
        Effect::Seed(seed(5, "x")),
    ];
    let at_limit = RawProject {
        clues: vec![Clue {
            id: Id(1),
            text: String::new(),
            source: String::new(),
        }],
        alternative_groups: vec![AlternativeGroup {
            id: Id(2),
            selected: None,
        }],
        hypotheses: vec![item],
        manual_seeds: (6..=MAX_ENTITIES).map(|id| seed(id as u32, "x")).collect(),
    };
    at_limit.clone().validate().unwrap();
    let mut over = at_limit;
    over.manual_seeds.push(seed((MAX_ENTITIES + 1) as u32, "x"));
    assert_code(over, DiagnosticCode::EntityLimit);
}

#[test]
fn hypothesis_count_at_limit_succeeds_and_one_more_fails() {
    let at_limit = RawProject {
        hypotheses: (1..=MAX_HYPOTHESES)
            .map(|id| hypothesis(id as u32))
            .collect(),
        ..RawProject::default()
    };
    let validated = at_limit.clone().validate().unwrap();
    assert_eq!(
        validated.resolve().active_hypotheses().len(),
        MAX_HYPOTHESES
    );
    let mut over = at_limit;
    over.hypotheses
        .push(hypothesis((MAX_HYPOTHESES + 1) as u32));
    assert_code(over, DiagnosticCode::HypothesisLimit);
}

#[test]
fn aggregate_effect_count_at_limit_succeeds_and_one_more_fails() {
    let mut first = hypothesis(1);
    let mut second = hypothesis(2);
    for index in 0..MAX_EFFECTS {
        let effect = if index % 2 == 0 {
            constraint((index + 3) as u32, Constraint::Contains("x".to_owned()))
        } else {
            Effect::Seed(seed((index + 3) as u32, "x"))
        };
        if index < MAX_EFFECTS / 2 {
            first.effects.push(effect);
        } else {
            second.effects.push(effect);
        }
    }
    let at_limit = RawProject {
        hypotheses: vec![first, second],
        ..RawProject::default()
    };
    let validated = at_limit.clone().validate().unwrap();
    let resolved = validated.resolve();
    assert_eq!(
        resolved.constraints().len() + resolved.seeds().len(),
        MAX_EFFECTS
    );
    let mut over = at_limit;
    over.hypotheses[1]
        .effects
        .push(Effect::Seed(seed((MAX_EFFECTS + 3) as u32, "x")));
    assert_code(over, DiagnosticCode::EffectLimit);
}

fn project_with_reference_edges(count: usize) -> RawProject {
    let mut project = RawProject {
        clues: vec![Clue {
            id: Id(1),
            text: String::new(),
            source: String::new(),
        }],
        alternative_groups: vec![AlternativeGroup {
            id: Id(2),
            selected: Some(Id(3)),
        }],
        ..RawProject::default()
    };
    for index in 0..MAX_HYPOTHESES {
        let mut item = hypothesis((index + 3) as u32);
        item.origin = HypothesisOrigin::Clue(Id(1));
        item.activation = Activation::Grouped { group_id: Id(2) };
        project.hypotheses.push(item);
    }
    let fixed_edges = MAX_HYPOTHESES * 2 + 1;
    for index in 0..count - fixed_edges {
        project.hypotheses[0]
            .effects
            .push(Effect::Seed(seed((MAX_HYPOTHESES + index + 3) as u32, "x")));
    }
    project
}

#[test]
fn reference_edges_at_limit_succeed_and_one_more_fails() {
    project_with_reference_edges(MAX_REFERENCE_EDGES)
        .validate()
        .unwrap();
    assert_code(
        project_with_reference_edges(MAX_REFERENCE_EDGES + 1),
        DiagnosticCode::ReferenceLimit,
    );
}

#[test]
fn individual_metadata_limit_measures_utf8_bytes() {
    let mut project = RawProject {
        clues: vec![Clue {
            id: Id(1),
            text: "é".repeat(MAX_STRING_BYTES / 2),
            source: String::new(),
        }],
        hypotheses: vec![hypothesis(2)],
        ..RawProject::default()
    };
    assert_eq!(project.clues[0].text.len(), MAX_STRING_BYTES);
    project.clone().validate().unwrap();
    project.clues[0].text.push('x');
    assert_code(project, DiagnosticCode::StringLimit);
    for field in ["source", "interpretation"] {
        let mut raw = RawProject {
            clues: vec![Clue {
                id: Id(1),
                text: String::new(),
                source: String::new(),
            }],
            hypotheses: vec![hypothesis(2)],
            ..RawProject::default()
        };
        if field == "source" {
            raw.clues[0].source = "x".repeat(MAX_STRING_BYTES);
        } else {
            raw.hypotheses[0].interpretation = "x".repeat(MAX_STRING_BYTES);
        }
        raw.clone().validate().unwrap();
        if field == "source" {
            raw.clues[0].source.push('x');
        } else {
            raw.hypotheses[0].interpretation.push('x');
        }
        assert_code(raw, DiagnosticCode::StringLimit);
    }
}

#[test]
fn aggregate_utf8_bytes_at_limit_succeed_and_one_more_fails() {
    let mut project = RawProject::default();
    let fields = MAX_TOTAL_STRING_BYTES / MAX_STRING_BYTES;
    assert_eq!(MAX_TOTAL_STRING_BYTES % MAX_STRING_BYTES, 0);
    assert_eq!(fields % 2, 0);
    for index in 0..fields / 2 {
        project.clues.push(Clue {
            id: Id((index + 1) as u32),
            text: "é".repeat(MAX_STRING_BYTES / 2),
            source: "x".repeat(MAX_STRING_BYTES),
        });
    }
    project.clone().validate().unwrap();
    project.clues.push(Clue {
        id: Id((fields + 1) as u32),
        text: "x".to_owned(),
        source: String::new(),
    });
    assert_code(project, DiagnosticCode::TotalStringLimit);
}

#[test]
fn aggregate_string_budget_includes_every_metadata_and_effect_string() {
    let mut item = hypothesis(9000);
    item.interpretation = "i".to_owned();
    item.effects = vec![
        constraint(
            9001,
            Constraint::Position {
                index: 1,
                allowed: "x".to_owned(),
            },
        ),
        constraint(9002, Constraint::Contains("x".to_owned())),
        constraint(9003, Constraint::NotContains("x".to_owned())),
        constraint(9004, Constraint::Prefix("x".to_owned())),
        constraint(9005, Constraint::Suffix("x".to_owned())),
        Effect::Seed(seed(9006, "x")),
    ];
    let mut project = RawProject {
        hypotheses: vec![item],
        manual_seeds: vec![seed(9007, "x")],
        ..RawProject::default()
    };
    // Eight bytes above: interpretation, six effect strings, and one manual seed.
    let mut remaining = MAX_TOTAL_STRING_BYTES - 8;
    while remaining != 0 {
        let text_bytes = remaining.min(MAX_STRING_BYTES);
        remaining -= text_bytes;
        let source_bytes = remaining.min(MAX_STRING_BYTES);
        remaining -= source_bytes;
        project.clues.push(Clue {
            id: Id((project.clues.len() + 1) as u32),
            text: "x".repeat(text_bytes),
            source: "x".repeat(source_bytes),
        });
    }
    project.clone().validate().unwrap();
    project.manual_seeds[0].text.push('x');
    assert_code(project, DiagnosticCode::TotalStringLimit);
}

#[test]
fn string_preflight_applies_to_seed_and_literal_fields_before_semantic_checks() {
    let large = "x".repeat(MAX_STRING_BYTES + 1);
    assert_code(
        RawProject {
            manual_seeds: vec![seed(1, &large)],
            ..RawProject::default()
        },
        DiagnosticCode::StringLimit,
    );
    let mut item = hypothesis(1);
    item.effects.push(Effect::Seed(seed(2, &large)));
    assert_code(
        RawProject {
            hypotheses: vec![item],
            ..RawProject::default()
        },
        DiagnosticCode::StringLimit,
    );
    for value in [
        Constraint::Contains(large.clone()),
        Constraint::NotContains(large.clone()),
        Constraint::Prefix(large.clone()),
        Constraint::Suffix(large),
    ] {
        assert_code(project_with_constraint(value), DiagnosticCode::StringLimit);
    }
}

#[test]
fn diagnostic_accumulation_is_bounded_and_reports_truncation() {
    let raw = RawProject {
        manual_seeds: (1..=MAX_DIAGNOSTICS + 1)
            .map(|id| seed(id as u32, ""))
            .collect(),
        ..RawProject::default()
    };
    let errors = raw.validate().unwrap_err();
    assert_eq!(errors.diagnostics().len(), MAX_DIAGNOSTICS);
    assert!(errors.is_truncated());
    let at_limit = RawProject {
        manual_seeds: (1..=MAX_DIAGNOSTICS)
            .map(|id| seed(id as u32, ""))
            .collect(),
        ..RawProject::default()
    }
    .validate()
    .unwrap_err();
    assert_eq!(at_limit.diagnostics().len(), MAX_DIAGNOSTICS);
    assert!(!at_limit.is_truncated());
}
