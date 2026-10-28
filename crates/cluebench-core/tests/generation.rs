// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::ControlFlow;

use cluebench_core::*;

#[derive(Debug, PartialEq, Eq)]
struct Delivery {
    text: String,
    seed_id: Id,
    owner: Option<(Id, HypothesisOrigin)>,
    path: GenerationPath,
    evaluation: Evaluation,
}

fn seed(id: u32, text: &str) -> Seed {
    Seed {
        id: Id(id),
        text: text.into(),
    }
}

fn constraint(id: u32, constraint: Constraint) -> Effect {
    Effect::Constraint(ConstraintEffect {
        id: Id(id),
        constraint,
    })
}

fn hypothesis(id: u32, applicability: Applicability, effects: Vec<Effect>) -> Hypothesis {
    Hypothesis {
        id: Id(id),
        origin: HypothesisOrigin::Manual,
        interpretation: "Synthetic interpretation, never a seed".into(),
        confidence: Confidence::High,
        applicability,
        activation: Activation::Independent { enabled: true },
        effects,
    }
}

fn collect(
    model: &ResolvedModel<'_>,
    config: &GenerationConfig,
) -> (GenerationStop, GenerationStats, Vec<Delivery>) {
    let mut deliveries = Vec::new();
    let result = model
        .generate(
            config,
            |_| false,
            |candidate| {
                let source = candidate.seed();
                let path = candidate.path();
                assert!(std::ptr::eq(candidate.model(), model));
                assert!(std::ptr::eq(candidate.config(), config));
                assert!(std::ptr::eq(
                    source.seed,
                    model.seeds()[path.seed_index].seed
                ));
                let stats = candidate.stats();
                assert_eq!(
                    stats.attempted_paths,
                    stats.domain_skips + stats.hard_rejections + stats.emitted_candidates
                );
                assert_eq!(stats.emitted_candidates, deliveries.len() as u64 + 1);
                deliveries.push(Delivery {
                    text: candidate.text().into(),
                    seed_id: source.seed.id,
                    owner: source.hypothesis.map(|owner| (owner.id, owner.origin)),
                    path,
                    evaluation: candidate.evaluation().clone(),
                });
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert!(std::ptr::eq(result.model(), model));
    assert!(std::ptr::eq(result.config(), config));
    (result.reason(), result.stats(), deliveries)
}

#[test]
fn resolved_sources_preserve_manual_hypothesis_and_clue_origins_for_a_b_none() {
    let hard = hypothesis(20, Applicability::Hard, vec![Effect::Seed(seed(21, "12"))]);
    let mut soft = hypothesis(30, Applicability::Soft, vec![Effect::Seed(seed(31, "12"))]);
    soft.origin = HypothesisOrigin::Clue(Id(100));
    let mut inactive = hypothesis(40, Applicability::Hard, vec![Effect::Seed(seed(41, "off"))]);
    inactive.activation = Activation::Independent { enabled: false };
    let mut a = hypothesis(50, Applicability::Hard, vec![Effect::Seed(seed(51, "12"))]);
    a.activation = Activation::Grouped { group_id: Id(70) };
    let mut b = hypothesis(60, Applicability::Soft, vec![Effect::Seed(seed(61, "12"))]);
    b.activation = Activation::Grouped { group_id: Id(70) };
    b.origin = HypothesisOrigin::Clue(Id(100));
    let raw = RawProject {
        clues: vec![Clue {
            id: Id(100),
            text: "Synthetic clue text, never a seed".into(),
            source: "Synthetic source".into(),
        }],
        hypotheses: vec![hard, soft, inactive, a, b],
        alternative_groups: vec![AlternativeGroup {
            id: Id(70),
            selected: None,
        }],
        manual_seeds: vec![seed(10, "12"), seed(11, "12")],
    };
    for (selection, selected_source) in [
        (
            Some(Id(50)),
            Some((Id(51), Id(50), HypothesisOrigin::Manual)),
        ),
        (
            Some(Id(60)),
            Some((Id(61), Id(60), HypothesisOrigin::Clue(Id(100)))),
        ),
        (None, None),
    ] {
        let mut scenario = raw.clone();
        scenario.alternative_groups[0].selected = selection;
        let snapshot = scenario.clone();
        let project = scenario.validate().unwrap();
        let model = project.resolve();
        let config = GenerationConfig::default();
        let (reason, stats, records) = collect(&model, &config);
        let mut expected = vec![
            (Id(10), None),
            (Id(11), None),
            (Id(21), Some((Id(20), HypothesisOrigin::Manual))),
            (Id(31), Some((Id(30), HypothesisOrigin::Clue(Id(100))))),
        ];
        if let Some((seed, owner, origin)) = selected_source {
            expected.push((seed, Some((owner, origin))));
        }
        assert_eq!(reason, GenerationStop::Completed);
        assert_eq!(stats.attempted_paths, expected.len() as u64);
        assert_eq!(stats.emitted_candidates, expected.len() as u64);
        assert_eq!(stats.domain_skips, 0);
        assert_eq!(stats.hard_rejections, 0);
        assert_eq!(
            records
                .iter()
                .map(|record| (record.seed_id, record.owner))
                .collect::<Vec<_>>(),
            expected
        );
        for record in &records {
            assert_eq!(record.text, "12");
            assert_eq!(record.evaluation.relative_points, 0);
            assert!(record.evaluation.matched_soft_hypotheses.is_empty());
        }
        assert_eq!(project.raw(), &snapshot);
    }
}

#[test]
fn clue_interpretation_and_constraint_literal_do_not_become_seeds() {
    let mut owner = hypothesis(
        20,
        Applicability::Soft,
        vec![constraint(
            21,
            Constraint::Contains("SyntheticLiteral".into()),
        )],
    );
    owner.origin = HypothesisOrigin::Clue(Id(10));
    let project = RawProject {
        clues: vec![Clue {
            id: Id(10),
            text: "SyntheticClue".into(),
            source: "SyntheticSource".into(),
        }],
        hypotheses: vec![owner],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let (reason, stats, records) = collect(&model, &GenerationConfig::default());
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(stats, GenerationStats::default());
    assert!(records.is_empty());
}

#[test]
fn case_variants_only_change_ascii_letters_inside_the_seed() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "aB 09-!zZ~")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        case_variants: vec![
            CaseVariant::Original,
            CaseVariant::Lowercase,
            CaseVariant::Uppercase,
        ],
        prefixes: vec!["PRE-".into()],
        suffixes: vec!["-X".into()],
        ..GenerationConfig::default()
    };
    let (reason, stats, records) = collect(&model, &config);
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(stats.emitted_candidates, 3);
    assert_eq!(
        records
            .iter()
            .map(|record| record.text.as_str())
            .collect::<Vec<_>>(),
        ["PRE-aB 09-!zZ~-X", "PRE-ab 09-!zz~-X", "PRE-AB 09-!ZZ~-X"]
    );
    assert_eq!(project.raw().manual_seeds[0].text, "aB 09-!zZ~");
}

#[test]
fn explicit_affixes_have_no_implicit_empty_option_or_recursive_application() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "aB")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        case_variants: vec![CaseVariant::Lowercase],
        prefixes: vec!["PRE-".into()],
        suffixes: vec!["-X".into()],
        ..GenerationConfig::default()
    };
    let (_, stats, records) = collect(&model, &config);
    assert_eq!(stats.attempted_paths, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].text, "PRE-ab-X");
    let empty_affixes = GenerationConfig {
        case_variants: vec![CaseVariant::Lowercase],
        prefixes: vec![String::new()],
        suffixes: vec![String::new()],
        ..GenerationConfig::default()
    };
    let (_, _, records) = collect(&model, &empty_affixes);
    assert_eq!(records[0].text, "ab");
}

#[test]
fn deterministic_order_matches_the_fixed_seed_case_prefix_suffix_sequence() {
    let project = RawProject {
        manual_seeds: vec![seed(20, "bA"), seed(10, "cD")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        case_variants: vec![
            CaseVariant::Uppercase,
            CaseVariant::Original,
            CaseVariant::Lowercase,
        ],
        prefixes: vec!["P".into(), String::new()],
        suffixes: vec!["Z".into(), "!".into()],
        ..GenerationConfig::default()
    };
    let (_, stats, records) = collect(&model, &config);
    let expected = [
        "PBAZ", "PBA!", "BAZ", "BA!", "PbAZ", "PbA!", "bAZ", "bA!", "PbaZ", "Pba!", "baZ", "ba!",
        "PCDZ", "PCD!", "CDZ", "CD!", "PcDZ", "PcD!", "cDZ", "cD!", "PcdZ", "Pcd!", "cdZ", "cd!",
    ];
    assert_eq!(stats.attempted_paths, 24);
    assert_eq!(
        records
            .iter()
            .map(|record| record.text.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    let expected_paths = [
        (0, 0, 0),
        (0, 0, 1),
        (0, 1, 0),
        (0, 1, 1),
        (1, 0, 0),
        (1, 0, 1),
        (1, 1, 0),
        (1, 1, 1),
        (2, 0, 0),
        (2, 0, 1),
        (2, 1, 0),
        (2, 1, 1),
    ];
    for (seed_index, chunk) in records.as_chunks::<12>().0.iter().enumerate() {
        for (record, &(case_index, prefix_index, suffix_index)) in chunk.iter().zip(&expected_paths)
        {
            assert_eq!(record.seed_id, [Id(20), Id(10)][seed_index]);
            assert_eq!(record.path.seed_index, seed_index);
            assert_eq!(record.path.case_index, case_index);
            assert_eq!(record.path.case_variant, config.case_variants[case_index]);
            assert_eq!(record.path.prefix_index, prefix_index);
            assert_eq!(record.path.suffix_index, suffix_index);
        }
    }
    assert_eq!(collect(&model, &config).2, records);
}

#[test]
fn duplicate_strings_preserve_every_seed_case_and_affix_path() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "1"), seed(2, "1")],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        case_variants: vec![
            CaseVariant::Original,
            CaseVariant::Lowercase,
            CaseVariant::Uppercase,
        ],
        prefixes: vec![String::new(), "1".into()],
        suffixes: vec![String::new(), "1".into()],
        ..GenerationConfig::default()
    };
    let (reason, stats, records) = collect(&model, &config);
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(stats.attempted_paths, 24);
    assert_eq!(stats.emitted_candidates, 24);
    for (seed_index, seed_records) in records.as_chunks::<12>().0.iter().enumerate() {
        for (case_index, case_records) in seed_records.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                case_records
                    .iter()
                    .map(|record| record.text.as_str())
                    .collect::<Vec<_>>(),
                ["1", "11", "11", "111"]
            );
            assert_ne!(case_records[1].path, case_records[2].path);
            for record in case_records {
                assert_eq!(record.seed_id, Id(seed_index as u32 + 1));
                assert_eq!(record.path.case_index, case_index);
            }
        }
    }
}

#[test]
fn combined_lengths_64_and_65_are_emitted_or_domain_skipped_without_truncation() {
    let project = RawProject {
        manual_seeds: vec![seed(1, &"a".repeat(63)), seed(2, &"b".repeat(64))],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        prefixes: vec![String::new(), "P".into()],
        suffixes: vec![String::new(), "S".into()],
        ..GenerationConfig::default()
    };
    let (reason, stats, records) = collect(&model, &config);
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(
        stats,
        GenerationStats {
            attempted_paths: 8,
            domain_skips: 4,
            hard_rejections: 0,
            emitted_candidates: 4,
        }
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record.text.clone())
            .collect::<Vec<_>>(),
        [
            "a".repeat(63),
            format!("{}S", "a".repeat(63)),
            format!("P{}", "a".repeat(63)),
            "b".repeat(64),
        ]
    );
    let oversized = GenerationConfig {
        prefixes: vec!["P".repeat(64)],
        ..GenerationConfig::default()
    };
    let (_, stats, records) = collect(&model, &oversized);
    assert_eq!(stats.domain_skips, 2);
    assert_eq!(stats.emitted_candidates, 0);
    assert!(records.is_empty());
}

#[test]
fn hard_filtering_and_soft_results_have_independent_fixed_expectations() {
    let hard = hypothesis(
        10,
        Applicability::Hard,
        vec![constraint(11, Constraint::Prefix("A".into()))],
    );
    let soft_high = hypothesis(
        20,
        Applicability::Soft,
        vec![constraint(21, Constraint::Contains("b".into()))],
    );
    let mut soft_low = hypothesis(
        30,
        Applicability::Soft,
        vec![constraint(31, Constraint::Suffix("1".into()))],
    );
    soft_low.confidence = Confidence::Low;
    let seed_owner = hypothesis(
        50,
        Applicability::Soft,
        vec![
            Effect::Seed(seed(51, "Ab1")),
            constraint(52, Constraint::Prefix("Z".into())),
        ],
    );
    let seed_only = hypothesis(60, Applicability::Soft, vec![Effect::Seed(seed(61, "A2"))]);
    let project = RawProject {
        manual_seeds: vec![seed(1, "Ab1"), seed(2, "ab1"), seed(3, "Ax"), seed(4, "A0")],
        hypotheses: vec![hard, soft_high, soft_low, seed_owner, seed_only],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig::default();
    let (reason, stats, records) = collect(&model, &config);
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(
        stats,
        GenerationStats {
            attempted_paths: 6,
            domain_skips: 0,
            hard_rejections: 1,
            emitted_candidates: 5,
        }
    );
    let expected = [
        (Id(1), "Ab1", vec![Id(20), Id(30)], 4),
        (Id(3), "Ax", vec![], 0),
        (Id(4), "A0", vec![], 0),
        (Id(51), "Ab1", vec![Id(20), Id(30)], 4),
        (Id(61), "A2", vec![], 0),
    ];
    for (record, (seed_id, text, matched, points)) in records.iter().zip(expected) {
        assert_eq!(record.seed_id, seed_id);
        assert_eq!(record.text, text);
        assert_eq!(
            record.evaluation,
            Evaluation {
                hard_eligible: true,
                matched_soft_hypotheses: matched,
                relative_points: points,
            }
        );
        assert_eq!(model.evaluate(text).unwrap(), record.evaluation);
    }
    assert_eq!(records[3].owner, Some((Id(50), HypothesisOrigin::Manual)));
    assert!(
        !records[3]
            .evaluation
            .matched_soft_hypotheses
            .contains(&Id(50))
    );
    assert!(!model.evaluate("ab1").unwrap().hard_eligible);
}

#[test]
fn a_delivery_preserves_the_maximum_bounded_soft_identity_list_and_points() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "AZ")],
        hypotheses: (0..limits::MAX_HYPOTHESES)
            .map(|index| {
                hypothesis(
                    100 + index as u32,
                    Applicability::Soft,
                    vec![constraint(
                        10_000 + index as u32,
                        Constraint::Prefix("A".into()),
                    )],
                )
            })
            .collect(),
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let (reason, stats, records) = collect(&model, &GenerationConfig::default());
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(stats.emitted_candidates, 1);
    assert_eq!(records[0].evaluation.relative_points, 3_072);
    assert_eq!(
        records[0].evaluation.matched_soft_hypotheses,
        (100..1_124).map(Id).collect::<Vec<_>>()
    );
}

#[test]
fn an_unknown_analysis_does_not_prevent_seed_generation() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "AZ")],
        hypotheses: vec![hypothesis(
            10,
            Applicability::Hard,
            vec![
                constraint(11, Constraint::Prefix("A".into())),
                constraint(12, Constraint::Suffix("Z".into())),
            ],
        )],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let analysis = model.analyze(AnalysisConfig::default(), |_| false);
    assert!(matches!(
        analysis.outcome(),
        AnalysisOutcome::Unknown(UnknownReason::InferenceLimit)
    ));
    let (reason, stats, records) = collect(&model, &GenerationConfig::default());
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(stats.emitted_candidates, 1);
    assert_eq!(records[0].text, "AZ");
}

#[test]
fn no_eligible_seed_paths_complete_without_claiming_structural_unsatisfiability() {
    let project = RawProject {
        manual_seeds: vec![seed(1, "q")],
        hypotheses: vec![hypothesis(
            10,
            Applicability::Hard,
            vec![constraint(11, Constraint::Prefix("A".into()))],
        )],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    assert!(model.evaluate("A").unwrap().hard_eligible);
    let (reason, stats, records) = collect(&model, &GenerationConfig::default());
    assert_eq!(reason, GenerationStop::Completed);
    assert_eq!(
        stats,
        GenerationStats {
            attempted_paths: 1,
            domain_skips: 0,
            hard_rejections: 1,
            emitted_candidates: 0,
        }
    );
    assert!(records.is_empty());
}

#[test]
fn generation_debug_redacts_source_candidate_affix_and_literal_contents() {
    let mut owner = hypothesis(
        10,
        Applicability::Soft,
        vec![
            Effect::Seed(seed(11, "SyntheticSeedMarker")),
            constraint(12, Constraint::Contains("SyntheticLiteralMarker".into())),
        ],
    );
    owner.origin = HypothesisOrigin::Clue(Id(20));
    owner.interpretation = "SyntheticInterpretationMarker".into();
    let project = RawProject {
        clues: vec![Clue {
            id: Id(20),
            text: "SyntheticClueMarker".into(),
            source: "SyntheticSourceMarker".into(),
        }],
        hypotheses: vec![owner],
        ..RawProject::default()
    }
    .validate()
    .unwrap();
    let model = project.resolve();
    let config = GenerationConfig {
        prefixes: vec!["SyntheticPrefixMarker".into()],
        suffixes: vec!["SyntheticSuffixMarker".into()],
        ..GenerationConfig::default()
    };
    let mut output = format!("{config:?}");
    let result = model
        .generate(
            &config,
            |_| false,
            |candidate| {
                assert_eq!(
                    candidate.text(),
                    "SyntheticPrefixMarkerSyntheticSeedMarkerSyntheticSuffixMarker"
                );
                output.push_str(&format!("{candidate:?}"));
                output.push_str(&format!("{:?}", candidate.path()));
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    output.push_str(&format!("{result:?}"));
    let invalid = GenerationConfig {
        prefixes: vec!["SyntheticInvalidMarker\n".into()],
        ..GenerationConfig::default()
    };
    let error = model
        .generate(
            &invalid,
            |_| panic!("invalid config must not poll"),
            |_| panic!("invalid config must not deliver"),
        )
        .unwrap_err();
    output.push_str(&format!("{invalid:?} {error:?} {error}"));
    for marker in [
        "SyntheticSeedMarker",
        "SyntheticLiteralMarker",
        "SyntheticInterpretationMarker",
        "SyntheticClueMarker",
        "SyntheticSourceMarker",
        "SyntheticPrefixMarker",
        "SyntheticSuffixMarker",
        "SyntheticInvalidMarker",
    ] {
        assert!(!output.contains(marker));
    }
    assert_eq!(result.stats().emitted_candidates, 1);
}
