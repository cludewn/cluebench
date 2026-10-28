// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{cell::Cell, ops::ControlFlow};

use cluebench_core::{limits::*, *};

fn project(seed_texts: &[&str]) -> RawProject {
    RawProject {
        manual_seeds: seed_texts
            .iter()
            .enumerate()
            .map(|(index, text)| Seed {
                id: Id(index as u32 + 1),
                text: (*text).into(),
            })
            .collect(),
        ..RawProject::default()
    }
}

fn hard_prefix(raw: &mut RawProject, prefix: &str) {
    raw.hypotheses.push(Hypothesis {
        id: Id(10_000),
        origin: HypothesisOrigin::Manual,
        interpretation: "Synthetic boundary fixture".into(),
        confidence: Confidence::High,
        applicability: Applicability::Hard,
        activation: Activation::Independent { enabled: true },
        effects: vec![Effect::Constraint(ConstraintEffect {
            id: Id(10_001),
            constraint: Constraint::Prefix(prefix.into()),
        })],
    });
}

fn counts(stats: GenerationStats) -> (u64, u64, u64, u64) {
    assert_eq!(
        stats.attempted_paths,
        stats.domain_skips + stats.hard_rejections + stats.emitted_candidates
    );
    (
        stats.attempted_paths,
        stats.domain_skips,
        stats.hard_rejections,
        stats.emitted_candidates,
    )
}

#[test]
fn termination_a_b_c_d_f_and_zero_output_follow_the_contract() {
    // Expected reasons and counters are written directly from the contract.
    for (name, seeds, attempts, emissions, reason, expected) in [
        ("A", 0, 0, 0, GenerationStop::Completed, (0, 0, 0, 0)),
        ("B", 1, 1, 1, GenerationStop::Completed, (1, 0, 0, 1)),
        ("C", 2, 1, 1, GenerationStop::AttemptLimit, (1, 0, 0, 1)),
        ("D", 2, 2, 1, GenerationStop::OutputLimit, (1, 0, 0, 1)),
        ("F", 1, 0, 0, GenerationStop::AttemptLimit, (0, 0, 0, 0)),
        (
            "zero output",
            1,
            1,
            0,
            GenerationStop::OutputLimit,
            (0, 0, 0, 0),
        ),
        (
            "zero attempt",
            1,
            0,
            1,
            GenerationStop::AttemptLimit,
            (0, 0, 0, 0),
        ),
    ] {
        let validated = project(&vec!["Fixture"; seeds]).validate().unwrap();
        let model = validated.resolve();
        let config = GenerationConfig {
            attempt_limit: attempts,
            emission_limit: emissions,
            ..GenerationConfig::default()
        };
        let mut deliveries = 0;
        let result = model
            .generate(
                &config,
                |stats| {
                    counts(stats);
                    false
                },
                |candidate| {
                    deliveries += 1;
                    assert_eq!(counts(candidate.stats()).3, deliveries);
                    ControlFlow::Continue(())
                },
            )
            .unwrap();
        assert_eq!(result.reason(), reason, "case {name}");
        assert_eq!(counts(result.stats()), expected, "case {name}");
        assert_eq!(deliveries, expected.3, "case {name}");
        assert_eq!(result.potential_paths(), seeds as u64);
        assert!(std::ptr::eq(result.model(), &model));
        assert!(std::ptr::eq(result.config(), &config));
    }
}

#[test]
fn termination_e_j_count_the_stopping_delivery_even_on_the_final_path() {
    for seeds in [1, 2] {
        let validated = project(&vec!["Fixture"; seeds]).validate().unwrap();
        let model = validated.resolve();
        let config = GenerationConfig {
            attempt_limit: 1,
            emission_limit: 1,
            ..GenerationConfig::default()
        };
        let cancellation_polls = Cell::new(0);
        let mut deliveries = 0;
        let result = model
            .generate(
                &config,
                |stats| {
                    assert_eq!(counts(stats), (0, 0, 0, 0));
                    cancellation_polls.set(cancellation_polls.get() + 1);
                    false
                },
                |candidate| {
                    deliveries += 1;
                    assert_eq!(counts(candidate.stats()), (1, 0, 0, 1));
                    ControlFlow::Break(())
                },
            )
            .unwrap();
        assert_eq!(result.reason(), GenerationStop::ConsumerStopped);
        assert_eq!(counts(result.stats()), (1, 0, 0, 1));
        assert_eq!(deliveries, 1);
        assert_eq!(cancellation_polls.get(), 1);
    }
}

#[test]
fn termination_g_cancellation_precedes_both_zero_allowances() {
    let validated = project(&["Fixture"]).validate().unwrap();
    let model = validated.resolve();
    let config = GenerationConfig {
        attempt_limit: 0,
        emission_limit: 0,
        ..GenerationConfig::default()
    };
    let mut polls = 0;
    let result = model
        .generate(
            &config,
            |stats| {
                polls += 1;
                assert_eq!(counts(stats), (0, 0, 0, 0));
                true
            },
            |_| panic!("cancelled generation must not deliver"),
        )
        .unwrap();
    assert_eq!(result.reason(), GenerationStop::Cancelled);
    assert_eq!(counts(result.stats()), (0, 0, 0, 0));
    assert_eq!(polls, 1);
}

#[test]
fn empty_completion_does_not_observe_cancellation_even_with_zero_allowances() {
    let validated = RawProject::default().validate().unwrap();
    let model = validated.resolve();
    let config = GenerationConfig {
        attempt_limit: 0,
        emission_limit: 0,
        ..GenerationConfig::default()
    };
    let result = model
        .generate(
            &config,
            |_| panic!("exhaustion precedes cancellation"),
            |_| panic!("empty sources must not deliver"),
        )
        .unwrap();
    assert_eq!(result.reason(), GenerationStop::Completed);
    assert_eq!(counts(result.stats()), (0, 0, 0, 0));
}

#[test]
fn delivery_cancellation_is_observed_only_between_paths_with_exact_precedence() {
    for (seed_count, consumer_stop, expected_reason, expected_polls) in [
        (2, false, GenerationStop::Cancelled, 2),
        (1, false, GenerationStop::Completed, 1),
        (2, true, GenerationStop::ConsumerStopped, 1),
        (1, true, GenerationStop::ConsumerStopped, 1),
    ] {
        let validated = project(&vec!["Fixture"; seed_count]).validate().unwrap();
        let model = validated.resolve();
        let config = GenerationConfig {
            attempt_limit: 1,
            emission_limit: 1,
            ..GenerationConfig::default()
        };
        let cancellation = Cell::new(false);
        let mut polls = 0;
        let mut deliveries = 0;
        let result = model
            .generate(
                &config,
                |stats| {
                    counts(stats);
                    polls += 1;
                    cancellation.get()
                },
                |candidate| {
                    deliveries += 1;
                    assert_eq!(counts(candidate.stats()), (1, 0, 0, 1));
                    cancellation.set(true);
                    if consumer_stop {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            )
            .unwrap();
        assert_eq!(result.reason(), expected_reason);
        assert_eq!(counts(result.stats()), (1, 0, 0, 1));
        assert_eq!(deliveries, 1);
        assert_eq!(polls, expected_polls);
        cancellation.set(false);
        assert_eq!(result.reason(), expected_reason);
    }
}

#[test]
fn termination_i_classifies_domain_skip_and_hard_rejection_separately() {
    let long_prefix = "P".repeat(64);
    for (prefix, hard, expected) in [
        (long_prefix.as_str(), false, (1, 1, 0, 0)),
        ("", true, (1, 0, 1, 0)),
    ] {
        let mut raw = project(&["Fixture"]);
        if hard {
            hard_prefix(&mut raw, "Different");
        }
        let validated = raw.validate().unwrap();
        let model = validated.resolve();
        let config = GenerationConfig {
            prefixes: vec![prefix.into()],
            attempt_limit: 1,
            emission_limit: 1,
            ..GenerationConfig::default()
        };
        let result = model
            .generate(&config, |_| false, |_| panic!("path cannot emit"))
            .unwrap();
        assert_eq!(result.reason(), GenerationStop::Completed);
        assert_eq!(counts(result.stats()), expected);
    }
}

#[test]
fn mixed_path_counters_are_classified_at_every_callback_boundary() {
    let mut raw = project(&["ok", "bad"]);
    hard_prefix(&mut raw, "ok");
    let validated = raw.validate().unwrap();
    let model = validated.resolve();
    let config = GenerationConfig {
        prefixes: vec![String::new(), "P".repeat(64)],
        ..GenerationConfig::default()
    };
    let mut checkpoints = Vec::new();
    let mut deliveries = 0;
    let result = model
        .generate(
            &config,
            |stats| {
                checkpoints.push(counts(stats));
                false
            },
            |candidate| {
                deliveries += 1;
                assert_eq!(candidate.text(), "ok");
                assert_eq!(counts(candidate.stats()), (1, 0, 0, 1));
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(
        checkpoints,
        [(0, 0, 0, 0), (1, 0, 0, 1), (2, 1, 0, 1), (3, 1, 1, 1)]
    );
    assert_eq!(result.reason(), GenerationStop::Completed);
    assert_eq!(counts(result.stats()), (4, 2, 1, 1));
    assert_eq!(deliveries, 1);
}

#[test]
fn cancellation_observes_completed_skips_and_rejections_before_the_next_path() {
    for (domain_skip, expected) in [(true, (1, 1, 0, 0)), (false, (1, 0, 1, 0))] {
        for (seed_count, reason, poll_count) in [
            (1, GenerationStop::Completed, 1),
            (2, GenerationStop::Cancelled, 2),
        ] {
            let mut raw = project(&vec!["Fixture"; seed_count]);
            hard_prefix(&mut raw, "Different");
            let validated = raw.validate().unwrap();
            let model = validated.resolve();
            let config = GenerationConfig {
                prefixes: vec![if domain_skip {
                    "P".repeat(64)
                } else {
                    String::new()
                }],
                attempt_limit: 1,
                emission_limit: 1,
                ..GenerationConfig::default()
            };
            let mut polls = 0;
            let result = model
                .generate(
                    &config,
                    |stats| {
                        polls += 1;
                        if stats.attempted_paths == 1 {
                            assert_eq!(counts(stats), expected);
                            true
                        } else {
                            assert_eq!(counts(stats), (0, 0, 0, 0));
                            false
                        }
                    },
                    |_| panic!("skip or rejection must not deliver"),
                )
                .unwrap();
            assert_eq!(result.reason(), reason);
            assert_eq!(counts(result.stats()), expected);
            assert_eq!(polls, poll_count);
        }
    }
}

#[test]
fn limits_prevent_inspecting_later_paths_that_would_skip_or_reject() {
    let long_seed = "x".repeat(64);
    for stop in [GenerationStop::AttemptLimit, GenerationStop::OutputLimit] {
        let mut raw = project(&["ok", "bad", &long_seed]);
        hard_prefix(&mut raw, "ok");
        let validated = raw.validate().unwrap();
        let model = validated.resolve();
        let config = GenerationConfig {
            suffixes: vec!["!".into()],
            attempt_limit: if stop == GenerationStop::AttemptLimit {
                1
            } else {
                3
            },
            emission_limit: 1,
            ..GenerationConfig::default()
        };
        let mut deliveries = 0;
        let mut polls = 0;
        let result = model
            .generate(
                &config,
                |stats| {
                    counts(stats);
                    polls += 1;
                    false
                },
                |candidate| {
                    deliveries += 1;
                    assert_eq!(candidate.text(), "ok!");
                    ControlFlow::Continue(())
                },
            )
            .unwrap();
        assert_eq!(result.reason(), stop);
        assert_eq!(counts(result.stats()), (1, 0, 0, 1));
        assert_eq!(deliveries, 1);
        assert_eq!(polls, 2);
    }
}

fn config_error(config: &GenerationConfig) -> GenerationConfigError {
    // H: validation must run before exhaustion, cancellation, and delivery.
    let validated = RawProject::default().validate().unwrap();
    let model = validated.resolve();
    model
        .generate(
            config,
            |_| panic!("invalid configuration must fail before cancellation"),
            |_| panic!("invalid configuration must fail before delivery"),
        )
        .unwrap_err()
}

fn assert_valid_config(config: &GenerationConfig) {
    let validated = RawProject::default().validate().unwrap();
    let model = validated.resolve();
    let result = model
        .generate(config, |_| false, |_| ControlFlow::Continue(()))
        .unwrap();
    assert_eq!(result.reason(), GenerationStop::Completed);
    assert_eq!(counts(result.stats()), (0, 0, 0, 0));
}

#[test]
fn termination_h_rejects_empty_and_duplicate_option_lists_even_without_seeds() {
    for config in [
        GenerationConfig {
            case_variants: vec![],
            ..GenerationConfig::default()
        },
        GenerationConfig {
            prefixes: vec![],
            ..GenerationConfig::default()
        },
        GenerationConfig {
            suffixes: vec![],
            ..GenerationConfig::default()
        },
    ] {
        assert_eq!(
            config_error(&config).code,
            GenerationConfigErrorCode::EmptyOptions
        );
    }
    for config in [
        GenerationConfig {
            case_variants: vec![CaseVariant::Original, CaseVariant::Original],
            ..GenerationConfig::default()
        },
        GenerationConfig {
            prefixes: vec!["same".into(), "same".into()],
            ..GenerationConfig::default()
        },
        GenerationConfig {
            suffixes: vec![String::new(), String::new()],
            ..GenerationConfig::default()
        },
    ] {
        assert_eq!(
            config_error(&config).code,
            GenerationConfigErrorCode::DuplicateOption
        );
    }
    // The same value on opposite boundaries is legal; uniqueness is list-local.
    assert_valid_config(&GenerationConfig {
        prefixes: vec!["same".into()],
        suffixes: vec!["same".into()],
        ..GenerationConfig::default()
    });
}

#[test]
fn affix_domain_and_individual_byte_limits_are_validated_before_generation() {
    for bad in ["\n", "\t", "\0", "\u{1f}", "\u{7f}", "é", "あ"] {
        for prefix in [true, false] {
            let mut config = GenerationConfig::default();
            if prefix {
                config.prefixes = vec![bad.into()];
            } else {
                config.suffixes = vec![bad.into()];
            }
            assert_eq!(
                config_error(&config).code,
                GenerationConfigErrorCode::UnsupportedAffixByte
            );
        }
    }
    for prefix in [true, false] {
        let mut config = GenerationConfig::default();
        let boundary = vec!["P".repeat(MAX_AFFIX_BYTES)];
        if prefix {
            config.prefixes = boundary;
        } else {
            config.suffixes = boundary;
        }
        assert_valid_config(&config);
        if prefix {
            config.prefixes[0].push('P');
        } else {
            config.suffixes[0].push('P');
        }
        assert_eq!(
            config_error(&config).code,
            GenerationConfigErrorCode::AffixTooLong
        );
    }
    assert_valid_config(&GenerationConfig {
        prefixes: vec![" ".into()],
        suffixes: vec!["!~ ".into()],
        ..GenerationConfig::default()
    });
}

#[test]
fn maximum_option_counts_are_accepted_and_one_more_is_rejected() {
    assert_eq!(MAX_CASE_OPTIONS, 3);
    let mut config = GenerationConfig {
        case_variants: vec![
            CaseVariant::Uppercase,
            CaseVariant::Original,
            CaseVariant::Lowercase,
        ],
        prefixes: (0..MAX_PREFIX_OPTIONS)
            .map(|index| format!("p{index}"))
            .collect(),
        suffixes: (0..MAX_SUFFIX_OPTIONS)
            .map(|index| format!("s{index}"))
            .collect(),
        ..GenerationConfig::default()
    };
    assert_valid_config(&config);
    config.case_variants.push(CaseVariant::Original);
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::OptionLimit
    );
    config.case_variants.pop();
    config.prefixes.push("one-more".into());
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::OptionLimit
    );
    config.prefixes.pop();
    config.suffixes.push("one-more".into());
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::OptionLimit
    );
}

#[test]
fn aggregate_configuration_bytes_include_cases_affixes_and_allowances() {
    assert_eq!(MAX_GENERATION_CONFIG_BYTES, 8192);
    // 127 * 64 + 47 affix bytes + one case byte + 16 allowance bytes = 8192.
    let mut config = GenerationConfig {
        prefixes: (0..127)
            .map(|index| format!("{index:03}{}", "P".repeat(61)))
            .collect(),
        suffixes: vec!["S".repeat(47)],
        ..GenerationConfig::default()
    };
    assert_valid_config(&config);
    config.suffixes[0].push('S');
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::TotalBytesLimit
    );
}

#[test]
fn absolute_attempt_and_emission_limits_are_independent_of_path_counts() {
    let mut config = GenerationConfig {
        attempt_limit: MAX_GENERATION_ATTEMPTS,
        emission_limit: MAX_GENERATION_EMISSIONS,
        ..GenerationConfig::default()
    };
    assert_valid_config(&config);
    config.attempt_limit += 1;
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::AttemptLimit
    );
    config.attempt_limit = MAX_GENERATION_ATTEMPTS;
    config.emission_limit += 1;
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::EmissionLimit
    );
    config.attempt_limit = u64::MAX;
    config.emission_limit = u64::MAX;
    assert_eq!(
        config_error(&config).code,
        GenerationConfigErrorCode::AttemptLimit
    );
}

#[test]
fn very_large_potential_path_count_is_processed_incrementally_under_small_allowance() {
    let validated = project(&vec!["Seed"; MAX_ENTITIES]).validate().unwrap();
    let model = validated.resolve();
    let config = GenerationConfig {
        case_variants: vec![
            CaseVariant::Original,
            CaseVariant::Lowercase,
            CaseVariant::Uppercase,
        ],
        prefixes: (0..MAX_PREFIX_OPTIONS)
            .map(|index| format!("p{index}"))
            .collect(),
        suffixes: (0..MAX_SUFFIX_OPTIONS)
            .map(|index| format!("s{index}"))
            .collect(),
        attempt_limit: 3,
        emission_limit: MAX_GENERATION_EMISSIONS,
    };
    let mut deliveries = 0;
    let result = model
        .generate(
            &config,
            |stats| {
                counts(stats);
                false
            },
            |candidate| {
                deliveries += 1;
                assert_eq!(counts(candidate.stats()), (deliveries, 0, 0, deliveries));
                assert_eq!(candidate.path().seed_index, 0);
                assert_eq!(candidate.path().suffix_index, deliveries as usize - 1);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(result.potential_paths(), 201_326_592);
    assert!(result.potential_paths() > MAX_GENERATION_ATTEMPTS);
    assert_eq!(result.reason(), GenerationStop::AttemptLimit);
    assert_eq!(counts(result.stats()), (3, 0, 0, 3));
    assert_eq!(deliveries, 3);
}
