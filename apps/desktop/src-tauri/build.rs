// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enrollment makes custom commands subject to capability checks, rather
    // than leaving invoke_handler commands callable from every window.
    let mut attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "core_probe",
            "candidate_preview",
            "project_open",
            "project_save",
        ]));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Tauri's resource compilation covers application binaries only.
        // Embed the same Common Controls v6 dependency through the linker so
        // library unit test executables can also load the native dialog API.
        // Omit the resource manifest to avoid embedding it twice in the app.
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or(
                "CARGO_MANIFEST_DIR is required to locate the Windows application manifest",
            )?)
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(attributes)?;
    Ok(())
}
