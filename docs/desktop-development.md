# Desktop development

The first Cluebench desktop shell uses Tauri 2 with a React/TypeScript SPA built
by Vite. Tauri connects the existing Rust Core directly to the webview in the
same native process. It avoids introducing a server or a second Core process and
keeps the domain implementation reusable outside the UI. This is an application
foundation, not the completed MVP. [Issue #12 verification](candidate-preview-verification.md)
and [Issue #9 verification](desktop-verification.md) preserve historical local
evidence, not current CI status. Change-specific verification belongs in the
implementation report and PR; inspect the target commit's GitHub Actions jobs
for CI outcomes.

## Architecture and layout

```text
React / TypeScript -> Tauri IPC -> Desktop Rust adapter -> cluebench-core

apps/desktop/
  src/App.tsx                 Candidate Preview and auxiliary Core probe
  src/CandidatePreview.tsx    Input/revision ownership and request snapshots
  src/direct-constraints.ts  Typed constraint drafts, validation, DTO conversion
  src/DirectConstraintEditor.tsx  Direct rows and shared constraint fields
  src/clue-hypotheses.ts      Clue/Hypothesis/effect drafts and validation
  src/ClueHypothesisEditor.tsx  Original evidence and interpretation editing
  src/use-raw-text-guard.ts   Raw-text/IME guards and accepted-value restoration
  src/unicode-text.ts         Unicode scalar and UTF-8 evidence validation
  src/preview-input.ts        Shared input bounds and request construction
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
is established by the target commit's locked builds, not historical reports.

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

For a reproducible synthetic release measurement of the bounded Rust adapter:

```sh
cargo test -p cluebench-desktop --release --locked \
  release_native_32_effect_4096_attempt_measurement -- --ignored --nocapture
```

This ignored test uses 64 synthetic 50-byte Seeds, eight prefixes and eight
suffixes of lengths 0–7, and 32 Hard Contains effects whose 32-byte literals
force repeated comparisons. It exercises 4,096 attempted paths and reports
elapsed adapter time, including actual request deserialization, Core project
construction/validation/resolution/generation, and response serialization. It
does not include WebView IPC. Record the input/effect/attempt counts, elapsed
time, toolchain, OS/CPU, and whether real IPC was included in the implementation
report. One local measurement is not a latency guarantee for other machines.

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
and emission limits. Up to 16 Direct Constraints preserve the independent
manual-origin workflow. Up to 8 Clues store separate original text and source,
and up to 8 Clue-backed Hypotheses reference them. Several Hypotheses can share
one Clue. Each has an interpretation, enabled state, Hard/Soft applicability,
confidence, and 0–4 effects, with at most 16 Clue-backed effects in total.
The editor reuses the seven existing constraint fields: Length, Position,
Contains, NotContains, Prefix, Suffix, and CharacterCount.

Every enabled Hard effect must match. An enabled Soft Hypothesis with effects
contributes Low/Medium/High relative points of 1/2/3 once only when all its
effects match. Zero effects neither filter nor score; removing the final effect
retains its Hypothesis. Disabled records remain subject to full validation.
Points are not probabilities and do not change generation order. TypeScript
validates authoring and response consistency without matching candidate text.
A Clue with any referring Hypothesis, including disabled, incomplete, or
zero-effect drafts, cannot be deleted until those references are removed or
explicitly reassigned. No automatic deletion or origin replacement occurs.

Evidence text areas preserve accepted Unicode scalar text, LF, TAB, empty
strings, surrounding whitespace, combining characters, and Emoji within byte
limits, without trimming, normalization, or case conversion. The raw guard
separates this contract from printable-ASCII constraint input. It rejects
observable CR/CRLF insertions before textarea normalization, validates lone
surrogates before UTF-8 encoding, and keeps the previous accepted field value
on individual byte overflow. IME intermediate values block Preview until the
completed value is validated. Noncancelable input may transiently change the
DOM before restoration. Browser-normalized source bytes cannot be recovered;
this is not byte-exact clipboard or file preservation.

The [Preview contract](bounded-candidate-preview.md) defines exact input grammar,
resource limits, DTOs, one-shot Raw IPC, single-flight worker ownership,
request/revision handling, fixed errors, and privacy limitations. Requests retain
`directConstraints` and add `clues` and `hypotheses`, including typed `effects`.
Responses retain `constraintBindings` and add `hypothesisBindings` for every
Clue-backed Hypothesis, including disabled and zero-effect ones. Shared
`matchedSoftHypothesisIds` trace through validated bindings and the submitted
request snapshot to interpretations and original Clue text/source. Later draft
values never reinterpret a response.

A single monotonically increasing nonzero `u32` editor allocator spans Direct
Constraints, Clues, Hypotheses, and effects without reuse after deletion or Clear.
Rust separately assigns request-local Core IDs in Seed, Direct Hypothesis/effect,
Clue, then Clue-backed Hypothesis/effects order, with a maximum ID of 128. Both
Raw and canonical typed requests are checked against 16,384 bytes; the whole
input shares that budget in addition to individual caps. Canonical counting
also covers shorter equivalent representations accepted by direct Serde parsing.
Responses are limited to 65,536 bytes, attempts to 4,096, emissions to 100,
and active workers to one with no waiting queue. These checks do not bound
WebView/transport allocations before Rust receives the body.

The auxiliary custom command, `core_probe`, takes no frontend input. It constructs a
fixed synthetic `RawProject` containing one enabled, manual-origin Hard
hypothesis with a Length constraint of 2 through 4. It calls `validate()`,
`resolve()`, and `evaluate()` on the synthetic cases `"abc"` and `"a"`.

Only the resulting `hard_eligible` booleans cross IPC:

```json
{"acceptedCaseEligible": true, "rejectedCaseEligible": false}
```

The candidate strings, hypotheses, constraints, seeds, and full domain model
for this fixed probe remain inside Rust. Preview separately transfers its
bounded user-authored input and candidate response through local IPC.
Validation/evaluation failures in the probe become the fixed codes
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
continues to hold Loading until its Promise settles. This includes all Clue,
Hypothesis, origin, effect, and Direct Constraint edits and rejected observable
raw input. Clear empties those lists without reusing editor IDs or cancelling
Rust. A complete Project Workspace, AlternativeGroups, persistence, and export
remain unavailable.

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
synthetic fixtures. Clue text/source, interpretations, constraints, Seeds, and
candidates exist in memory and local IPC only; Clear does not promise secure erasure. Raw IPC limits application
DTO parsing, not prior WebView/transport allocations or a compromised renderer.
Rust tests exercise the generated ACL with Tauri's mock runtime:
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
4. Clear, enter seeds `aB`, `AB`, and `12`, select only Original and the two
   no-affix options. Add an enabled Hard Contains condition with literal `B`.
   Add an enabled Soft Prefix condition with literal `a` and High confidence.
   Preview must emit `aB` then `AB`, with attempted 3, Hard rejected 1, emitted 2,
   and Completed. `aB` has 3 relative points and identifies the Soft Prefix row
   as matched; `AB` has 0 points and no Soft matches. Confirm literal case and
   request-snapshot provenance. Disable the Hard row and rerun: all three seeds
   are emitted, and the Soft score still does not reorder them.
5. Reenable the Hard row and change its literal to `Z`. Preview is Empty with
   attempted 3, Hard rejected 3, emitted 0, and Completed, without an
   Unsatisfiable claim. Restore literal `B`, set attempt limit 1, and rerun:
   the first record is delivered with AttemptLimit and Limited. Restore limits,
   remove or disable conditions, Clear, enter a valid seed, and verify a new
   successful Preview with no conditions. Confirm invalid disabled rows block
   Preview until corrected or removed.
6. Where the native clipboard/drop path can safely preserve synthetic source
   text, try an invalid control-bearing insertion in a new constraint literal,
   Position allowed-character field, and numeric text field. Confirm a fixed
   rejection, preserved previous value, and invalidated results, then correct
   the field and rerun. Record exactly which native transfer/beforeinput paths
   were exercised; a jsdom test does not establish native behavior.
7. Clear while a controlled request is pending when testing lifecycle behavior:
   Clear invalidates display, but is not cancellation. The normal bounded run
   may finish too quickly to observe Loading; focused frontend tests deliberately
   hold Promises pending for these transitions.
8. Clear, use Original with no affixes and synthetic seeds `aB`, `AB`, and
   `12`. Add a Clue with text `two clues\n合成 😀` and a separate source
   ` example\t source `. Add a Hard Hypothesis with Contains `B` and Length
   2–2 effects. Add a Medium Soft Hypothesis on the same Clue with Prefix `a`
   and Suffix `B`. Preview emits `aB` and `AB`: `aB` adds exactly 2 points,
   `AB` adds 0, and the Soft provenance displays the submitted interpretation,
   original Clue text, and source. Add a direct Soft Length 2–2 condition with
   Low confidence and rerun: points become 3 and 1, in the same order.
9. Confirm a referenced Clue cannot be deleted. Disable its Hypotheses and
   confirm deletion is still blocked. Remove all effects from one Hypothesis;
   it remains visible, contributes no points or filtering, and still blocks
   Clue deletion. Explicitly reassign all referring Hypotheses to a second
   Clue or delete them, then remove the unreferenced Clue. Each operation must
   invalidate results. Verify an invalid disabled effect blocks Preview and
   an empty interpretation is accepted. Clear and successfully rerun seed-only
   Preview. Exercise LF/TAB/Emoji and byte-limit rejection in evidence fields,
   IME composition, observable CR/CRLF paste/drop/beforeinput rejection, and
   noncancelable restoration where native facilities permit. Record actual
   paths exercised without inferring them from jsdom tests.
10. Build with `pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked`.
   Stop Vite, launch `./target/release/cluebench-desktop` on Linux or
   `.\target\release\cluebench-desktop.exe` on Windows, and repeat the checks.
   This verifies bundled assets, production CSP and real Raw IPC separately.
11. Run `pnpm -C apps/desktop dev` alone in a browser and submit valid input.
   Confirm Desktop unavailable, safe retry, and no fabricated candidates.
12. Optionally run the auxiliary Core connection probe. Close the native window
   normally and confirm clean process exit.

Controlled frontend tests cover rejected invocations and recovery without adding
a privileged debug command to the product. Do not report a browser render,
mock-runtime ACL test, or compile as a native GUI/IPC success. Record frontend
build, Core tests, Linux build, Windows build, GUI startup, real IPC, and GitHub
Actions separately in the implementation report and relevant PR. Historical
verification records remain unchanged.

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

The Desktop does not implement a complete Project Workspace, AlternativeGroup
editing or exclusive selection, Hypothesis-owned Seed effects, structural
analysis UI, structure canvas, export, persistence/migration/encryption, general
generation jobs, streaming, interactive cancellation, routing/global state/i18n
frameworks, final branding, installers, signing, updates, release automation,
macOS/mobile readiness, CLI/HTTP APIs, stable external Rust APIs, or runtime AI.
Preview request/revision handling and bounded worker ownership are implemented
locally without introducing those broader frameworks. Issue #11's glib
vulnerability investigation remains separate and unresolved by this change.
