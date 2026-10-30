# Desktop development

The first Cluebench desktop shell uses Tauri 2 with a React/TypeScript SPA built
by Vite. Tauri connects the existing Rust Core directly to the webview in the
same native process. It avoids introducing a server or a second Core process and
keeps the domain implementation reusable outside the UI. This is an application
foundation, not the completed MVP. Recorded execution results and remaining
checks are in [Issue #12 verification](candidate-preview-verification.md);
[Issue #9 verification](desktop-verification.md) preserves the earlier shell record.

## Architecture and layout

```text
React / TypeScript -> Tauri IPC -> Desktop Rust adapter -> cluebench-core

apps/desktop/
  src/App.tsx                 Candidate Preview and auxiliary Core probe
  src/preview-input.ts        Seed grammar, input bounds, request construction
  src/preview-bridge.ts       Raw request encoding, response checks, safe errors
  src/core-bridge.ts          Auxiliary probe bridge and safe errors
  src-tauri/src/preview.rs    Bounded worker, DTOs, Core generation, single-flight
  src-tauri/src/probe.rs      Synthetic Core operation and small response DTO
  src-tauri/src/lib.rs        Command registration and application startup
  src-tauri/build.rs          Tauri build and custom-command ACL registration
  src-tauri/capabilities/     Main-window permission
  src-tauri/tauri.conf.json   Native window, assets, CSP, development hooks
  package.json               One frontend application and pinned pnpm version
  pnpm-lock.yaml             Frontend dependency resolution
crates/cluebench-core/       UI-independent domain implementation
Cargo.toml                  One Cargo workspace, edition 2024, resolver 3
Cargo.lock                  Shared Core and Desktop dependency resolution
rust-toolchain.toml          Shared local and CI compiler/tool versions
```

The Desktop crate is a workspace member. Its Core dependency is
`../../../crates/cluebench-core`, relative to `apps/desktop/src-tauri/Cargo.toml`.
The Core has no Tauri or frontend dependency; its API and domain semantics are
unchanged. The frontend contains no constraint matcher or candidate evaluator.
There is no root pnpm workspace or shared UI package. Run frontend commands from
the repository root with `pnpm -C apps/desktop`.

## Pinned tools and direct dependencies

Install Node.js **24.22.2**, recorded in `apps/desktop/.node-version`, and pnpm
**10.34.9**, recorded in `package.json`'s `packageManager` field. For an existing
Node installation, pnpm can be installed with:

```sh
npm install --global pnpm@10.34.9
node --version
pnpm --version
```

Install Rust through rustup. From the repository root:

```sh
rustup toolchain install --profile minimal --component rustfmt --component clippy
rustup show active-toolchain
rustc --version
```

`rust-toolchain.toml` selects **Rust 1.99.1**, rustfmt, and Clippy. This fixes the
compiler and lint version across local development and both CI platforms, so a
moving `stable` channel does not introduce CI-only lint changes. Edition 2024 and
resolver 3 require Rust 1.85 or newer; the selected Tauri and tauri-build releases
declare Rust 1.90. Cargo metadata reports 1.90 as the highest declared MSRV among
the locked registry packages. The pin is a reproducible development baseline,
not a promise to support every older compiler. Actual dependency compatibility
is established by the locked builds recorded in the verification report.

An explicit `cargo +...`, `RUSTUP_TOOLCHAIN`, or rustup directory override can
take precedence over the toolchain file. Check the active toolchain before
testing; remove an unintended `stable` override from your environment. CI does
not set `RUSTUP_TOOLCHAIN=stable`.

The following license identifiers were checked against installed package
metadata and downloaded Cargo manifests:

| Direct dependency | Version | Purpose | License |
| --- | --- | --- | --- |
| `tauri` | 2.12.5 | Native window, webview, IPC | MIT OR Apache-2.0 |
| `tauri-build` | 2.7.4 | Build-time config, assets, ACL | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | Preview request/response/error serialization | MIT OR Apache-2.0 |
| `serde_json` | 1.0.154 | Preview JSON parsing/serialization and test assertions | MIT OR Apache-2.0 |
| `@tauri-apps/api`, `@tauri-apps/cli` | 2.12.4 | Supported JS IPC API and native build CLI | MIT OR Apache-2.0 |
| `react`, `react-dom` | 19.3.1 | SPA rendering and local UI state | MIT |
| `typescript` | 7.0.2 | Static type checking | Apache-2.0 |
| `vite` | 8.3.5 | Development server and local production assets | MIT |
| `@vitejs/plugin-react` | 6.1.2 | React/Vite integration | MIT |
| `vitest` | 5.0.5 | Focused frontend tests | MIT |
| `jsdom` | 30.1.3 | Test-only browser environment | MIT |
| `@testing-library/react`, `@testing-library/dom` | 16.3.3 / 10.4.2 | UI state assertions | MIT |
| `@types/node` | 24.19.2 | Build/test configuration types | MIT |
| `@types/react`, `@types/react-dom` | 19.3.0 | React types | MIT |

The local `cluebench-core` path dependency and new Cluebench sources remain
MPL-2.0. pnpm itself is MIT-licensed build tooling. No third-party license is
replaced by the application license. Transitive dependencies and system
components retain their licenses. Notable transitive components include Wry
(MIT OR Apache-2.0), Tao (Apache-2.0), MPL-2.0 packages such as `cssparser` and
`selectors`, and ICU packages under Unicode-3.0. GTK/WebKitGTK include LGPL and
other upstream license/notice requirements; Microsoft's WebView2 has its own
terms. Installer/distribution license compliance needs a separate review when
distribution is implemented; this inventory is not a complete transitive
license audit.

## Linux and WSL2 prerequisites

For Ubuntu 24.04, install the native development packages from the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/):

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends -y \
  build-essential pkg-config libwebkit2gtk-4.1-dev libssl-dev \
  libxdo-dev libayatana-appindicator3-dev librsvg2-dev
pkg-config --modversion webkit2gtk-4.1 gtk+-3.0
```

Install `curl`, `wget`, and `file` as well if your base image lacks them. Use
WebKitGTK **4.1** development packages; older 4.0 packages do not provide the
selected Tauri 2 backend. The packages above also match the Linux CI setup.
System fonts must be available for the UI to render text.

WSL2 builds the **Linux** target. A working WSLg graphical session can display
that Linux application on the Windows desktop; this remains Linux execution.
Check that `DISPLAY` or `WAYLAND_DISPLAY` identifies a working session. A
successful compile in a terminal without a display does not verify GUI startup.
Keep the checkout and Linux build artifacts in the WSL Linux filesystem.

The recorded WSLg smoke test selected X11 with `GDK_BACKEND=x11`. On that host,
the development webview initially reported **Unspecified proxy lookup failure**
from the GIO proxy resolver. For that specific localhost development failure,
the following process-local environment adjustment worked:

```sh
GDK_BACKEND=x11 GIO_USE_PROXY_RESOLVER=dummy \
  pnpm -C apps/desktop tauri dev -- --locked
```

Do not make this a global proxy change. Production bundled assets did not need
the resolver adjustment. See the [execution record](desktop-verification.md)
for the failed attempt, successful retry and nonfatal WSLg graphics warnings.

## Windows native prerequisites

Use a separate native Windows checkout and native Windows Node/pnpm/Rust tools.
Do not reuse Linux `node_modules` or Cargo artifacts for Windows compilation.
Install:

- Microsoft C++ Build Tools, with **Desktop development with C++**, the MSVC
  compiler/linker, and a Windows SDK.
- Rustup with the **MSVC** host toolchain, normally `x86_64-pc-windows-msvc`, at
  the repository's Rust version.
- Microsoft Edge **WebView2 Runtime** for GUI execution.
- The pinned Node.js and pnpm versions above.

Follow the Windows section of the
[official prerequisites](https://v2.tauri.app/start/prerequisites/). Confirm the
MSVC workload and SDK in Visual Studio Installer and the active host with
`rustup show`. Running these commands from a suitable Visual Studio Developer
PowerShell is useful when checking the compiler and linker. A WebView2 runtime
alone does not supply native C++ build tools. Linux cross-compilation is not the
Windows verification path used by this project.

## Development and verification commands

Run these from the repository root after installing prerequisites:

```sh
pnpm -C apps/desktop install --frozen-lockfile
pnpm -C apps/desktop typecheck
pnpm -C apps/desktop test
pnpm -C apps/desktop build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test -p cluebench-core --locked
pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked
git diff --check
```

Build frontend assets before workspace Rust checks: the Desktop context embeds
`apps/desktop/dist` when using production assets. The Tauri CLI's
`beforeBuildCommand` also runs `pnpm build` automatically. `--no-bundle` skips
installer creation; the arguments after `--` are forwarded to Cargo. The root
`Cargo.lock` is authoritative for both workspace members. Frozen pnpm install
checks the app's lockfile without rewriting it.

Workspace checks include the native adapter and therefore need its native
packages and frontend preparation. `cargo test -p cluebench-core --locked`
compiles/tests only the UI-independent Core, without building the Desktop
adapter or requiring its frontend assets or GTK development libraries. Cargo
still uses the shared workspace lockfile.

Start the native development application with:

```sh
pnpm -C apps/desktop tauri dev -- --locked
```

Tauri runs `pnpm dev` and opens the `main` window. Vite binds to
`http://localhost:1420`; Tauri's `devUrl` matches it, and `strictPort` makes a
port conflict fail visibly. To inspect the frontend alone, run
`pnpm -C apps/desktop dev` and open that URL in a browser. Clicking the check
or Preview there reports an unavailable Desktop connection; it never fabricates
Core output.

## Candidate Preview and auxiliary probe

Candidate Preview is the primary screen. It accepts ordered manual seed lines,
explicit case transformations and prefix/suffix lists, and independent attempt
and emission limits. The [Preview contract](bounded-candidate-preview.md)
defines the exact input grammar, all resource limits, DTOs, one-shot Raw IPC,
blocking worker and single-flight ownership, request/revision handling, safe
errors, statistics/stop semantics, and privacy limitations. No interactive
cancellation or export is provided.

The auxiliary custom command, `core_probe`, takes no frontend input. It constructs a
fixed synthetic `RawProject` containing one enabled, manual-origin Hard
hypothesis with a Length constraint of 2 through 4. It calls `validate()`,
`resolve()`, and `evaluate()` on the synthetic cases `"abc"` and `"a"`.

Only the resulting `hard_eligible` booleans cross IPC:

```json
{"acceptedCaseEligible": true, "rejectedCaseEligible": false}
```

The candidate strings, hypotheses, constraints, seeds, and full domain model
remain inside Rust. Validation/evaluation failures become the fixed codes
`CORE_VALIDATION_FAILED` and `CORE_EVALUATION_FAILED`. The command has no
production `unwrap()`/`expect()` path or domain debug logging.

`src/core-bridge.ts` owns the supported Tauri 2 `invoke` call, exact command name,
response type, and runtime shape validation. It accepts exactly two boolean
fields, translates recognized codes to fixed messages, and replaces arbitrary
IPC rejection content with a safe fallback. A TypeScript generic is not treated
as runtime validation.

The auxiliary probe displays both booleans with independent connection states.
The primary Preview distinguishes Idle, Invalid, Loading, Success, Empty,
Limited, and Error. Empty/Limited may coexist; Core termination is shown
separately. Editing or Clear invalidates results while the outstanding request
continues to hold Loading until its Promise settles. The footer identifies
remaining editors, persistence, and export as unavailable.

## Command ACL, CSP, and privacy

Registering an `invoke_handler` alone does not limit custom commands by window.
`build.rs` explicitly enrolls `core_probe` and `candidate_preview` with
`tauri_build::AppManifest::commands`, generating their allow/deny permissions during the build. `capabilities/main-probe.json` grants only
`allow-core-probe` and `allow-candidate-preview`, only to the exact window label `main`, and only for local
application content. There is no remote-origin capability, wildcard window, or
`core:default` permission. The config selects that one capability explicitly.
Generated schemas and permissions are build artifacts, not hand-maintained
source. See [Tauri capabilities](https://v2.tauri.app/security/capabilities/).

The production CSP starts with `default-src 'none'`. It allows same-origin
packaged scripts, styles, and images, and the Tauri IPC endpoints `ipc:` and
`http://ipc.localhost`. The latter is Tauri's IPC transport, not an application
HTTP server. Frames, objects, form submissions, and base-URL overrides are
blocked. Tauri's asset CSP modification remains enabled. There is no
`unsafe-eval`, production `unsafe-inline`, external font, CDN, or network origin.

`devCsp` separately permits Vite's localhost HTTP/WebSocket connection and inline
development scripts/styles needed by React refresh and Vite CSS injection.
Those allowances do not appear in the production CSP. The production window
loads bundled `index.html` from `frontendDist`; it does not navigate to remote
application content. See [Tauri CSP](https://v2.tauri.app/security/csp/).

No Tauri plugins, filesystem/shell/network commands, telemetry, analytics,
authentication, updater, or secret-bearing logs are added. Tests use only
synthetic fixtures. Preview input/results exist in memory and local IPC only;
Clear does not promise secure erasure. Raw IPC limits application DTO parsing,
not prior WebView/transport allocations or a compromised renderer. Rust tests exercise the generated ACL with Tauri's mock runtime:
the real handler/Core result is allowed for local `main`, denied for another
window, and denied for an external origin. That test is distinct from a native
webview smoke test.

## Native GUI smoke test

Perform this in both a working Linux/WSLg session and a native Windows session
when those environments are available:

1. Run `pnpm -C apps/desktop tauri dev -- --locked`. Confirm the native
   **Cluebench** window renders Candidate Preview. Use the process-local proxy
   workaround above only if the documented WSLg lookup failure occurs.
2. Enter synthetic seeds `aB` and `12` on separate lines. Select Original,
   ASCII Lowercase and ASCII Uppercase. Add ordered prefixes `""`, `PRE-` and
   suffixes `""`, `-X`, using the explicit no-affix choices for empty strings.
   Run Preview and confirm 24 records in Seed/Case/Prefix/Suffix order, including
   `aB`, `aB-X`, `PRE-aB`, `PRE-aB-X` first. Inspect seed IDs, paths and affixes.
3. Confirm attempted 24, skipped/rejected 0, emitted 24, relative points 0,
   and Completed. Run again. Edit an input and confirm old results disappear.
   Enter an internal empty seed line and confirm a fixed line-number error.
   Restore input and verify retry. Set emission 1 to inspect OutputLimit;
   use a 64-byte seed plus a nonempty prefix to inspect domain skips.
4. Clear while a controlled request is pending when testing lifecycle behavior:
   Clear invalidates display, but is not cancellation. The normal bounded run
   may finish too quickly to observe Loading; focused frontend tests deliberately
   hold Promises pending for these transitions.
5. Build with `pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked`.
   Stop Vite, launch `./target/release/cluebench-desktop` on Linux or
   `.\target\release\cluebench-desktop.exe` on Windows, and repeat the checks.
   This verifies bundled assets, production CSP and real Raw IPC separately.
6. Run `pnpm -C apps/desktop dev` alone in a browser and submit valid input.
   Confirm Desktop unavailable, safe retry, and no fabricated candidates.
7. Optionally run the auxiliary Core connection probe. Close the native window
   normally and confirm clean process exit.

Controlled frontend tests cover rejected invocations and recovery without adding
a privileged debug command to the product. Do not report a browser render,
mock-runtime ACL test, or compile as a native GUI/IPC success. Record frontend
build, Core tests, Linux build, Windows build, GUI startup, real IPC, and GitHub
Actions separately in the verification report.

## CI and remaining scope

`.github/workflows/rust.yml` has a native `ubuntu-24.04` / `windows-2025` matrix.
Both jobs install the pinned tools and frontend dependencies and run the native
Tauri build without installers. Linux additionally checks formatting,
TypeScript, frontend tests, production assets, Clippy with `-D warnings`, the
whole Cargo workspace, and the Core alone. Frontend assets are built before
Rust checks that embed them. The Windows job runs on an actual Windows native
runner. See its [documented image tools](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md).

Actions are pinned to commit SHAs, permissions are `contents: read`, checkout
does not persist credentials, and no release permissions or secrets are needed.
The 45-minute timeout accommodates cold native builds as an initial budget; it
is not a measured runtime guarantee. CI runs after push and does not currently
automate a native GUI smoke test. Local workflow editing does not establish a
GitHub Actions PASS.

The Desktop does not implement Clue/Hypothesis/Constraint editors, alternative
selection, structure canvas, export, persistence/migration/encryption, general
generation jobs, streaming, interactive cancellation, routing/global state/i18n
frameworks, final branding, installers, signing, updates, release automation,
macOS/mobile readiness, CLI/HTTP APIs, stable external Rust APIs, or runtime AI.
Preview request/revision handling and bounded worker ownership are implemented
locally without introducing those broader frameworks. Issue #11's glib
vulnerability investigation remains separate and unresolved by this change.
