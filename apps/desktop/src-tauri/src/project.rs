// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Local-only project commands. Paths never cross IPC; only the current and
//! one prepared binding exist. A prepared binding needs explicit adoption.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use age::secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use tauri::{
    AppHandle, Runtime, State,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    project_crypto,
    project_document::{self, ProjectDocumentV1, ProjectError},
    project_fs::{FileBinding, FsError, WriteOutcome},
};

// Two escaped 1,024-byte phrases plus the bounded authoring JSON and envelope.
const MAX_COMMAND_BYTES: usize = project_document::MAX_JSON_BYTES + 16 * 1024;
type CommandResult = Result<ProjectReply, &'static str>;

#[derive(Default, Clone)]
pub(crate) struct ProjectState {
    busy: Arc<AtomicBool>,
    registry: Arc<Mutex<Registry>>,
}

#[derive(Default)]
struct Registry {
    current: Option<Binding>,
    pending: Option<Binding>,
    // During adoption the old current binding occupies pending until the UI
    // acknowledges replacement. No third filesystem binding is retained.
    tentative: Option<String>,
}

struct Binding {
    token: String,
    file: FileBinding,
    mode: BindingMode,
    adoptable: bool,
    uncertain: bool,
}

#[derive(PartialEq, Eq)]
enum BindingMode {
    Existing,
    New,
}

struct Lease(Arc<AtomicBool>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl ProjectState {
    fn acquire(&self) -> Result<Lease, &'static str> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "PROJECT_BUSY")?;
        Ok(Lease(Arc::clone(&self.busy)))
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum ProjectReply {
    Cancelled,
    Selected {
        token: String,
    },
    Opened {
        token: String,
        document: Box<ProjectDocumentV1>,
    },
    Adopted,
    Finalized,
    Discarded,
    NotCommitted {
        code: &'static str,
    },
    Saved,
    CommittedDurabilityUncertain,
    OutcomeUnknown,
}

struct SecretInput(SecretString);
impl<'de> Deserialize<'de> for SecretInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self(SecretString::from(value)))
    }
}

#[derive(Default)]
enum Confirmation {
    #[default]
    Missing,
    Present(Option<SecretInput>),
}
impl<'de> Deserialize<'de> for Confirmation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<SecretInput>::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope<'a> {
    action: String,
    token: Option<String>,
    passphrase: Option<SecretInput>,
    #[serde(default)]
    confirmation: Confirmation,
    #[serde(borrow)]
    document: Option<&'a RawValue>,
}

fn payload(request: Request<'_>) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("PROJECT_INVALID_BODY");
    };
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err("PROJECT_BUDGET_EXCEEDED");
    }
    Ok(Zeroizing::new(bytes.clone()))
}

fn envelope(bytes: &[u8]) -> Result<Envelope<'_>, &'static str> {
    // Derived struct visitors also accept sequences; command records are maps.
    if bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return Err("PROJECT_INVALID_REQUEST");
    }
    serde_json::from_slice(bytes).map_err(|_| "PROJECT_INVALID_REQUEST")
}

fn error_code(error: ProjectError) -> &'static str {
    error.code()
}

fn registry(state: &ProjectState) -> Result<std::sync::MutexGuard<'_, Registry>, &'static str> {
    state.registry.lock().map_err(|_| "PROJECT_UNAVAILABLE")
}

fn binding<'a>(registry: &'a mut Registry, token: &str) -> Result<&'a mut Binding, &'static str> {
    if registry
        .current
        .as_ref()
        .is_some_and(|value| value.token == token)
    {
        return registry.current.as_mut().ok_or("PROJECT_INVALID_TOKEN");
    }
    registry
        .pending
        .as_mut()
        .filter(|value| value.token == token)
        .ok_or("PROJECT_INVALID_TOKEN")
}

fn select<R: Runtime>(app: &AppHandle<R>, state: &ProjectState, save: bool) -> CommandResult {
    // Invalidate an abandoned preparation before showing another dialog.
    {
        let mut registry = registry(state)?;
        if registry.tentative.is_some() {
            return Err("PROJECT_BUSY");
        }
        registry.pending = None;
    }
    let dialog = app
        .dialog()
        .file()
        .add_filter("Encrypted Cluebench project", &["cluebench", "age"]);
    let selected = if save {
        dialog
            .set_title("Save encrypted project to a new file")
            .blocking_save_file()
    } else {
        dialog
            .set_title("Open encrypted Cluebench project")
            .blocking_pick_file()
    };
    let Some(selected) = selected else {
        return Ok(ProjectReply::Cancelled);
    };
    let path = selected
        .into_path()
        .map_err(|_| "PROJECT_UNSAFE_DESTINATION")?;
    let file = if save {
        FileBinding::new(&path).map_err(fs_code)?
    } else {
        FileBinding::open(&path).map_err(fs_code)?.0
    };
    let token = Uuid::new_v4().to_string();
    registry(state)?.pending = Some(Binding {
        token: token.clone(),
        file,
        mode: if save {
            BindingMode::New
        } else {
            BindingMode::Existing
        },
        adoptable: false,
        uncertain: false,
    });
    Ok(ProjectReply::Selected { token })
}

fn open_action<R: Runtime>(
    app: &AppHandle<R>,
    state: &ProjectState,
    bytes: &[u8],
) -> CommandResult {
    let request = envelope(bytes)?;
    if request.document.is_some() || !matches!(request.confirmation, Confirmation::Missing) {
        return Err("PROJECT_INVALID_REQUEST");
    }
    match request.action.as_str() {
        "select" if request.token.is_none() && request.passphrase.is_none() => {
            select(app, state, false)
        }
        "read" => {
            let token = request.token.ok_or("PROJECT_INVALID_REQUEST")?;
            let phrase = request.passphrase.ok_or("PROJECT_INVALID_REQUEST")?.0;
            let mut registry = registry(state)?;
            if registry.tentative.is_some() {
                return Err("PROJECT_BUSY");
            }
            let prepared = registry
                .pending
                .as_mut()
                .filter(|value| {
                    value.token == token && value.mode == BindingMode::Existing && !value.uncertain
                })
                .ok_or("PROJECT_INVALID_TOKEN")?;
            prepared.adoptable = false;
            let encrypted = prepared.file.read_unchanged().map_err(fs_code)?;
            let plaintext = project_crypto::decrypt(&encrypted, phrase).map_err(error_code)?;
            let document = project_document::parse_document(&plaintext).map_err(error_code)?;
            prepared.adoptable = true;
            Ok(ProjectReply::Opened {
                token,
                document: Box::new(document),
            })
        }
        "adopt" if request.passphrase.is_none() => {
            let token = request.token.ok_or("PROJECT_INVALID_REQUEST")?;
            let mut registry = registry(state)?;
            if registry
                .current
                .as_ref()
                .is_some_and(|value| value.token == token && !value.uncertain)
            {
                return Ok(ProjectReply::Adopted);
            }
            if registry.tentative.is_some() {
                return Err("PROJECT_BUSY");
            }
            if !registry
                .pending
                .as_ref()
                .is_some_and(|value| value.token == token && value.adoptable && !value.uncertain)
            {
                return Err("PROJECT_INVALID_TOKEN");
            }
            let prepared = registry.pending.take();
            registry.pending = registry.current.take();
            registry.current = prepared;
            registry.tentative = Some(token);
            Ok(ProjectReply::Adopted)
        }
        "finalize" if request.passphrase.is_none() => {
            let token = request.token.ok_or("PROJECT_INVALID_REQUEST")?;
            let mut registry = registry(state)?;
            if !registry
                .current
                .as_ref()
                .is_some_and(|value| value.token == token && !value.uncertain)
            {
                return Err("PROJECT_INVALID_TOKEN");
            }
            if registry
                .tentative
                .as_ref()
                .is_some_and(|value| value == &token)
            {
                registry.pending = None;
                registry.tentative = None;
            }
            Ok(ProjectReply::Finalized)
        }
        "discard" if request.passphrase.is_none() => {
            let token = request.token.ok_or("PROJECT_INVALID_REQUEST")?;
            let mut registry = registry(state)?;
            if registry
                .tentative
                .as_ref()
                .is_some_and(|value| value == &token)
            {
                registry.current = registry.pending.take();
                registry.tentative = None;
                return Ok(ProjectReply::Discarded);
            }
            if registry.tentative.is_some() {
                return Err("PROJECT_BUSY");
            }
            if registry
                .pending
                .as_ref()
                .is_some_and(|value| value.token == token)
            {
                registry.pending = None;
            }
            Ok(ProjectReply::Discarded)
        }
        _ => Err("PROJECT_INVALID_REQUEST"),
    }
}

fn save_action<R: Runtime>(
    app: &AppHandle<R>,
    state: &ProjectState,
    bytes: &[u8],
) -> CommandResult {
    let request = envelope(bytes)?;
    match request.action.as_str() {
        "select"
            if request.token.is_none()
                && request.passphrase.is_none()
                && request.document.is_none()
                && matches!(request.confirmation, Confirmation::Missing) =>
        {
            select(app, state, true)
        }
        "write" => {
            let token = request.token.ok_or("PROJECT_INVALID_REQUEST")?;
            let phrase = request.passphrase.ok_or("PROJECT_INVALID_REQUEST")?.0;
            project_crypto::validate_passphrase(phrase.expose_secret()).map_err(error_code)?;
            let raw_document = request.document.ok_or("PROJECT_INVALID_REQUEST")?;
            let document = project_document::parse_document(raw_document.get().as_bytes())
                .map_err(error_code)?;
            let plaintext = project_document::serialize_document(&document).map_err(error_code)?;
            let mut registry = registry(state)?;
            if registry.tentative.is_some() {
                return Err("PROJECT_BUSY");
            }
            // An Open preparation is read-only until the frontend successfully
            // stages it and explicitly adopts it as the current project.
            if registry
                .pending
                .as_ref()
                .is_some_and(|value| value.token == token && value.mode != BindingMode::New)
            {
                return Err("PROJECT_INVALID_TOKEN");
            }
            let target = binding(&mut registry, &token)?;
            if target.uncertain {
                return Err("PROJECT_OUTCOME_UNKNOWN");
            }
            match (&target.mode, request.confirmation) {
                (BindingMode::New, Confirmation::Present(Some(confirmation))) => {
                    project_crypto::validate_passphrase(confirmation.0.expose_secret())
                        .map_err(error_code)?;
                    if confirmation.0.expose_secret() != phrase.expose_secret() {
                        return Err("PROJECT_PASSPHRASE_MISMATCH");
                    }
                }
                (BindingMode::Existing, Confirmation::Present(None)) => {
                    let ciphertext = target.file.read_unchanged().map_err(fs_code)?;
                    // Authentication finishes before the encryption KDF begins.
                    let authenticated =
                        project_crypto::decrypt(&ciphertext, phrase.clone()).map_err(error_code)?;
                    drop(authenticated);
                }
                _ => return Err("PROJECT_INVALID_REQUEST"),
            }
            let ciphertext = project_crypto::encrypt(&plaintext, phrase).map_err(error_code)?;
            match target.file.write(&ciphertext) {
                WriteOutcome::NotCommitted(error) => Ok(ProjectReply::NotCommitted {
                    code: fs_code(error),
                }),
                WriteOutcome::Saved => {
                    target.mode = BindingMode::Existing;
                    target.adoptable = true;
                    Ok(ProjectReply::Saved)
                }
                WriteOutcome::CommittedDurabilityUncertain => {
                    target.mode = BindingMode::Existing;
                    target.adoptable = true;
                    Ok(ProjectReply::CommittedDurabilityUncertain)
                }
                WriteOutcome::OutcomeUnknown => {
                    target.uncertain = true;
                    target.adoptable = false;
                    Ok(ProjectReply::OutcomeUnknown)
                }
            }
        }
        _ => Err("PROJECT_INVALID_REQUEST"),
    }
}

#[tauri::command]
pub(crate) async fn project_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ProjectState>,
    request: Request<'_>,
) -> CommandResult {
    let lease = state.acquire()?;
    let bytes = payload(request)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        open_action(&app, &state, &bytes)
    })
    .await
    .map_err(|_| "PROJECT_WORKER_FAILED")?
}

#[tauri::command]
pub(crate) async fn project_save<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ProjectState>,
    request: Request<'_>,
) -> CommandResult {
    let lease = state.acquire()?;
    let bytes = payload(request)?;
    let state = state.inner().clone();
    // A failed worker could have crossed the commit point. Never label it a
    // precommit failure, and never automatically retry an unknown write.
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        save_action(&app, &state, &bytes)
    })
    .await
    .unwrap_or(Ok(ProjectReply::OutcomeUnknown))
}

fn fs_code(error: FsError) -> &'static str {
    error.code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tauri::{
        WebviewWindowBuilder, ipc,
        test::{MockRuntime, get_ipc_response, mock_builder},
        webview::InvokeRequest,
    };

    fn document() -> ProjectDocumentV1 {
        project_document::parse_document(br#"{
          "format":"cluebench-project","schemaVersion":1,
          "clues":[],"hypotheses":[],"groups":[],"directConstraints":[],
          "manualSeeds":{"text":"Synthetic\n\t\n"},
          "options":{"caseVariants":[],"prefixes":{"includeEmpty":true,"rows":[]},"suffixes":{"includeEmpty":false,"rows":[]}},
          "limits":{"attemptLimit":"0002","emissionLimit":"-"}
        }"#).unwrap()
    }

    fn context() -> tauri::Context<MockRuntime> {
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();
        context
    }

    fn prepare(state: &ProjectState, path: &Path, new: bool) -> String {
        let file = if new {
            FileBinding::new(path).unwrap()
        } else {
            FileBinding::open(path).unwrap().0
        };
        let token = Uuid::new_v4().to_string();
        registry(state).unwrap().pending = Some(Binding {
            token: token.clone(),
            file,
            mode: if new {
                BindingMode::New
            } else {
                BindingMode::Existing
            },
            adoptable: false,
            uncertain: false,
        });
        token
    }

    fn transition<R: Runtime>(
        app: &AppHandle<R>,
        state: &ProjectState,
        action: &str,
        token: &str,
    ) -> CommandResult {
        open_action(
            app,
            state,
            &serde_json::to_vec(&serde_json::json!({"action":action,"token":token})).unwrap(),
        )
    }

    #[test]
    fn tentative_adoption_can_roll_back_a_lost_reply_without_eviction_or_extra_bindings() {
        let directory = tempfile::tempdir().unwrap();
        let state = ProjectState::default();
        let app = mock_builder().build(context()).unwrap();
        let old = prepare(&state, &directory.path().join("synthetic-old.age"), true);
        registry(&state)
            .unwrap()
            .pending
            .as_mut()
            .unwrap()
            .adoptable = true;
        transition(app.handle(), &state, "adopt", &old).unwrap();
        assert!(matches!(
            transition(app.handle(), &state, "finalize", &old).unwrap(),
            ProjectReply::Finalized
        ));
        let new = prepare(&state, &directory.path().join("synthetic-new.age"), true);
        registry(&state)
            .unwrap()
            .pending
            .as_mut()
            .unwrap()
            .adoptable = true;
        transition(app.handle(), &state, "adopt", &new).unwrap();
        transition(app.handle(), &state, "adopt", &new).unwrap();
        {
            let registry = registry(&state).unwrap();
            assert_eq!(registry.current.as_ref().unwrap().token, new);
            assert_eq!(registry.pending.as_ref().unwrap().token, old);
            assert_eq!(registry.tentative.as_deref(), Some(new.as_str()));
        }
        assert_eq!(
            open_action(app.handle(), &state, br#"{"action":"select"}"#).err(),
            Some("PROJECT_BUSY")
        );
        assert_eq!(
            save_action(app.handle(), &state, br#"{"action":"select"}"#).err(),
            Some("PROJECT_BUSY")
        );
        assert_eq!(
            open_action(
                app.handle(),
                &state,
                &serde_json::to_vec(
                    &serde_json::json!({"action":"read","token":new,"passphrase":"Synthetic only"})
                )
                .unwrap()
            )
            .err(),
            Some("PROJECT_BUSY")
        );
        let write = serde_json::json!({"action":"write","token":new,"passphrase":"Synthetic only","confirmation":"Synthetic only","document":document()});
        assert_eq!(
            save_action(app.handle(), &state, &serde_json::to_vec(&write).unwrap()).err(),
            Some("PROJECT_BUSY")
        );
        assert_eq!(
            transition(app.handle(), &state, "adopt", &old).err(),
            Some("PROJECT_BUSY")
        );
        assert_eq!(
            transition(app.handle(), &state, "discard", &old).err(),
            Some("PROJECT_BUSY")
        );
        assert_eq!(
            transition(app.handle(), &state, "finalize", &old).err(),
            Some("PROJECT_INVALID_TOKEN")
        );
        transition(app.handle(), &state, "discard", &new).unwrap();
        transition(app.handle(), &state, "discard", &new).unwrap();
        assert_eq!(
            registry(&state).unwrap().current.as_ref().unwrap().token,
            old
        );
        assert!(registry(&state).unwrap().pending.is_none());
        assert!(registry(&state).unwrap().tentative.is_none());
        assert_eq!(
            transition(app.handle(), &state, "adopt", &new).err(),
            Some("PROJECT_INVALID_TOKEN")
        );
        transition(app.handle(), &state, "finalize", &old).unwrap();
        let final_token = prepare(&state, &directory.path().join("synthetic-final.age"), true);
        registry(&state)
            .unwrap()
            .pending
            .as_mut()
            .unwrap()
            .adoptable = true;
        transition(app.handle(), &state, "adopt", &final_token).unwrap();
        transition(app.handle(), &state, "finalize", &final_token).unwrap();
        transition(app.handle(), &state, "finalize", &final_token).unwrap();
        transition(app.handle(), &state, "discard", &final_token).unwrap();
        assert_eq!(
            registry(&state).unwrap().current.as_ref().unwrap().token,
            final_token
        );
        assert!(registry(&state).unwrap().pending.is_none());
        assert!(registry(&state).unwrap().tentative.is_none());
    }

    #[test]
    fn raw_envelope_is_strict_and_errors_are_fixed() {
        for bytes in [
            b"[]".as_slice(),
            br#"{"action":"read","action":"adopt"}"#,
            br#"{"action":"select","path":"synthetic-private-path"}"#,
            br#"{"action":"read","passphrase":"\ud800"}"#,
        ] {
            assert_eq!(envelope(bytes).err(), Some("PROJECT_INVALID_REQUEST"));
        }
        let state = ProjectState::default();
        let _lease = state.acquire().unwrap();
        assert_eq!(state.acquire().err(), Some("PROJECT_BUSY"));
        assert_eq!(FsError::Io.code(), "PROJECT_IO");
    }

    #[test]
    fn actual_filesystem_and_crypto_save_open_save_with_transactional_adoption() {
        let _crypto = project_crypto::crypto_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("synthetic.cluebench");
        let state = ProjectState::default();
        let app = mock_builder().build(context()).unwrap();
        let token = prepare(&state, &path, true);
        let snapshot = document();
        let mut write = serde_json::json!({"action":"write","token":token,"passphrase":"Synthetic phrase only", "confirmation":"Synthetic phrase only","document":snapshot});
        write["confirmation"] = serde_json::json!("Mismatch");
        assert_eq!(
            save_action(app.handle(), &state, &serde_json::to_vec(&write).unwrap()).err(),
            Some("PROJECT_PASSPHRASE_MISMATCH")
        );
        assert!(!path.exists());
        write["confirmation"] = serde_json::json!("Synthetic phrase only");
        assert!(matches!(
            save_action(app.handle(), &state, &serde_json::to_vec(&write).unwrap()).unwrap(),
            ProjectReply::Saved | ProjectReply::CommittedDurabilityUncertain
        ));
        assert!(registry(&state).unwrap().current.is_none());
        let adopt =
            serde_json::to_vec(&serde_json::json!({"action":"adopt","token":token})).unwrap();
        assert!(matches!(
            open_action(app.handle(), &state, &adopt).unwrap(),
            ProjectReply::Adopted
        ));
        transition(app.handle(), &state, "finalize", &token).unwrap();
        let before = std::fs::read(&path).unwrap();
        write["confirmation"] = serde_json::Value::Null;
        write["passphrase"] = serde_json::json!("Wrong synthetic phrase");
        assert_eq!(
            save_action(app.handle(), &state, &serde_json::to_vec(&write).unwrap()).err(),
            Some("PROJECT_AUTHENTICATION_FAILED")
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let opened_token = prepare(&state, &path, false);
        let read = serde_json::to_vec(&serde_json::json!({"action":"read","token":opened_token,"passphrase":"Synthetic phrase only"})).unwrap();
        let ProjectReply::Opened {
            document: opened, ..
        } = open_action(app.handle(), &state, &read).unwrap()
        else {
            panic!("expected opened");
        };
        assert!(*opened == snapshot);
        assert_eq!(
            registry(&state).unwrap().current.as_ref().unwrap().token,
            token
        );
        let adopt_open =
            serde_json::to_vec(&serde_json::json!({"action":"adopt","token":opened_token}))
                .unwrap();
        open_action(app.handle(), &state, &adopt_open).unwrap();
        transition(app.handle(), &state, "finalize", &opened_token).unwrap();
        write["token"] = serde_json::json!(opened_token);
        write["passphrase"] = serde_json::json!("Synthetic phrase only");
        assert!(matches!(
            save_action(app.handle(), &state, &serde_json::to_vec(&write).unwrap()).unwrap(),
            ProjectReply::Saved | ProjectReply::CommittedDurabilityUncertain
        ));
        let encrypted = std::fs::read(&path).unwrap();
        let plaintext = project_crypto::decrypt(
            &encrypted,
            SecretString::from("Synthetic phrase only".to_owned()),
        )
        .unwrap();
        assert!(project_document::parse_document(&plaintext).unwrap() == snapshot);
        assert!(
            directory
                .path()
                .read_dir()
                .unwrap()
                .all(|item| item.unwrap().path() == path)
        );
        // Discard invalidates a pending token permanently; it cannot later be
        // adopted, and the current file association remains intact.
        let abandoned = prepare(&state, &path, false);
        open_action(
            app.handle(),
            &state,
            &serde_json::to_vec(&serde_json::json!({"action":"discard","token":abandoned}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            open_action(
                app.handle(),
                &state,
                &serde_json::to_vec(&serde_json::json!({"action":"adopt","token":abandoned}))
                    .unwrap()
            )
            .err(),
            Some("PROJECT_INVALID_TOKEN")
        );
        assert_eq!(
            registry(&state).unwrap().current.as_ref().unwrap().token,
            opened_token
        );
    }

    #[test]
    fn project_commands_use_generated_acl_and_raw_body_boundaries() {
        let app = mock_builder()
            .manage(ProjectState::default())
            .invoke_handler(tauri::generate_handler![project_open, project_save])
            .build(context())
            .unwrap();
        let main = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "other", Default::default())
            .build()
            .unwrap();
        let local = if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        for command in ["project_open", "project_save"] {
            let invoke = |window: &tauri::WebviewWindow<MockRuntime>, url: &str, body| {
                get_ipc_response(
                    window,
                    InvokeRequest {
                        cmd: command.into(),
                        callback: ipc::CallbackFn(0),
                        error: ipc::CallbackFn(1),
                        url: url.parse().unwrap(),
                        body,
                        headers: Default::default(),
                        invoke_key: tauri::test::INVOKE_KEY.into(),
                    },
                )
                .map(|response| response.deserialize::<serde_json::Value>().unwrap())
            };
            let bytes = || {
                ipc::InvokeBody::Raw(
                    br#"{"action":"read","token":"invalid","passphrase":"synthetic"}"#.to_vec(),
                )
            };
            let local_error = invoke(&main, local, bytes()).unwrap_err();
            assert_eq!(
                local_error,
                serde_json::json!(if command == "project_open" {
                    "PROJECT_INVALID_TOKEN"
                } else {
                    "PROJECT_INVALID_REQUEST"
                })
            );
            assert!(invoke(&other, local, bytes()).is_err());
            assert!(invoke(&main, "https://example.invalid", bytes()).is_err());
            assert_eq!(
                invoke(&main, local, ipc::InvokeBody::default()).unwrap_err(),
                "PROJECT_INVALID_BODY"
            );
            assert_eq!(
                invoke(
                    &main,
                    local,
                    ipc::InvokeBody::Raw(vec![0; MAX_COMMAND_BYTES + 1])
                )
                .unwrap_err(),
                "PROJECT_BUDGET_EXCEEDED"
            );
        }
    }
}
