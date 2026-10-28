// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! UI-independent validation, resolution, candidate evaluation, and bounded analysis.
//!
//! Only validated projects can be resolved and evaluated. The API is internal
//! product infrastructure; external API stability is not promised.
//!
//! ```
//! use cluebench_core::RawProject;
//! let project = RawProject::default().validate()?;
//! let model = project.resolve();
//! assert!(model.evaluate("Synthetic1!")?.hard_eligible);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Raw data cannot be evaluated directly:
//!
//! ```compile_fail
//! use cluebench_core::RawProject;
//! RawProject::default().evaluate("Synthetic1!");
//! ```
//!
//! Validated projects and resolved models have no public construction fields:
//!
//! ```compile_fail
//! use cluebench_core::{RawProject, ValidatedProject};
//! let project = ValidatedProject { raw: RawProject::default() };
//! ```
//!
//! ```compile_fail
//! use cluebench_core::ResolvedModel;
//! let model = ResolvedModel {
//!     active_hypotheses: Vec::new(), constraints: Vec::new(), seeds: Vec::new(),
//! };
//! ```
//!
//! Validation cannot be invalidated through a mutable source accessor:
//!
//! ```compile_fail
//! use cluebench_core::RawProject;
//! let mut project = RawProject::default().validate().unwrap();
//! project.raw().hypotheses.clear();
//! ```
//!
//! Analysis also starts only from a resolved, validated scenario:
//!
//! ```
//! use cluebench_core::{AnalysisConfig, AnalysisOutcome, RawProject};
//! let project = RawProject::default().validate()?;
//! let model = project.resolve();
//! let result = model.analyze(AnalysisConfig::default(), |_| false);
//! if let AnalysisOutcome::Satisfiable(witness) = result.outcome() {
//!     assert!(model.evaluate(witness.as_str())?.hard_eligible);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```compile_fail
//! use cluebench_core::{AnalysisConfig, RawProject};
//! RawProject::default().analyze(AnalysisConfig::default(), |_| false);
//! ```
//!
//! Results cannot outlive the resolved scenario that they describe:
//!
//! ```compile_fail
//! use cluebench_core::{AnalysisConfig, RawProject};
//! let project = RawProject::default().validate().unwrap();
//! let result;
//! {
//!     let model = project.resolve();
//!     result = model.analyze(AnalysisConfig::default(), |_| false);
//! }
//! let _ = result.outcome();
//! ```

#![forbid(unsafe_code)]

mod analysis;
mod evaluation;
pub mod limits;
mod matching;
mod model;
mod resolution;
mod validation;

pub use analysis::{
    ANALYSIS_DOMAIN, AnalysisCheckpoint, AnalysisConfig, AnalysisDomain, AnalysisOutcome,
    AnalysisPhase, AnalysisResult, Contradiction, ContradictionCategory, UnknownReason,
    VerifiedWitness,
};
pub use evaluation::{CandidateError, Evaluation};
pub use model::*;
pub use resolution::{ResolvedConstraint, ResolvedModel, ResolvedSeed};
pub use validation::{Diagnostic, DiagnosticCode, ValidatedProject, ValidationErrors};
