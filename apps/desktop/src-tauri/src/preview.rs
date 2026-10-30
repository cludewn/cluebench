// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Bounded, manual-only one-shot preview. No secret-bearing type implements Debug.

use std::{
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use cluebench_core::{
    CaseVariant, GenerationConfig, GenerationStats, GenerationStop, Id, RawProject, Seed,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request, Response};

pub(crate) const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub(crate) const MAX_REQUEST_ID_BYTES: usize = 32;
pub(crate) const MAX_SEEDS: usize = 64;
pub(crate) const MAX_SEED_BYTES: usize = 64;
pub(crate) const MAX_TOTAL_SEED_BYTES: usize = 4096;
pub(crate) const MAX_CASE_OPTIONS: usize = 3;
pub(crate) const MAX_AFFIX_OPTIONS: usize = 8;
pub(crate) const MAX_AFFIX_BYTES: usize = 64;
pub(crate) const MAX_CONFIG_BYTES: usize = 1043;
pub(crate) const DEFAULT_ATTEMPT_LIMIT: u64 = 2048;
pub(crate) const MAX_ATTEMPT_LIMIT: u64 = 4096;
pub(crate) const DEFAULT_EMISSION_LIMIT: u64 = 50;
pub(crate) const MAX_EMISSION_LIMIT: u64 = 100;
pub(crate) const MAX_CANDIDATES: usize = 100;
pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum PreviewError {
    #[serde(rename = "PREVIEW_INVALID_BODY")]
    InvalidBody,
    #[serde(rename = "PREVIEW_REQUEST_TOO_LARGE")]
    RequestTooLarge,
    #[serde(rename = "PREVIEW_INVALID_REQUEST")]
    InvalidRequest,
    #[serde(rename = "PREVIEW_BUSY")]
    Busy,
    #[serde(rename = "PREVIEW_CORE_FAILED")]
    CoreFailed,
    #[serde(rename = "PREVIEW_WORKER_FAILED")]
    WorkerFailed,
    #[serde(rename = "PREVIEW_RESPONSE_TOO_LARGE")]
    ResponseTooLarge,
}

#[derive(Default)]
pub(crate) struct PreviewState(Arc<AtomicBool>);

struct PreviewGuard(Arc<AtomicBool>);

impl PreviewState {
    fn acquire(&self) -> Result<PreviewGuard, PreviewError> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| PreviewError::Busy)?;
        Ok(PreviewGuard(Arc::clone(&self.0)))
    }
}

impl Drop for PreviewGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum PreviewCase {
    Original,
    AsciiLowercase,
    AsciiUppercase,
}

impl From<PreviewCase> for CaseVariant {
    fn from(value: PreviewCase) -> Self {
        match value {
            PreviewCase::Original => Self::Original,
            PreviewCase::AsciiLowercase => Self::Lowercase,
            PreviewCase::AsciiUppercase => Self::Uppercase,
        }
    }
}

impl From<CaseVariant> for PreviewCase {
    fn from(value: CaseVariant) -> Self {
        match value {
            CaseVariant::Original => Self::Original,
            CaseVariant::Lowercase => Self::AsciiLowercase,
            CaseVariant::Uppercase => Self::AsciiUppercase,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewRequest {
    request_id: String,
    seeds: Vec<String>,
    options: PreviewOptions,
    limits: PreviewLimits,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewOptions {
    case_variants: Vec<PreviewCase>,
    prefixes: Vec<String>,
    suffixes: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewLimits {
    #[serde(default = "default_attempt_limit")]
    attempt_limit: u64,
    #[serde(default = "default_emission_limit")]
    emission_limit: u64,
}

fn default_attempt_limit() -> u64 {
    DEFAULT_ATTEMPT_LIMIT
}
fn default_emission_limit() -> u64 {
    DEFAULT_EMISSION_LIMIT
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResponse {
    request_id: String,
    effective_limits: PreviewLimits,
    candidates: Vec<PreviewCandidate>,
    stats: PreviewStats,
    potential_paths: u64,
    stop: PreviewStop,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewCandidate {
    text: String,
    seed_id: u32,
    path: PreviewPath,
    prefix: String,
    suffix: String,
    relative_points: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewPath {
    seed_index: usize,
    case_index: usize,
    case_variant: PreviewCase,
    prefix_index: usize,
    suffix_index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewStats {
    attempted_paths: u64,
    domain_skips: u64,
    hard_rejections: u64,
    emitted_candidates: u64,
}

impl From<GenerationStats> for PreviewStats {
    fn from(value: GenerationStats) -> Self {
        Self {
            attempted_paths: value.attempted_paths,
            domain_skips: value.domain_skips,
            hard_rejections: value.hard_rejections,
            emitted_candidates: value.emitted_candidates,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum PreviewStop {
    Completed,
    AttemptLimit,
    OutputLimit,
    Cancelled,
    ConsumerStopped,
}

impl From<GenerationStop> for PreviewStop {
    fn from(value: GenerationStop) -> Self {
        match value {
            GenerationStop::Completed => Self::Completed,
            GenerationStop::AttemptLimit => Self::AttemptLimit,
            GenerationStop::OutputLimit => Self::OutputLimit,
            GenerationStop::Cancelled => Self::Cancelled,
            GenerationStop::ConsumerStopped => Self::ConsumerStopped,
        }
    }
}

fn bounded_body(body: &InvokeBody) -> Result<Vec<u8>, PreviewError> {
    let InvokeBody::Raw(bytes) = body else {
        return Err(PreviewError::InvalidBody);
    };
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(PreviewError::RequestTooLarge);
    }
    // Copy only after checking length. Transport allocations precede this boundary.
    Ok(bytes.clone())
}

fn printable_ascii(text: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&text.len()) && text.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn unique<T: PartialEq>(options: &[T]) -> bool {
    options
        .iter()
        .enumerate()
        .all(|(index, value)| !options[..index].contains(value))
}

impl PreviewRequest {
    fn validate(&self) -> Result<(), PreviewError> {
        let options = &self.options;
        let limits = &self.limits;
        let id_valid = !self.request_id.is_empty()
            && self.request_id.len() <= MAX_REQUEST_ID_BYTES
            && self
                .request_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte));
        let seeds_valid = (1..=MAX_SEEDS).contains(&self.seeds.len())
            && self
                .seeds
                .iter()
                .all(|seed| printable_ascii(seed, 1, MAX_SEED_BYTES))
            && self.seeds.iter().map(String::len).sum::<usize>() <= MAX_TOTAL_SEED_BYTES;
        let cases_valid = (1..=MAX_CASE_OPTIONS).contains(&options.case_variants.len())
            && unique(&options.case_variants);
        let affixes_valid = [&options.prefixes, &options.suffixes]
            .iter()
            .all(|affixes| {
                (1..=MAX_AFFIX_OPTIONS).contains(&affixes.len())
                    && affixes
                        .iter()
                        .all(|affix| printable_ascii(affix, 0, MAX_AFFIX_BYTES))
                    && unique(affixes)
            });
        let config_bytes = 16
            + options.case_variants.len()
            + options.prefixes.iter().map(String::len).sum::<usize>()
            + options.suffixes.iter().map(String::len).sum::<usize>();
        if !(id_valid
            && seeds_valid
            && cases_valid
            && affixes_valid
            && config_bytes <= MAX_CONFIG_BYTES
            && (1..=MAX_ATTEMPT_LIMIT).contains(&limits.attempt_limit)
            && (1..=MAX_EMISSION_LIMIT).contains(&limits.emission_limit))
        {
            return Err(PreviewError::InvalidRequest);
        }
        Ok(())
    }
}

fn generate_preview(bytes: Vec<u8>) -> Result<String, PreviewError> {
    let request: PreviewRequest =
        serde_json::from_slice(&bytes).map_err(|_| PreviewError::InvalidRequest)?;
    request.validate()?;
    let config = GenerationConfig {
        case_variants: request
            .options
            .case_variants
            .into_iter()
            .map(Into::into)
            .collect(),
        prefixes: request.options.prefixes,
        suffixes: request.options.suffixes,
        attempt_limit: request.limits.attempt_limit,
        emission_limit: request.limits.emission_limit,
    };
    let project = RawProject {
        manual_seeds: request
            .seeds
            .into_iter()
            .enumerate()
            .map(|(index, text)| Seed {
                id: Id((index + 1) as u32),
                text,
            })
            .collect(),
        ..RawProject::default()
    }
    .validate()
    .map_err(|_| PreviewError::CoreFailed)?;
    let model = project.resolve();
    let mut candidates = Vec::with_capacity(config.emission_limit as usize);
    let result = model
        .generate(
            &config,
            |_| false,
            |record| {
                let path = record.path();
                candidates.push(PreviewCandidate {
                    text: record.text().to_owned(),
                    seed_id: record.seed().seed.id.0,
                    path: PreviewPath {
                        seed_index: path.seed_index,
                        case_index: path.case_index,
                        case_variant: path.case_variant.into(),
                        prefix_index: path.prefix_index,
                        suffix_index: path.suffix_index,
                    },
                    prefix: record.config().prefixes[path.prefix_index].clone(),
                    suffix: record.config().suffixes[path.suffix_index].clone(),
                    relative_points: record.evaluation().relative_points,
                });
                ControlFlow::Continue(())
            },
        )
        .map_err(|_| PreviewError::CoreFailed)?;
    if candidates.len() > MAX_CANDIDATES
        || candidates.len() as u64 != result.stats().emitted_candidates
    {
        return Err(PreviewError::CoreFailed);
    }
    let response = PreviewResponse {
        request_id: request.request_id,
        effective_limits: request.limits,
        candidates,
        stats: result.stats().into(),
        potential_paths: result.potential_paths(),
        stop: result.reason().into(),
    };
    serialize_response(&response)
}

fn serialize_response(response: &PreviewResponse) -> Result<String, PreviewError> {
    let serialized = serde_json::to_string(response).map_err(|_| PreviewError::CoreFailed)?;
    if serialized.len() > MAX_RESPONSE_BYTES {
        return Err(PreviewError::ResponseTooLarge);
    }
    Ok(serialized)
}

fn start_worker<T: Send + 'static>(
    state: &PreviewState,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<tauri::async_runtime::JoinHandle<T>, PreviewError> {
    let guard = state.acquire()?;
    Ok(tauri::async_runtime::spawn_blocking(move || {
        // The worker owns the slot through actual completion/unwinding. Dropping
        // or aborting an IPC caller's future cannot release an active worker.
        let _guard = guard;
        work()
    }))
}

async fn finish_worker<T>(
    worker: tauri::async_runtime::JoinHandle<Result<T, PreviewError>>,
) -> Result<T, PreviewError> {
    worker.await.map_err(|_| PreviewError::WorkerFailed)?
}

async fn execute(body: &InvokeBody, state: &PreviewState) -> Result<Response, PreviewError> {
    let bytes = bounded_body(body)?;
    let worker = start_worker(state, move || generate_preview(bytes))?;
    let serialized = finish_worker(worker).await?;
    // A String is a JSON InvokeResponseBody; Vec<u8> would be an ArrayBuffer.
    Ok(Response::new(serialized))
}

#[tauri::command]
pub(crate) async fn candidate_preview(
    request: Request<'_>,
    state: tauri::State<'_, PreviewState>,
) -> Result<Response, PreviewError> {
    execute(request.body(), &state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        future::Future,
        sync::mpsc,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    use tauri::ipc::IpcResponse;

    fn fixture() -> Value {
        json!({
            "requestId": "preview-1",
            "seeds": ["Ab"],
            "options": {"caseVariants": ["original"], "prefixes": [""], "suffixes": [""]},
            "limits": {"attemptLimit": 2048, "emissionLimit": 50}
        })
    }

    fn generate(value: &Value) -> Result<Value, PreviewError> {
        let bytes = bounded_body(&InvokeBody::Raw(serde_json::to_vec(value).unwrap()))?;
        serde_json::from_str(&generate_preview(bytes)?).map_err(|_| PreviewError::CoreFailed)
    }

    fn assert_invalid(value: Value) {
        assert_eq!(generate(&value), Err(PreviewError::InvalidRequest));
    }

    fn assert_accounting(value: &Value) {
        let stats = &value["stats"];
        assert_eq!(
            stats["attemptedPaths"].as_u64().unwrap(),
            stats["domainSkips"].as_u64().unwrap()
                + stats["hardRejections"].as_u64().unwrap()
                + stats["emittedCandidates"].as_u64().unwrap()
        );
        assert_eq!(
            value["candidates"].as_array().unwrap().len() as u64,
            stats["emittedCandidates"].as_u64().unwrap()
        );
    }

    #[test]
    fn raw_request_uses_actual_core_and_returns_owned_json_response() {
        let state = PreviewState::default();
        let body = InvokeBody::Raw(serde_json::to_vec(&fixture()).unwrap());
        let response = tauri::async_runtime::block_on(execute(&body, &state)).unwrap();
        let value: Value = response.body().unwrap().deserialize().unwrap();
        assert_eq!(value["requestId"], "preview-1");
        assert_eq!(
            value["candidates"][0],
            json!({
                "text":"Ab", "seedId":1,
                "path":{"seedIndex":0,"caseIndex":0,"caseVariant":"original","prefixIndex":0,"suffixIndex":0},
                "prefix":"", "suffix":"", "relativePoints":0
            })
        );
        assert_eq!(value["stop"], "completed");
        assert_eq!(value["potentialPaths"], 1);
        assert_accounting(&value);
        // All source model/config/record borrows have ended before JSON is used.
        assert_eq!(value["candidates"][0]["text"], "Ab");
    }

    #[test]
    fn raw_boundary_rejects_json_and_overlong_body_before_parsing() {
        assert_eq!(
            bounded_body(&InvokeBody::Json(fixture())),
            Err(PreviewError::InvalidBody)
        );
        assert_eq!(
            bounded_body(&InvokeBody::Raw(vec![b'x'; MAX_REQUEST_BYTES + 1])),
            Err(PreviewError::RequestTooLarge)
        );
        let mut bytes = serde_json::to_vec(&fixture()).unwrap();
        bytes.resize(MAX_REQUEST_BYTES, b' ');
        assert!(generate_preview(bounded_body(&InvokeBody::Raw(bytes)).unwrap()).is_ok());
    }

    #[test]
    fn malformed_unknown_variant_unknown_fields_and_wrong_types_are_fixed_errors() {
        for bytes in [b"{".to_vec(), vec![0xff], b"null".to_vec(), b"[]".to_vec()] {
            assert_eq!(generate_preview(bytes), Err(PreviewError::InvalidRequest));
        }
        for (pointer, bad) in [
            ("/seeds", json!("secret-fixture")),
            ("/seeds/0", json!(42)),
            ("/options/caseVariants/0", json!("unknown")),
            ("/options/prefixes", json!(null)),
            ("/limits/attemptLimit", json!(1.5)),
            ("/limits/emissionLimit", json!(-1)),
            ("/limits/attemptLimit", json!("4096")),
            ("/requestId", json!(23)),
        ] {
            let mut value = fixture();
            *value.pointer_mut(pointer).unwrap() = bad;
            assert_invalid(value);
        }
        for pointer in ["", "/options", "/limits"] {
            let mut value = fixture();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), json!("secret-fixture"));
            assert_invalid(value);
        }
        let duplicate = String::from_utf8(serde_json::to_vec(&fixture()).unwrap())
            .unwrap()
            .replacen(
                "\"requestId\":",
                "\"requestId\":\"duplicate\",\"requestId\":",
                1,
            );
        assert_eq!(
            generate_preview(duplicate.into_bytes()),
            Err(PreviewError::InvalidRequest)
        );
    }

    #[test]
    fn request_ids_are_bounded_ascii_identifiers() {
        for id in [
            "".into(),
            "x".repeat(33),
            "é".into(),
            "id\n".into(),
            "has space".into(),
        ] {
            let mut value = fixture();
            value["requestId"] = json!(id);
            assert_invalid(value);
        }
        let mut value = fixture();
        value["requestId"] = json!("x".repeat(MAX_REQUEST_ID_BYTES));
        assert!(generate(&value).is_ok());
    }

    #[test]
    fn seed_count_byte_domain_spaces_and_duplicate_identity_are_enforced() {
        for seeds in [
            vec![],
            vec!["x".to_string(); 65],
            vec!["x".repeat(65)],
            vec!["".into()],
            vec!["é".into()],
            vec!["A\t".into()],
            vec!["A\n".into()],
            vec!["\u{7f}".into()],
        ] {
            let mut value = fixture();
            value["seeds"] = json!(seeds);
            assert_invalid(value);
        }
        let mut value = fixture();
        value["seeds"] = json!(vec!["x".repeat(64); 64]);
        let output = generate(&value).unwrap();
        assert_eq!(output["potentialPaths"], 64);
        assert_eq!(output["stop"], "outputLimit");
        value["seeds"] = json!([" A ", " ", " A "]);
        let output = generate(&value).unwrap();
        let records = output["candidates"].as_array().unwrap();
        assert_eq!(
            records
                .iter()
                .map(|record| record["text"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [" A ", " ", " A "]
        );
        assert_eq!(
            records
                .iter()
                .map(|record| record["seedId"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_accounting(&output);
    }

    #[test]
    fn case_and_affix_options_are_explicit_ordered_unique_and_bounded() {
        for cases in [
            json!([]),
            json!(["original", "original"]),
            json!(["original", "asciiLowercase", "asciiUppercase", "original"]),
        ] {
            let mut value = fixture();
            value["options"]["caseVariants"] = cases;
            assert_invalid(value);
        }
        for field in ["prefixes", "suffixes"] {
            for affixes in [
                json!([]),
                json!(["", ""]),
                json!(vec!["x"; 9]),
                json!(["x".repeat(65)]),
                json!(["é"]),
                json!(["\t"]),
            ] {
                let mut value = fixture();
                value["options"][field] = affixes;
                assert_invalid(value);
            }
        }
        let mut value = fixture();
        value["options"] = json!({"caseVariants":["asciiUppercase","asciiLowercase"],"prefixes":["", " "],"suffixes":["!", ""]});
        let output = generate(&value).unwrap();
        assert_eq!(output["candidates"][0]["text"], "AB!");
        assert_eq!(output["candidates"][1]["text"], "AB");
        assert_eq!(output["candidates"][2]["text"], " AB!");
        assert_eq!(output["candidates"][4]["text"], "ab!");
        assert_eq!(output["candidates"][4]["path"]["caseIndex"], 1);
        assert_eq!(
            output["candidates"][0]["path"]["caseVariant"],
            "asciiUppercase"
        );
        assert_accounting(&output);
    }

    #[test]
    fn maximum_logical_config_is_accepted_without_changing_core_limits() {
        let mut value = fixture();
        let affixes: Vec<_> = (0..8)
            .map(|index| format!("{index}{}", "x".repeat(63)))
            .collect();
        value["options"] = json!({"caseVariants":["original","asciiLowercase","asciiUppercase"],"prefixes":affixes,"suffixes":affixes});
        let output = generate(&value).unwrap();
        assert_eq!(output["potentialPaths"], 192);
        assert_eq!(output["stats"]["domainSkips"], 192);
        assert!(output["candidates"].as_array().unwrap().is_empty());
        assert_eq!(output["stop"], "completed");
        assert_accounting(&output);
    }

    #[test]
    fn preview_limits_are_positive_and_independent_with_documented_defaults() {
        for field in ["attemptLimit", "emissionLimit"] {
            let max = if field == "attemptLimit" {
                MAX_ATTEMPT_LIMIT
            } else {
                MAX_EMISSION_LIMIT
            };
            for bad in [0, max + 1, u64::MAX] {
                let mut value = fixture();
                value["limits"][field] = json!(bad);
                assert_invalid(value);
            }
            let mut value = fixture();
            value["limits"][field] = json!(max);
            assert!(generate(&value).is_ok());
        }
        let mut value = fixture();
        value["limits"] = json!({});
        let output = generate(&value).unwrap();
        assert_eq!(
            output["effectiveLimits"],
            json!({"attemptLimit":2048,"emissionLimit":50})
        );
        value["seeds"] = json!(["A", "B", "C"]);
        value["limits"] = json!({"attemptLimit":1,"emissionLimit":100});
        let output = generate(&value).unwrap();
        assert_eq!(output["stop"], "attemptLimit");
        assert_eq!(output["stats"]["attemptedPaths"], 1);
        assert_accounting(&output);
    }

    #[test]
    fn deterministic_order_provenance_affix_case_and_duplicate_paths_are_preserved() {
        let mut value = fixture();
        value["seeds"] = json!(["aB", "aB"]);
        value["options"] = json!({"caseVariants":["asciiLowercase","asciiUppercase"],"prefixes":["PRE-", ""],"suffixes":["-X", ""]});
        let output = generate(&value).unwrap();
        let repeat = generate(&value).unwrap();
        assert_eq!(output, repeat);
        let records = output["candidates"].as_array().unwrap();
        assert_eq!(records.len(), 16);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record["seedId"], index / 8 + 1);
            assert_eq!(record["path"]["seedIndex"], index / 8);
            assert_eq!(record["path"]["caseIndex"], (index / 4) % 2);
            assert_eq!(record["path"]["prefixIndex"], (index / 2) % 2);
            assert_eq!(record["path"]["suffixIndex"], index % 2);
            assert_eq!(record["relativePoints"], 0);
            assert!(record.get("hypothesisId").is_none());
            assert!(record.get("clueId").is_none());
        }
        assert_eq!(records[0]["text"], "PRE-ab-X");
        assert_eq!(records[0]["prefix"], "PRE-");
        assert_eq!(records[0]["suffix"], "-X");
        assert_eq!(records[4]["text"], "PRE-AB-X");
        assert_eq!(records[0]["text"], records[8]["text"]);
        assert_ne!(records[0]["path"], records[8]["path"]);
        assert_accounting(&output);
    }

    #[test]
    fn maximum_100_records_stops_by_core_output_limit_and_exact_exhaustion_completes() {
        let mut value = fixture();
        value["seeds"] = json!(vec!["1"; 50]);
        value["options"]["caseVariants"] = json!(["original", "asciiLowercase"]);
        value["limits"]["emissionLimit"] = json!(100);
        let complete = generate(&value).unwrap();
        assert_eq!(complete["candidates"].as_array().unwrap().len(), 100);
        assert_eq!(complete["stop"], "completed");
        assert_eq!(complete["stats"]["emittedCandidates"], 100);
        assert!(serde_json::to_vec(&complete).unwrap().len() <= MAX_RESPONSE_BYTES);
        value["seeds"].as_array_mut().unwrap().push(json!("1"));
        let limited = generate(&value).unwrap();
        assert_eq!(limited["candidates"].as_array().unwrap().len(), 100);
        assert_eq!(limited["stop"], "outputLimit");
        assert_accounting(&complete);
        assert_accounting(&limited);
    }

    #[test]
    fn all_domain_skips_still_obey_attempt_limit_and_empty_keeps_stop() {
        let mut value = fixture();
        value["seeds"] = json!(vec!["x".repeat(64); 64]);
        value["options"]["caseVariants"] = json!(["original", "asciiLowercase", "asciiUppercase"]);
        value["options"]["prefixes"] = json!(["a", "b", "c", "d", "e", "f", "g", "h"]);
        value["options"]["suffixes"] = json!(["a", "b", "c", "d", "e", "f", "g", "h"]);
        value["limits"] = json!({"attemptLimit":4096,"emissionLimit":1});
        let output = generate(&value).unwrap();
        assert_eq!(output["potentialPaths"], 12288);
        assert_eq!(
            output["stats"],
            json!({"attemptedPaths":4096,"domainSkips":4096,"hardRejections":0,"emittedCandidates":0})
        );
        assert_eq!(output["stop"], "attemptLimit");
        assert!(output["candidates"].as_array().unwrap().is_empty());
        assert_accounting(&output);
    }

    #[test]
    fn response_byte_guard_accepts_exact_boundary_and_rejects_instead_of_truncating() {
        let mut response = PreviewResponse {
            request_id: String::new(),
            effective_limits: PreviewLimits {
                attempt_limit: 2048,
                emission_limit: 50,
            },
            candidates: vec![],
            stats: GenerationStats::default().into(),
            potential_paths: 0,
            stop: PreviewStop::Completed,
        };
        let overhead = serialize_response(&response).unwrap().len();
        // Artificial DTO tests a defensive output boundary beyond valid requests.
        response.request_id = "x".repeat(MAX_RESPONSE_BYTES - overhead);
        assert_eq!(
            serialize_response(&response).unwrap().len(),
            MAX_RESPONSE_BYTES
        );
        response.request_id.push('x');
        assert_eq!(
            serialize_response(&response),
            Err(PreviewError::ResponseTooLarge)
        );
    }

    #[test]
    fn core_stop_mapping_is_exact_and_does_not_invent_cancellation() {
        for (core, expected) in [
            (GenerationStop::Completed, "completed"),
            (GenerationStop::AttemptLimit, "attemptLimit"),
            (GenerationStop::OutputLimit, "outputLimit"),
            (GenerationStop::Cancelled, "cancelled"),
            (GenerationStop::ConsumerStopped, "consumerStopped"),
        ] {
            assert_eq!(
                serde_json::to_value(PreviewStop::from(core)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn single_flight_denies_concurrent_workers_without_queue_then_releases() {
        let state = Arc::new(PreviewState::default());
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = start_worker(&state, move || {
            started_tx.send(()).unwrap();
            finish_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok::<_, PreviewError>(())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let competitors: Vec<_> = (0..16)
            .map(|_| {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    start_worker(&state, || panic!("busy work must never start")).err()
                })
            })
            .collect();
        for competitor in competitors {
            assert_eq!(competitor.join().unwrap(), Some(PreviewError::Busy));
        }
        finish_tx.send(()).unwrap();
        tauri::async_runtime::block_on(finish_worker(worker)).unwrap();
        let next = start_worker(&state, || Ok::<_, PreviewError>(())).unwrap();
        tauri::async_runtime::block_on(finish_worker(next)).unwrap();
    }

    #[test]
    fn dropped_awaiter_keeps_slot_until_worker_actually_finishes() {
        let state = PreviewState::default();
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = start_worker(&state, move || {
            started_tx.send(()).unwrap();
            finish_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok::<_, PreviewError>(())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut awaiter = Box::pin(finish_worker(worker));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(awaiter.as_mut().poll(&mut context), Poll::Pending));
        drop(awaiter);
        assert_eq!(state.acquire().err(), Some(PreviewError::Busy));
        finish_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while state.0.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(state.acquire().is_ok());
    }

    #[test]
    fn aborting_started_blocking_worker_does_not_cancel_computation_or_release_slot() {
        let state = PreviewState::default();
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = start_worker(&state, move || {
            started_tx.send(()).unwrap();
            finish_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok::<_, PreviewError>(())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        worker.abort();
        assert_eq!(state.acquire().err(), Some(PreviewError::Busy));
        finish_tx.send(()).unwrap();
        tauri::async_runtime::block_on(finish_worker(worker)).unwrap();
        assert!(state.acquire().is_ok());
    }

    #[test]
    fn worker_error_and_panic_join_error_are_safe_and_release_slot_for_retry() {
        let state = PreviewState::default();
        let invalid =
            start_worker(&state, || generate_preview(b"{secret-fixture".to_vec())).unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(finish_worker(invalid)),
            Err(PreviewError::InvalidRequest)
        );
        let panicked = start_worker(&state, || -> Result<(), PreviewError> {
            panic!("synthetic worker failure")
        })
        .unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(finish_worker(panicked)),
            Err(PreviewError::WorkerFailed)
        );
        let body = InvokeBody::Raw(serde_json::to_vec(&fixture()).unwrap());
        assert!(tauri::async_runtime::block_on(execute(&body, &state)).is_ok());
    }

    #[test]
    fn every_error_serializes_as_content_free_fixed_code() {
        for (error, code) in [
            (PreviewError::InvalidBody, "PREVIEW_INVALID_BODY"),
            (PreviewError::RequestTooLarge, "PREVIEW_REQUEST_TOO_LARGE"),
            (PreviewError::InvalidRequest, "PREVIEW_INVALID_REQUEST"),
            (PreviewError::Busy, "PREVIEW_BUSY"),
            (PreviewError::CoreFailed, "PREVIEW_CORE_FAILED"),
            (PreviewError::WorkerFailed, "PREVIEW_WORKER_FAILED"),
            (PreviewError::ResponseTooLarge, "PREVIEW_RESPONSE_TOO_LARGE"),
        ] {
            assert_eq!(serde_json::to_value(error).unwrap(), code);
        }
    }
}
