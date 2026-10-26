// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cluebench_core::{
    Activation, Applicability, CandidateError, CharacterClass, Confidence, Constraint,
    ConstraintEffect, Effect, Evaluation, Hypothesis, HypothesisOrigin, Id, RawProject, Seed,
};

fn hypothesis(
    id: u32,
    applicability: Applicability,
    confidence: Confidence,
    constraints: Vec<Constraint>,
) -> Hypothesis {
    Hypothesis {
        id: Id(id),
        origin: HypothesisOrigin::Manual,
        interpretation: "Synthetic evaluation fixture".into(),
        confidence,
        applicability,
        activation: Activation::Independent { enabled: true },
        effects: constraints
            .into_iter()
            .enumerate()
            .map(|(index, constraint)| {
                Effect::Constraint(ConstraintEffect {
                    id: Id(10_000 + id * 100 + index as u32),
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
            Applicability::Hard,
            Confidence::Medium,
            constraints,
        )],
        ..RawProject::default()
    }
}

fn assert_matches(constraint: Constraint, cases: &[(&str, bool)]) {
    let validated = project(vec![constraint]).validate().unwrap();
    let resolved = validated.resolve();
    for &(candidate, expected) in cases {
        let actual = resolved.evaluate(candidate).unwrap();
        assert_eq!(
            actual.hard_eligible, expected,
            "synthetic case: {candidate:?}"
        );
        assert!(actual.matched_soft_hypotheses.is_empty());
        assert_eq!(actual.relative_points, 0);
    }
}

#[test]
fn length_ranges_are_inclusive() {
    assert_matches(
        Constraint::Length { min: 2, max: 4 },
        &[
            ("a", false),
            ("ab", true),
            ("abc", true),
            ("abcd", true),
            ("abcde", false),
        ],
    );
    assert_matches(
        Constraint::Length { min: 1, max: 1 },
        &[(" ", true), ("aa", false)],
    );
    let boundary = "x".repeat(64);
    let shorter = "x".repeat(63);
    assert_matches(
        Constraint::Length { min: 64, max: 64 },
        &[(&boundary, true), (&shorter, false)],
    );
}

#[test]
fn positions_are_one_based_and_fail_beyond_candidate_length() {
    assert_matches(
        Constraint::Position {
            index: 1,
            allowed: "a".into(),
        },
        &[("ab", true), ("ba", false), ("a", true)],
    );
    assert_matches(
        Constraint::Position {
            index: 2,
            allowed: "b".into(),
        },
        &[("ab", true), ("b", false), ("a ", false)],
    );
    let last = format!("{}~", "a".repeat(63));
    let short = "~".repeat(63);
    assert_matches(
        Constraint::Position {
            index: 64,
            allowed: "~".into(),
        },
        &[(&last, true), (&short, false)],
    );
}

#[test]
fn position_accepts_all_95_printable_ascii_alternatives() {
    let all = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    assert_eq!(all.len(), 95);
    let validated = project(vec![Constraint::Position {
        index: 1,
        allowed: all.into(),
    }])
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    for byte in 0x20..=0x7e {
        let candidate = char::from(byte).to_string();
        assert!(resolved.evaluate(&candidate).unwrap().hard_eligible);
    }
}

#[test]
fn position_order_and_duplicate_alternatives_do_not_change_matching() {
    assert_matches(
        Constraint::Position {
            index: 1,
            allowed: "~aa ~~".into(),
        },
        &[("a", true), ("~", true), (" ", true), ("b", false)],
    );
}

#[test]
fn contains_requires_a_contiguous_case_sensitive_substring() {
    assert_matches(
        Constraint::Contains("ab".into()),
        &[
            ("ab", true),
            ("zabz", true),
            ("a-b", false),
            ("AB", false),
            ("a", false),
        ],
    );
    assert_matches(
        Constraint::Contains("aa".into()),
        &[("aaa", true), ("aaaa", true), ("aba", false)],
    );
}

#[test]
fn not_contains_tests_absence_without_occurrence_counts() {
    assert_matches(
        Constraint::NotContains("ab".into()),
        &[
            ("ab", false),
            ("zabz", false),
            ("a-b", true),
            ("AB", true),
            ("a", true),
        ],
    );
}

#[test]
fn prefix_is_anchored_and_case_sensitive() {
    assert_matches(
        Constraint::Prefix("Ab".into()),
        &[
            ("Ab", true),
            ("Abc", true),
            ("xAb", false),
            ("ab", false),
            ("A", false),
        ],
    );
}

#[test]
fn suffix_is_anchored_and_case_sensitive() {
    assert_matches(
        Constraint::Suffix("bZ".into()),
        &[
            ("bZ", true),
            ("abZ", true),
            ("bZx", false),
            ("bz", false),
            ("Z", false),
        ],
    );
}

#[test]
fn literals_and_prefix_suffix_regions_may_overlap() {
    let validated = project(vec![
        Constraint::Contains("aba".into()),
        Constraint::Contains("bab".into()),
        Constraint::Prefix("aba".into()),
        Constraint::Suffix("aba".into()),
    ])
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    assert!(resolved.evaluate("ababa").unwrap().hard_eligible);
    assert!(!resolved.evaluate("aba").unwrap().hard_eligible);
    let validated = project(vec![
        Constraint::Prefix("aba".into()),
        Constraint::Suffix("aba".into()),
    ])
    .validate()
    .unwrap();
    assert!(validated.resolve().evaluate("aba").unwrap().hard_eligible);
}

#[test]
fn character_count_ranges_are_inclusive_and_count_the_whole_candidate() {
    assert_matches(
        Constraint::CharacterCount {
            class: CharacterClass::Digit,
            min: 1,
            max: 2,
        },
        &[("ab", false), ("a1b", true), ("1a2", true), ("123", false)],
    );
    assert_matches(
        Constraint::CharacterCount {
            class: CharacterClass::Digit,
            min: 0,
            max: 0,
        },
        &[("abc", true), ("1abc", false)],
    );
    let full = "0".repeat(64);
    let shorter = "0".repeat(63);
    assert_matches(
        Constraint::CharacterCount {
            class: CharacterClass::Digit,
            min: 64,
            max: 64,
        },
        &[(&full, true), (&shorter, false)],
    );
}

#[test]
fn all_printable_ascii_characters_have_exactly_one_expected_class() {
    // Explicit alphabets are independent of the production classifier.
    let alphabets = [
        (CharacterClass::Lowercase, "abcdefghijklmnopqrstuvwxyz"),
        (CharacterClass::Uppercase, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        (CharacterClass::Digit, "0123456789"),
        (CharacterClass::Space, " "),
        (CharacterClass::Symbol, "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~"),
    ];
    for (class, alphabet) in alphabets {
        let validated = project(vec![Constraint::CharacterCount {
            class,
            min: 1,
            max: 1,
        }])
        .validate()
        .unwrap();
        let resolved = validated.resolve();
        for byte in 0x20..=0x7e {
            let character = char::from(byte);
            assert_eq!(
                resolved
                    .evaluate(&character.to_string())
                    .unwrap()
                    .hard_eligible,
                alphabet.contains(character),
                "class {class:?}, synthetic byte {byte}",
            );
        }
    }
}

#[test]
fn space_is_not_symbol_and_mixed_class_totals_are_exact() {
    let validated = project(vec![
        Constraint::CharacterCount {
            class: CharacterClass::Lowercase,
            min: 2,
            max: 2,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Uppercase,
            min: 1,
            max: 1,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Digit,
            min: 2,
            max: 2,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Space,
            min: 2,
            max: 2,
        },
        Constraint::CharacterCount {
            class: CharacterClass::Symbol,
            min: 2,
            max: 2,
        },
    ])
    .validate()
    .unwrap();
    assert!(
        validated
            .resolve()
            .evaluate("a B3!z7 ~")
            .unwrap()
            .hard_eligible
    );
    assert_matches(
        Constraint::CharacterCount {
            class: CharacterClass::Symbol,
            min: 0,
            max: 0,
        },
        &[(" ", true), ("~", false), ("!", false)],
    );
}

#[test]
fn unsupported_candidates_return_errors_before_hard_evaluation() {
    let validated = project(vec![Constraint::Length { min: 64, max: 64 }])
        .validate()
        .unwrap();
    let resolved = validated.resolve();
    assert_eq!(resolved.evaluate(""), Err(CandidateError::Empty));
    assert_eq!(
        resolved.evaluate(&"a".repeat(65)),
        Err(CandidateError::TooLong { length: 65 })
    );
    for invalid in ["\0", "\u{1f}", "\u{7f}", "\n", "\t", "é", "Ａ", "a\u{301}"] {
        assert!(matches!(
            resolved.evaluate(invalid),
            Err(CandidateError::UnsupportedByte { .. })
        ));
    }
    assert_eq!(
        resolved.evaluate("ab\n"),
        Err(CandidateError::UnsupportedByte { index: 2 })
    );
    assert!(!resolved.evaluate("a").unwrap().hard_eligible);
    assert!(resolved.evaluate(&"~".repeat(64)).unwrap().hard_eligible);
}

#[test]
fn candidates_are_not_trimmed_or_case_converted() {
    assert_matches(
        Constraint::Length { min: 3, max: 3 },
        &[(" a ", true), ("a", false), ("   ", true)],
    );
    assert_matches(
        Constraint::Prefix(" A".into()),
        &[(" Ab", true), ("Ab", false), (" ab", false)],
    );
}

#[test]
fn hard_and_soft_results_are_independent_with_fixed_expected_points() {
    let validated = RawProject {
        hypotheses: vec![
            hypothesis(
                1,
                Applicability::Hard,
                Confidence::High,
                vec![Constraint::Prefix("A".into())],
            ),
            hypothesis(
                2,
                Applicability::Soft,
                Confidence::High,
                vec![
                    Constraint::Contains("xy".into()),
                    Constraint::Length { min: 3, max: 3 },
                ],
            ),
            hypothesis(
                3,
                Applicability::Soft,
                Confidence::Low,
                vec![Constraint::Suffix("y".into())],
            ),
        ],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    let cases = [
        ("Axy", true, vec![Id(2), Id(3)], 4),
        ("Azy", true, vec![Id(3)], 1),
        ("Bxy", false, vec![Id(2), Id(3)], 4),
        ("Axya", true, vec![], 0),
    ];
    for (candidate, hard_eligible, matched_soft_hypotheses, relative_points) in cases {
        assert_eq!(
            resolved.evaluate(candidate).unwrap(),
            Evaluation {
                hard_eligible,
                matched_soft_hypotheses,
                relative_points,
            }
        );
    }
}

#[test]
fn a_soft_hypothesis_scores_once_only_if_every_constraint_matches() {
    let mut soft = hypothesis(
        1,
        Applicability::Soft,
        Confidence::Medium,
        vec![
            Constraint::Length { min: 3, max: 3 },
            Constraint::Prefix("a".into()),
            Constraint::Suffix("z".into()),
        ],
    );
    soft.effects.push(Effect::Seed(Seed {
        id: Id(50),
        text: "different seed".into(),
    }));
    let validated = RawProject {
        hypotheses: vec![soft],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    for (candidate, points, ids) in [
        ("abz", 2, vec![Id(1)]),
        ("abx", 0, vec![]),
        ("az", 0, vec![]),
    ] {
        let actual = resolved.evaluate(candidate).unwrap();
        assert!(actual.hard_eligible);
        assert_eq!(actual.relative_points, points);
        assert_eq!(actual.matched_soft_hypotheses, ids);
    }
}

#[test]
fn all_confidence_levels_have_fixed_relative_weights() {
    let validated = RawProject {
        hypotheses: vec![
            hypothesis(
                1,
                Applicability::Soft,
                Confidence::Low,
                vec![Constraint::Contains("a".into())],
            ),
            hypothesis(
                2,
                Applicability::Soft,
                Confidence::Medium,
                vec![Constraint::Contains("b".into())],
            ),
            hypothesis(
                3,
                Applicability::Soft,
                Confidence::High,
                vec![Constraint::Contains("c".into())],
            ),
        ],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    for (candidate, points) in [("a", 1), ("b", 2), ("c", 3), ("abc", 6), ("z", 0)] {
        assert_eq!(
            resolved.evaluate(candidate).unwrap().relative_points,
            points
        );
    }
}

#[test]
fn seed_only_and_effect_free_hypotheses_do_not_filter_or_score() {
    let mut hypotheses = Vec::new();
    for (id, applicability) in [(1, Applicability::Hard), (2, Applicability::Soft)] {
        hypotheses.push(hypothesis(id, applicability, Confidence::High, vec![]));
        let mut seeded = hypothesis(id + 2, applicability, Confidence::High, vec![]);
        seeded.effects.push(Effect::Seed(Seed {
            id: Id(id + 20),
            text: "synthetic".into(),
        }));
        hypotheses.push(seeded);
    }
    let validated = RawProject {
        hypotheses,
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    assert_eq!(
        validated.resolve().evaluate("other").unwrap(),
        Evaluation {
            hard_eligible: true,
            matched_soft_hypotheses: vec![],
            relative_points: 0,
        }
    );
}

#[test]
fn contradictory_individually_valid_constraints_are_preserved() {
    let validated = project(vec![
        Constraint::Length { min: 1, max: 1 },
        Constraint::Length { min: 2, max: 2 },
    ])
    .validate()
    .unwrap();
    let resolved = validated.resolve();
    assert!(!resolved.evaluate("a").unwrap().hard_eligible);
    assert!(!resolved.evaluate("ab").unwrap().hard_eligible);
}

#[test]
fn an_empty_project_accepts_supported_candidates_without_points() {
    let validated = RawProject::default().validate().unwrap();
    assert_eq!(
        validated.resolve().evaluate(" ").unwrap(),
        Evaluation {
            hard_eligible: true,
            matched_soft_hypotheses: vec![],
            relative_points: 0,
        }
    );
}

#[test]
fn literal_constraints_support_the_64_byte_boundary() {
    let full = "q".repeat(64);
    let shorter = "q".repeat(63);
    for constraint in [
        Constraint::Contains(full.clone()),
        Constraint::Prefix(full.clone()),
        Constraint::Suffix(full.clone()),
    ] {
        assert_matches(constraint, &[(&full, true), (&shorter, false)]);
    }
    assert_matches(
        Constraint::NotContains(full.clone()),
        &[(&full, false), (&shorter, true)],
    );
}

#[test]
fn maximum_hypothesis_population_has_a_bounded_exact_score() {
    let hypotheses = (1..=1_024)
        .map(|id| {
            hypothesis(
                id,
                Applicability::Soft,
                Confidence::High,
                vec![Constraint::Length { min: 1, max: 64 }],
            )
        })
        .collect();
    let validated = RawProject {
        hypotheses,
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let actual = validated.resolve().evaluate("x").unwrap();
    assert!(actual.hard_eligible);
    assert_eq!(actual.relative_points, 3_072);
    assert_eq!(actual.matched_soft_hypotheses.len(), 1_024);
    assert_eq!(actual.matched_soft_hypotheses.first(), Some(&Id(1)));
    assert_eq!(actual.matched_soft_hypotheses.last(), Some(&Id(1_024)));
}
