// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Bounded, synchronous traversal of resolved seeds and fixed transformations.
//! See `docs/bounded-seed-generation.md` for accounting and resource guarantees.

use std::{fmt, ops::ControlFlow};

use crate::limits::*;
use crate::{Evaluation, ResolvedModel, ResolvedSeed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CaseVariant {
    Original,
    Lowercase,
    Uppercase,
}

/// Caller-owned options. Every run validates the entire borrowed configuration,
/// even with no seeds or zero allowances. No implicit empty affix is inserted.
#[derive(Clone, PartialEq, Eq)]
pub struct GenerationConfig {
    pub case_variants: Vec<CaseVariant>,
    pub prefixes: Vec<String>,
    pub suffixes: Vec<String>,
    pub attempt_limit: u64,
    pub emission_limit: u64,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            case_variants: vec![CaseVariant::Original],
            prefixes: vec![String::new()],
            suffixes: vec![String::new()],
            attempt_limit: 100_000,
            emission_limit: 10_000,
        }
    }
}

impl fmt::Debug for GenerationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationConfig")
            .field("case_options", &self.case_variants.len())
            .field("prefix_options", &self.prefixes.len())
            .field("suffix_options", &self.suffixes.len())
            .field("attempt_limit", &self.attempt_limit)
            .field("emission_limit", &self.emission_limit)
            .finish_non_exhaustive()
    }
}

/// A bounded, content-free diagnostic. `index` identifies an option, never its
/// contents. Validation returns the first error in deterministic field order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationConfigError {
    pub field: &'static str,
    pub code: GenerationConfigErrorCode,
    pub index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationConfigErrorCode {
    EmptyOptions,
    OptionLimit,
    DuplicateOption,
    AffixTooLong,
    UnsupportedAffixByte,
    TotalBytesLimit,
    AttemptLimit,
    EmissionLimit,
    ArithmeticOverflow,
}

impl GenerationConfigError {
    fn new(field: &'static str, code: GenerationConfigErrorCode, index: Option<usize>) -> Self {
        Self { field, code, index }
    }
}

impl fmt::Display for GenerationConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generation configuration {}: {:?}",
            self.field, self.code
        )?;
        if let Some(index) = self.index {
            write!(f, " (option {index})")?;
        }
        Ok(())
    }
}

impl std::error::Error for GenerationConfigError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationStats {
    pub attempted_paths: u64,
    pub domain_skips: u64,
    pub hard_rejections: u64,
    pub emitted_candidates: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationStop {
    /// Only the configured seed/transformation paths have been exhausted.
    Completed,
    Cancelled,
    AttemptLimit,
    OutputLimit,
    ConsumerStopped,
}

/// Option indices belong to the immutable configuration borrowed by the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationPath {
    pub seed_index: usize,
    pub case_index: usize,
    pub prefix_index: usize,
    pub suffix_index: usize,
    pub case_variant: CaseVariant,
}

/// One delivery. Text and evaluation are borrowed only for this callback;
/// retaining either requires an explicit caller copy. Source provenance remains
/// borrowed from the actual validated project, not reconstructed from text.
pub struct GeneratedCandidate<'record, 'run, 'source> {
    text: &'record str,
    evaluation: &'record Evaluation,
    seed: ResolvedSeed<'source>,
    path: GenerationPath,
    stats: GenerationStats,
    model: &'run ResolvedModel<'source>,
    config: &'run GenerationConfig,
}

impl<'record, 'run, 'source> GeneratedCandidate<'record, 'run, 'source> {
    pub fn text(&self) -> &'record str {
        self.text
    }

    pub fn evaluation(&self) -> &'record Evaluation {
        self.evaluation
    }

    pub fn seed(&self) -> ResolvedSeed<'source> {
        self.seed
    }

    pub fn path(&self) -> GenerationPath {
        self.path
    }

    /// Includes this delivery, even if the consumer requests a stop.
    pub fn stats(&self) -> GenerationStats {
        self.stats
    }

    pub fn model(&self) -> &'run ResolvedModel<'source> {
        self.model
    }

    pub fn config(&self) -> &'run GenerationConfig {
        self.config
    }
}

impl fmt::Debug for GeneratedCandidate<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneratedCandidate")
            .field("seed_id", &self.seed.seed.id)
            .field("hypothesis_id", &self.seed.hypothesis.map(|owner| owner.id))
            .field("path", &self.path)
            .field("evaluation", &self.evaluation)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// The committed terminal outcome stays associated with both input snapshots.
/// This summary owns no candidates or source strings.
pub struct GenerationResult<'run, 'source> {
    model: &'run ResolvedModel<'source>,
    config: &'run GenerationConfig,
    reason: GenerationStop,
    stats: GenerationStats,
    potential_paths: u64,
}

impl<'run, 'source> GenerationResult<'run, 'source> {
    pub fn model(&self) -> &'run ResolvedModel<'source> {
        self.model
    }

    pub fn config(&self) -> &'run GenerationConfig {
        self.config
    }

    pub fn reason(&self) -> GenerationStop {
        self.reason
    }

    pub fn stats(&self) -> GenerationStats {
        self.stats
    }

    /// Configured paths, including duplicates and paths that may be skipped.
    /// This is not candidate-domain cardinality or a count of eligible outputs.
    pub fn potential_paths(&self) -> u64 {
        self.potential_paths
    }
}

impl fmt::Debug for GenerationResult<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationResult")
            .field("reason", &self.reason)
            .field("stats", &self.stats)
            .field("potential_paths", &self.potential_paths)
            .finish_non_exhaustive()
    }
}

impl<'source> ResolvedModel<'source> {
    /// Stream eligible candidates in seed/case/prefix/suffix order, with suffix
    /// innermost. Cancellation is polled only between fully classified paths.
    /// `Break(())` commits consumer termination immediately after that delivery.
    ///
    /// ```
    /// use cluebench_core::{GenerationConfig, GenerationStop, RawProject, Seed, Id};
    /// use std::ops::ControlFlow;
    /// let project = RawProject {
    ///     manual_seeds: vec![Seed { id: Id(1), text: "Synthetic1!".into() }],
    ///     ..RawProject::default()
    /// }.validate()?;
    /// let model = project.resolve();
    /// let config = GenerationConfig::default();
    /// let result = model.generate(&config, |_| false, |record| {
    ///     assert_eq!(record.text(), "Synthetic1!");
    ///     ControlFlow::Continue(())
    /// })?;
    /// assert_eq!(result.reason(), GenerationStop::Completed);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Unvalidated input cannot start generation:
    ///
    /// ```compile_fail
    /// use cluebench_core::{GenerationConfig, RawProject};
    /// use std::ops::ControlFlow;
    /// RawProject::default().generate(&GenerationConfig::default(), |_| false,
    ///     |_| ControlFlow::Continue(()));
    /// ```
    ///
    /// A delivery's temporary text cannot escape the callback:
    ///
    /// ```compile_fail
    /// use cluebench_core::{GenerationConfig, RawProject};
    /// use std::ops::ControlFlow;
    /// let project = RawProject::default().validate().unwrap();
    /// let model = project.resolve();
    /// let mut saved = None;
    /// model.generate(&GenerationConfig::default(), |_| false, |record| {
    ///     saved = Some(record.text());
    ///     ControlFlow::Continue(())
    /// });
    /// println!("{}", saved.unwrap());
    /// ```
    pub fn generate<'run>(
        &'run self,
        config: &'run GenerationConfig,
        mut cancel: impl FnMut(GenerationStats) -> bool,
        mut consume: impl FnMut(GeneratedCandidate<'_, 'run, 'source>) -> ControlFlow<()>,
    ) -> Result<GenerationResult<'run, 'source>, GenerationConfigError> {
        validate_config(config)?;
        let potential_paths = checked_path_count([
            self.seeds().len(),
            config.case_variants.len(),
            config.prefixes.len(),
            config.suffixes.len(),
        ])?;
        let mut stats = GenerationStats::default();
        let mut text = String::new();
        let reason = loop {
            // Never observe cancellation or allowances after path exhaustion.
            if stats.attempted_paths == potential_paths {
                break GenerationStop::Completed;
            }
            if cancel(stats) {
                break GenerationStop::Cancelled;
            }
            if stats.attempted_paths >= config.attempt_limit {
                break GenerationStop::AttemptLimit;
            }
            if stats.emitted_candidates >= config.emission_limit {
                break GenerationStop::OutputLimit;
            }

            // Decode one ordinal, without allocating a list of paths or probing
            // any later candidate. Bounded option counts make these casts safe.
            let path = path_at(stats.attempted_paths, config);
            let seed = self.seeds()[path.seed_index];
            stats.attempted_paths += 1;
            let prefix = &config.prefixes[path.prefix_index];
            let suffix = &config.suffixes[path.suffix_index];
            let length = checked_candidate_length(prefix.len(), seed.seed.text.len(), suffix.len());
            if !matches!(length, Some(1..=MAX_CANDIDATE_BYTES)) {
                stats.domain_skips += 1;
                continue;
            }

            // Every requested candidate allocation is at most 64 bytes. No
            // transformed string is ever reused as a seed or saved by the core.
            text.clear();
            if text.capacity() == 0 {
                text.reserve_exact(MAX_CANDIDATE_BYTES);
            }
            text.push_str(prefix);
            for byte in seed.seed.text.bytes() {
                let converted = match path.case_variant {
                    CaseVariant::Original => byte,
                    CaseVariant::Lowercase => byte.to_ascii_lowercase(),
                    CaseVariant::Uppercase => byte.to_ascii_uppercase(),
                };
                text.push(char::from(converted));
            }
            text.push_str(suffix);
            if text.is_empty() || !text.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                stats.domain_skips += 1;
                continue;
            }

            let evaluation = self
                .evaluate(&text)
                .expect("validated generation candidate remains inside the candidate domain");
            if !evaluation.hard_eligible {
                stats.hard_rejections += 1;
                continue;
            }
            // Classification precedes the external callback: it observes the
            // accounting invariant and includes the record being delivered.
            stats.emitted_candidates += 1;
            if consume(GeneratedCandidate {
                text: &text,
                evaluation: &evaluation,
                seed,
                path,
                stats,
                model: self,
                config,
            })
            .is_break()
            {
                break GenerationStop::ConsumerStopped;
            }
        };
        Ok(GenerationResult {
            model: self,
            config,
            reason,
            stats,
            potential_paths,
        })
    }
}

fn path_at(mut ordinal: u64, config: &GenerationConfig) -> GenerationPath {
    let suffix_index = (ordinal % config.suffixes.len() as u64) as usize;
    ordinal /= config.suffixes.len() as u64;
    let prefix_index = (ordinal % config.prefixes.len() as u64) as usize;
    ordinal /= config.prefixes.len() as u64;
    let case_index = (ordinal % config.case_variants.len() as u64) as usize;
    ordinal /= config.case_variants.len() as u64;
    GenerationPath {
        seed_index: ordinal as usize,
        case_index,
        prefix_index,
        suffix_index,
        case_variant: config.case_variants[case_index],
    }
}

fn validate_config(config: &GenerationConfig) -> Result<(), GenerationConfigError> {
    use GenerationConfigErrorCode as Code;
    // Size preflight precedes content inspection and duplicate comparisons.
    for (field, count, maximum) in [
        (
            "case_variants",
            config.case_variants.len(),
            MAX_CASE_OPTIONS,
        ),
        ("prefixes", config.prefixes.len(), MAX_PREFIX_OPTIONS),
        ("suffixes", config.suffixes.len(), MAX_SUFFIX_OPTIONS),
    ] {
        if count == 0 {
            return Err(GenerationConfigError::new(field, Code::EmptyOptions, None));
        }
        if count > maximum {
            return Err(GenerationConfigError::new(field, Code::OptionLimit, None));
        }
    }
    if config.attempt_limit > MAX_GENERATION_ATTEMPTS {
        return Err(GenerationConfigError::new(
            "attempt_limit",
            Code::AttemptLimit,
            None,
        ));
    }
    if config.emission_limit > MAX_GENERATION_EMISSIONS {
        return Err(GenerationConfigError::new(
            "emission_limit",
            Code::EmissionLimit,
            None,
        ));
    }
    let mut total_bytes = checked_config_bytes(16, config.case_variants.len())?;
    for (field, options) in [
        ("prefixes", &config.prefixes),
        ("suffixes", &config.suffixes),
    ] {
        for (index, affix) in options.iter().enumerate() {
            if affix.len() > MAX_AFFIX_BYTES {
                return Err(GenerationConfigError::new(
                    field,
                    Code::AffixTooLong,
                    Some(index),
                ));
            }
            total_bytes = checked_config_bytes(total_bytes, affix.len())?;
            if total_bytes > MAX_GENERATION_CONFIG_BYTES {
                return Err(GenerationConfigError::new(
                    "total_bytes",
                    Code::TotalBytesLimit,
                    None,
                ));
            }
        }
    }
    for (index, variant) in config.case_variants.iter().enumerate() {
        if config.case_variants[..index].contains(variant) {
            return Err(GenerationConfigError::new(
                "case_variants",
                Code::DuplicateOption,
                Some(index),
            ));
        }
    }
    for (field, options) in [
        ("prefixes", &config.prefixes),
        ("suffixes", &config.suffixes),
    ] {
        for (index, affix) in options.iter().enumerate() {
            if !affix.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                return Err(GenerationConfigError::new(
                    field,
                    Code::UnsupportedAffixByte,
                    Some(index),
                ));
            }
            if options[..index].contains(affix) {
                return Err(GenerationConfigError::new(
                    field,
                    Code::DuplicateOption,
                    Some(index),
                ));
            }
        }
    }
    Ok(())
}

fn checked_config_bytes(total: usize, additional: usize) -> Result<usize, GenerationConfigError> {
    total.checked_add(additional).ok_or_else(|| {
        GenerationConfigError::new(
            "total_bytes",
            GenerationConfigErrorCode::ArithmeticOverflow,
            None,
        )
    })
}

fn checked_path_count(counts: [usize; 4]) -> Result<u64, GenerationConfigError> {
    let overflow = || {
        GenerationConfigError::new(
            "potential_paths",
            GenerationConfigErrorCode::ArithmeticOverflow,
            None,
        )
    };
    counts.into_iter().try_fold(1u64, |total, count| {
        let count = u64::try_from(count).map_err(|_| overflow())?;
        total.checked_mul(count).ok_or_else(overflow)
    })
}

fn checked_candidate_length(prefix: usize, seed: usize, suffix: usize) -> Option<usize> {
    prefix.checked_add(seed)?.checked_add(suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_arithmetic_is_checked_without_allocating_extreme_inputs() {
        assert_eq!(
            checked_config_bytes(usize::MAX, 1).unwrap_err().code,
            GenerationConfigErrorCode::ArithmeticOverflow
        );
        assert_eq!(
            checked_path_count([usize::MAX, usize::MAX, 3, 128])
                .unwrap_err()
                .code,
            GenerationConfigErrorCode::ArithmeticOverflow
        );
        assert_eq!(
            checked_path_count([
                MAX_ENTITIES,
                MAX_CASE_OPTIONS,
                MAX_PREFIX_OPTIONS,
                MAX_SUFFIX_OPTIONS
            ]),
            Ok(201_326_592)
        );
        assert_eq!(checked_path_count([0, 3, 128, 128]), Ok(0));
        assert_eq!(checked_candidate_length(usize::MAX, 1, 0), None);
        assert_eq!(checked_candidate_length(0, usize::MAX, 1), None);
        assert_eq!(checked_candidate_length(1, 62, 1), Some(64));
        assert_eq!(checked_candidate_length(1, 63, 1), Some(65));
    }
}
