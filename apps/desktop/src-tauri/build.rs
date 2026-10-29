// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enrollment makes custom commands subject to capability checks, rather
    // than leaving invoke_handler commands callable from every window.
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(&["core_probe"]));
    tauri_build::try_build(attributes)?;
    Ok(())
}
