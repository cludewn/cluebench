# Desktop shell verification

Issue [#9](https://github.com/cludewn/cluebench/issues/9) implementation record,
2026-10-29. The latest issue body and its 16 acceptance criteria were read before
implementation. This report distinguishes executed checks from static review
and checks that require another environment. See the
[development guide](desktop-development.md) for setup and repeatable commands.

**Overall verdict: CHANGES REQUIRED for full issue acceptance.** Local Linux
implementation and runtime checks pass; acceptance criterion 13 still requires
an actual Windows native build. The Windows CI job is configured but has not
run. No code defect is currently identified by the executed checks.

## Environment and changes to the local setup

- Branch: `feat/desktop-app-shell`.
- Starting HEAD: `cb879922b4c31132fdb0932096ca70295c2c70f0`.
- Starting tracked/untracked working tree: clean.
- Linux: Ubuntu 24.04.5 on WSL2, x86_64, kernel
  `6.6.87.2-microsoft-standard-WSL2`, with WSLg/X11 display `:0`.
- Rust/Cargo: 1.99.1, installed with rustfmt and Clippy, selected by the new
  `rust-toolchain.toml`; the previous default `stable` resolved to 1.94.0.
- Node.js: 24.22.2, downloaded from the official Node distribution, SHA-256
  verified, and unpacked under `/tmp/cluebench-tools` for this session.
- pnpm: 10.34.9, installed under `/tmp/cluebench-tools/pnpm`. The host's existing
  Node 18 was left in place. Verification prepended these temporary tools to
  `PATH`; install the pinned tools normally for later sessions.
- Native setup: ran `apt-get update` and `apt-get install -y
  --no-install-recommends build-essential pkg-config libwebkit2gtk-4.1-dev
  libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev xdotool
  x11-apps python3-gi gir1.2-atspi-2.0`. Ordinary sudo required a password;
  the authorized installation used `wsl.exe -d Ubuntu -u root -- bash -lc`.
  WebKitGTK 4.1 reports **2.52.6**, GTK 3 reports **3.24.41**.
  `xdotool`, X11 tools and Python GI are session verification tools only,
  not application dependencies.
- Windows prerequisites were inspected read-only: WebView2 Runtime was present;
  Visual Studio Installer's `vswhere` and an MSVC installation were not found,
  no Windows SDK path or native Cargo/Node/pnpm command was reported. This is
  not a usable native Windows build environment.
- An initial pnpm 10.28.2 tooling candidate was replaced after npm audit reported
  known High advisories. The selected 10.34.9 installer audit and the frontend
  dependency audit reported zero known vulnerabilities at verification time.

## Executed automated checks

| Check | Result | Evidence and limits |
| --- | --- | --- |
| `pnpm -C apps/desktop install --frozen-lockfile` | PASS | App-local lockfile accepted without resolution changes |
| `pnpm -C apps/desktop typecheck` | PASS | Strict TypeScript check |
| `pnpm -C apps/desktop test` | PASS | 13 tests across 2 files |
| `pnpm -C apps/desktop build` | PASS | Vite production HTML, JS and CSS generated locally |
| `cargo fmt --all -- --check` | PASS | Both workspace members |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | No warning suppression or lint weakening |
| `cargo test --workspace --locked` | PASS | Core: 103 unit/integration tests and 11 doctests; adapter: 5 tests |
| `cargo test -p cluebench-core --locked` | PASS | Same 114 Core tests without building the Desktop adapter |
| Generated ACL dispatch test | PASS | Actual handler/Core, Tauri MockRuntime: local main allowed, other window and external origin denied |
| `pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked` | PASS (Linux) | Native optimized binary in root `target/release`; release Cargo compilation reported 1m 22s on this machine, not a Windows/CI cold-build guarantee |
| `git diff --check` and new-file inspection | PASS | Tracked diff plus all nonignored untracked source/config/docs files inspected separately |
| Windows native build | NOT RUN | MSVC/SDK/native tools unavailable locally; CI matrix added, not executed |
| GitHub Actions | NOT RUN | No push or online workflow changes authorized |

The probe tests validate the actual Manual Hard Length 2..4 fixture, evaluate
`"abc"` as eligible and `"a"` as ineligible, serialize exactly two boolean
fields, and deliberately reverse evaluations to detect a hard-coded DTO.
Actual Core validation and candidate-domain errors are converted to fixed,
non-sensitive codes. Production command code has no `unwrap()`/`expect()`.

Frontend tests cover Idle, held Loading with disabled action, successful boolean
display, rejection, clearing Loading, retry, browser-only unavailability,
runtime response validation, and fixed/sanitized errors. Controlled mocks are
test-only; they do not prove a native WebView IPC call.

## Native runtime smoke tests

| Environment | Window / React | Real Core IPC | Details |
| --- | --- | --- | --- |
| WSL2 Linux build | PASS | Not a runtime check | Official Tauri no-bundle release build |
| WSLg Linux release | PASS | PASS | Idle, button action, `true` / `false`, repeat action; also repeated after stopping Vite |
| WSLg Linux development | PASS with session environment adjustment | PASS | Vite on strict port 1420, actual Tauri window and development CSP |
| Vite-only WebKit browser | PASS | Correctly unavailable | Actual Vite page in a WebKitGTK view without Tauri; action displays disconnected error, never Core success |
| Windows native | NOT RUN | NOT RUN | Missing local native build environment; CI and manual GUI verification remain |

The release binary was launched with
`GDK_BACKEND=x11 ./target/release/cluebench-desktop`. Actual X11 window captures
confirmed the React Idle view, followed by `Core check completed.`, accepted
`true`, rejected `false`, and an enabled repeat action after clicking the
button. Vite was stopped before the final release run, establishing that the
packaged assets and production CSP work without the development server.
Standard `WM_DELETE_WINDOW` closed both development and release applications
with exit code 0. These are Linux WebKitGTK calls, not Windows WebView2 calls.

Development initially displayed **Unspecified proxy lookup failure**, despite
the Vite server returning HTTP 200. This occurred in the host GIO proxy resolver
with proxy environment variables present. A process-local adjustment allowed
the same app and assets to render and invoke Core:

```sh
GDK_BACKEND=x11 GIO_USE_PROXY_RESOLVER=dummy \
  pnpm -C apps/desktop tauri dev -- --locked
```

This is a session environment workaround, not a global proxy or application
code change. It should be used only for local development when that failure
occurs. The normal development command was not a runtime PASS in this host's
unadjusted proxy environment. Production's local assets did not need it.

WSLg emitted nonfatal EGL/DRI3 acceleration warnings; rendering and IPC still
worked. An initial verification-tool `xdotool windowclose` destroyed the X11
drawable and produced a GDK `BadDrawable` exit. This did not test the normal
close path; repeating with `WM_DELETE_WINDOW` exited cleanly. The Windows
Computer Use tool could not initialize for this WSL workspace, so Linux X11
window captures/input were used instead. No Windows GUI PASS is implied.

The standalone browser check used a temporary Python GI/WebKitGTK harness,
loading the real Vite page without Tauri, clicking the UI action and checking
the disconnected result. No browser harness, fake Core or extra E2E dependency
was added to the repository. Loading/rejection/retry transitions remain covered
by the focused frontend tests; the tiny real probe can finish before Loading is
visually observable.

## Acceptance criteria

Numbering follows the latest Issue #9 body, not the additional implementation
instructions. Pending verification must not be treated as a PASS.

| # | Acceptance criterion | Status |
| --- | --- | --- |
| 1 | Tauri 2, React, TypeScript, Vite, pnpm | PASS: manifests, lockfiles, frontend build, adapter checks |
| 2 | Core remains UI-independent and retains behavior | PASS: no Core source/manifest changes, all existing tests |
| 3 | Native window builds and launches in a supported environment | PASS: Linux release build and WSLg native startup |
| 4 | Shell renders and does not claim unfinished features | PASS: UI tests and actual development/release rendering |
| 5 | Frontend action invokes a real Core operation through Tauri | PASS: native WebView action -> real Core -> expected booleans |
| 6 | Explicit non-sensitive bridge and error handling | PASS: no request payload, two booleans, fixed errors and shape tests |
| 7 | No frontend implementation of Core semantics | PASS: static review of all frontend sources |
| 8 | Local production assets, minimal privileges, no remote access | PASS: static ACL/CSP review, ACL tests, production native assets/IPC with Vite stopped |
| 9 | Document WSL2/Linux versus native Windows | PASS: development guide and this environment record |
| 10 | Reproducible build/workspace verification | PASS for local Linux locked checks/build; Windows evidence tracked separately under criterion 13 |
| 11 | Existing formatting, Clippy, workspace tests operational | PASS: commands above on pinned Rust |
| 12 | Frontend typecheck and production build | PASS |
| 13 | Linux and Windows compilation verified; runtime gaps identified | INCOMPLETE: Windows native CI has not run |
| 14 | Focused IPC verification beyond hard-coded mock | PASS: real Core fixture, DTO/error tests and generated ACL dispatch |
| 15 | Consistent dependencies/lockfiles, no unnecessary framework stack | PASS: shared Cargo lockfile, app-local pnpm lockfile, no plugins/UI framework |
| 16 | Accurate documentation of foundation, evidence and future work | PASS: recorded executed results, proxy workaround and unverified Windows/CI |

## Workspace, security, and dependency review

The sole Cargo workspace retains edition 2024 and resolver 3. Its two members
share root `Cargo.lock`, which now resolves 407 packages including the two local
crates. `cluebench-core` still has zero third-party dependencies; its path
dependency is consumed only by the Desktop adapter. No Core public API or
semantics were changed. The lockfile expansion is the native framework and its
build, platform and test dependencies, not a new Core dependency stack.

Downloaded package metadata declares at most Rust 1.90 across the lock graph;
the selected 1.99.1 is validated by the actual local checks above. This is not a
cross-platform MSRV proof. CI uses the same repository pin and no moving stable
environment override. Direct dependency purposes, versions and licenses are in
the development guide. No dependencies were removed from Core, no Core license
was changed, and new source files carry MPL-2.0 headers.

Static review confirms `AppManifest::commands(&["core_probe"])`, explicit
`main-probe` capability selection, local-only exact `main` window scope and only
`allow-core-probe`. Tauri default features are disabled, including dynamic ACL;
there is no `core:default` permission grant or Tauri plugin. Production CSP has
no development localhost/WebSocket origin, inline/eval allowance, CDN or remote
application content. Development CSP is separate. No sensitive domain data is
returned or logged. No external API, telemetry, analytics, authentication,
updater, filesystem/shell permission or separate Core process was added.

## Files and responsibilities

| Files | Main change/effect |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Add Desktop member and coherent native dependency graph |
| `rust-toolchain.toml` | Pin Rust 1.99.1, rustfmt and Clippy for local/CI |
| `.github/workflows/rust.yml` | Linux full checks plus Linux/Windows native no-bundle build matrix; read-only permissions |
| `.gitignore` | Ignore generated Tauri artifacts and private `.local/` notes |
| `apps/desktop/package.json`, `pnpm-lock.yaml`, `.node-version` | Pinned app dependencies, scripts and Node/pnpm versions |
| `apps/desktop/tsconfig.json`, `vite.config.ts`, `index.html` | Strict SPA configuration, local assets, fixed strict development port |
| `apps/desktop/src/main.tsx`, `App.tsx`, `styles.css` | Minimal Cluebench UI and explicit connection states |
| `apps/desktop/src/core-bridge.ts` | No-input invoke, DTO shape check, safe errors |
| `apps/desktop/src/App.test.tsx`, `core-bridge.test.ts` | Focused UI/bridge tests |
| `apps/desktop/src-tauri/Cargo.toml`, `build.rs` | Tauri dependencies, relative Core dependency, build and command ACL enrollment |
| `apps/desktop/src-tauri/src/main.rs`, `lib.rs`, `probe.rs` | Startup, handler, synthetic Core operation, DTO/errors and 5 Rust tests |
| `apps/desktop/src-tauri/tauri.conf.json`, `capabilities/main-probe.json` | Window, local production assets, CSP split, exact command permission |
| `apps/desktop/src-tauri/icons/icon.png`, `icon.ico`, `README.md` | Original minimal native resource placeholders and license description |
| `README.md`, `README-ja.md` | Shell status, quick start and aligned feature exclusions; fix Japanese README link |
| `AGENTS.md`, `docs/product-scope.md`, `docs/core-contract.md` | Align adopted adapter, workspace and independent Core verification |
| `docs/desktop-development.md`, `docs/desktop-verification.md` | Architecture/setup/security/licenses/manual checks and execution evidence |

## Remaining verification and scope

After the developer's independent final review and authorized commit/push, check
both native CI jobs on the exact pushed commit. Windows compilation has not
passed yet. On native Windows with the documented prerequisites, run the frozen
install, no-bundle build, and both development/release smoke procedures from
the guide; record the actual GUI and Core boolean results separately.
The CI timeout is an initial 45-minute budget, not a measured Windows cold-build
guarantee. No hosted CI or Windows GUI success is claimed here.

Editors, canvas, Candidate Preview, Wordlist Export, save/load, persistence,
encryption, streaming/jobs/revisions/backpressure, full domain DTOs, global state,
routing/i18n frameworks, final branding, installers, distribution/signing,
updates, macOS/mobile readiness, CLI/HTTP/stable external API and runtime AI are
deferred. Future candidate work must handle the Core generator's synchronous
callbacks and borrowed data in an application job/ownership design.

All implementation changes are intentionally unstaged and uncommitted. No
stage, commit, push, branch operation, Issue/PR mutation or online workflow
mutation was performed. Private notes and the old PoC were not read.
Final local state: branch `feat/desktop-app-shell`, unchanged HEAD
`cb879922b4c31132fdb0932096ca70295c2c70f0`, 9 modified tracked files and 25
nonignored untracked files. The index is unchanged. All 23 new text files were
checked for whitespace/conflict markers separately from `git diff --check`;
the two original icon resources were also inspected. Relative documentation
links and the generated app ACL/capability were checked successfully.
