// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Desktop adapter. The domain model stays inside the UI-independent Core.

mod preview;
mod probe;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .manage(preview::PreviewState::default())
        .invoke_handler(tauri::generate_handler![
            probe::core_probe,
            preview::candidate_preview
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use tauri::test::{MockRuntime, get_ipc_response, mock_builder};
    use tauri::{WebviewWindow, WebviewWindowBuilder, ipc, webview::InvokeRequest};

    fn invoke(
        window: &WebviewWindow<MockRuntime>,
        url: &str,
        command: &str,
        body: ipc::InvokeBody,
    ) -> Result<serde_json::Value, serde_json::Value> {
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
        .map(|body| body.deserialize().unwrap())
    }

    #[test]
    fn generated_acl_allows_main_but_denies_other_windows_and_remote_origins() {
        // The real generated context contains the application's build-time ACL;
        // the runtime is mocked, but neither the handler nor Core result is.
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![crate::probe::core_probe])
            .build(context)
            .unwrap();
        let main = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "other", Default::default())
            .build()
            .unwrap();
        let local_url = if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        assert_eq!(
            invoke(&main, local_url, "core_probe", ipc::InvokeBody::default()).unwrap(),
            serde_json::json!({"acceptedCaseEligible": true, "rejectedCaseEligible": false})
        );
        assert!(invoke(&other, local_url, "core_probe", ipc::InvokeBody::default()).is_err());
        assert!(
            invoke(
                &main,
                "https://example.invalid",
                "core_probe",
                ipc::InvokeBody::default()
            )
            .is_err()
        );
    }

    #[test]
    fn preview_generated_acl_scopes_actual_raw_command_to_local_main() {
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();
        let app = mock_builder()
            .manage(crate::preview::PreviewState::default())
            .invoke_handler(tauri::generate_handler![
                crate::probe::core_probe,
                crate::preview::candidate_preview
            ])
            .build(context)
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
        let payload = serde_json::json!({
            "requestId":"acl-preview-1", "seeds":["Synthetic1!"],
            "options":{"caseVariants":["asciiLowercase"],"prefixes":[""],"suffixes":[""]},
            "limits":{"attemptLimit":2048,"emissionLimit":50},
            "directConstraints":[
                {"editorId":41,"enabled":true,"applicability":"hard","confidence":"low",
                 "constraint":{"type":"prefix","literal":"synthetic"}},
                {"editorId":42,"enabled":true,"applicability":"soft","confidence":"high",
                 "constraint":{"type":"suffix","literal":"!"}}
            ]
        });
        let raw = || ipc::InvokeBody::Raw(serde_json::to_vec(&payload).unwrap());
        let result = invoke(&main, local, "candidate_preview", raw()).unwrap();
        assert_eq!(result["candidates"][0]["text"], "synthetic1!");
        assert_eq!(result["candidates"][0]["relativePoints"], 3);
        assert_eq!(
            result["candidates"][0]["matchedSoftHypothesisIds"],
            serde_json::json!([4])
        );
        assert_eq!(
            result["constraintBindings"],
            serde_json::json!([
                {"editorId":41,"hypothesisId":2}, {"editorId":42,"hypothesisId":4}
            ])
        );
        assert_eq!(result["stop"], "completed");
        assert!(invoke(&other, local, "candidate_preview", raw()).is_err());
        assert!(invoke(&main, "https://example.invalid", "candidate_preview", raw()).is_err());
        assert_eq!(
            invoke(
                &main,
                local,
                "candidate_preview",
                ipc::InvokeBody::Json(payload)
            )
            .unwrap_err(),
            "PREVIEW_INVALID_BODY"
        );
        assert_eq!(
            invoke(
                &main,
                local,
                "candidate_preview",
                ipc::InvokeBody::Raw(vec![b'x'; crate::preview::MAX_REQUEST_BYTES + 1])
            )
            .unwrap_err(),
            "PREVIEW_REQUEST_TOO_LARGE"
        );
        assert!(invoke(&main, local, "core_probe", ipc::InvokeBody::default()).is_ok());
    }
}
