// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Desktop adapter. The domain model stays inside the UI-independent Core.

mod probe;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![probe::core_probe])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use tauri::test::{MockRuntime, get_ipc_response, mock_builder};
    use tauri::{WebviewWindow, WebviewWindowBuilder, ipc, webview::InvokeRequest};

    fn invoke(
        window: &WebviewWindow<MockRuntime>,
        url: &str,
    ) -> Result<serde_json::Value, serde_json::Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: "core_probe".into(),
                callback: ipc::CallbackFn(0),
                error: ipc::CallbackFn(1),
                url: url.parse().unwrap(),
                body: ipc::InvokeBody::default(),
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
            invoke(&main, local_url).unwrap(),
            serde_json::json!({"acceptedCaseEligible": true, "rejectedCaseEligible": false})
        );
        assert!(invoke(&other, local_url).is_err());
        assert!(invoke(&main, "https://example.invalid").is_err());
    }
}
