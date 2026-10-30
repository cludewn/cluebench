# Candidate Preview verification

This is the original Issue #12 implementation record. The subsequent limited
DOM input-normalization correction has its own
[verification record](input-normalization-verification.md), including current
Frontend test counts and the precise input-event guarantee.

Issue [#12](https://github.com/cludewn/cluebench/issues/12), implementation and
local verification on 2026-10-30 (Asia/Tokyo). The latest Issue body and its 17
acceptance criteria were read before implementation. This record does not
replace the historical [Issue #9 verification](desktop-verification.md).
The [Preview contract](bounded-candidate-preview.md) explains implementation
boundaries; the [development guide](desktop-development.md) gives commands.

**Overall verdict: CHANGES REQUIRED for complete cross-platform acceptance.**
Local automated checks and Linux production GUI/Raw IPC pass. Windows native
build/GUI and GitHub Actions are
unverified; configured CI is not evidence of an executed Windows build.
No currently known implementation defect remains after local review.

## Environment and Git boundary

- Starting and final branch: `feat/bounded-candidate-preview`.
- Starting and final HEAD: `3d13905fd79ffa2705fd674378fd33f5170e5247`.
- Starting working tree and index: clean.
- Ubuntu 24.04/WSL2 with WSLg; WebKitGTK 4.1 version 2.52.6, GTK 3.24.41.
- Rust/Cargo 1.99.1 from the repository toolchain; Node 24.22.2 and pnpm
  10.34.9 reused from `/tmp/cluebench-tools` with a process-local PATH.
- No system package installation or unrelated dependency/security update.
- All work remains unstaged/uncommitted. No staging, commit, push, branch
  creation/switch, merge/rebase, PR/Issue mutation or GitHub setting change.

## Executed automated checks

| Check | Result | Evidence / limitation |
| --- | --- | --- |
| `pnpm -C apps/desktop install --frozen-lockfile` | PASS | Existing lock accepted, no resolution changes |
| `pnpm -C apps/desktop typecheck` | PASS | Strict TypeScript |
| `pnpm -C apps/desktop test` | PASS | 83 tests, 4 files |
| `pnpm -C apps/desktop build` | PASS | Production Vite assets |
| `cargo fmt --all -- --check` | PASS | Entire workspace |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | No lint suppression |
| `cargo test --workspace --locked` | PASS | Core 103 unit/integration + 11 doctests; Desktop 24 tests |
| `cargo test -p cluebench-core --locked` | PASS | Same 114 Core tests independently |
| `cargo test -p cluebench-desktop --locked` | PASS | Focused adapter run, 24 tests |
| Generated Preview ACL dispatch | PASS | Actual command/Core with Tauri MockRuntime; local main allowed, other window/remote denied |
| `pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked` | PASS (Linux) | Production assets and optimized native binary, no installers |
| Native production GUI / actual Raw IPC | PASS (WSLg Linux) | Actual input/actions, Core candidates/provenance/stats/stops, normal exit 0 |
| `git diff --check` and untracked-file inspection | PASS | All 9 new text files checked for contents, license headers where applicable, whitespace and conflict markers; relative doc links checked |
| Windows native build / GUI | NOT RUN | No configured Windows native compiler/SDK/toolchain environment |
| GitHub Actions | NOT RUN | Push prohibited; no hosted workflow invoked |

Initial frontend type narrowing and test assertion/harness failures were fixed
before the final passing typecheck/test/build. These were not ignored failures.
The existing Linux/Windows workflow is unchanged: read-only native matrix,
Linux frontend/Rust checks and native no-bundle builds on both operating systems.
The Core source, tests, manifest and absolute generation limits are unchanged.

## Adapter tests

Desktop has 24 tests: 18 new Preview unit tests, one new generated Preview ACL
test, and five existing probe/ACL tests. Important cases are grouped rather than
mirroring each Core algorithm test:

- Actual Raw request, ordinary JSON rejection, exactly 16 KiB versus one byte
  over, malformed JSON, unknown fields/variant, invalid types and huge numbers.
- Request ID bounds; 0/64/65 seeds; 64/65 bytes; controls/non-ASCII; preserved
  spaces; equal text with distinct seed IDs.
- Explicit/ordered case and affix lists, empty option versus empty list,
  duplicate rejection, affix counts/domain and the 1,043-byte config maximum.
- Positive/default/max attempt/emission allowances, independent stops, 100
  records, exact final-path Completed, and all-domain-skip attempt limiting.
- Owned callback conversion after borrowed inputs expire, path/seed/config
  provenance, affix case, duplicate paths, Core counter accounting/stop mapping.
- Exactly 64 KiB versus oversized serialized response, failing without trimming.
- A held worker with 16 concurrent competitors: every competitor is Busy and its
  closure never starts. Successful completion permits the next worker.
- Dropped awaiting Future and abort of a started blocking JoinHandle do not
  release the slot before actual worker completion. Validation error and
  panic/JoinError map to fixed errors and allow retry.
- Real generated command ACL accepts local main and denies other windows and
  remote origins. It preserves the probe grant and checks actual Raw dispatch,
  JSON rejection, and oversized rejection. This is MockRuntime, not a WebView.

## Frontend tests

83 tests comprise App/UI 21, Preview bridge 29, Preview input 24, and existing
Core bridge 9. They cover multiline LF/CRLF/trailing terminator grammar, invalid
empty lines, exact space/duplicate preservation, ASCII/UTF-8 and count bounds,
case ordering, explicit no-affix options, affix row order/blank validation,
limits/defaults and explicit request construction. Bridge tests assert actual
Uint8Array argument encoding, no unrelated-field forwarding, non-Tauri failure,
fixed safe errors, strict response/accounting/provenance checks and rejection
of impossible one-shot cancellation/consumer-stop results.

UI tests hold Promises pending to verify Loading/Busy, all display states,
Empty plus Limited, retry, duplicate candidate paths rendered as text,
request IDs/revisions, stale success and stale error, Clear during a pending
request, editing options while pending, and unmount/remount isolation. Clear
never marks a pending request complete. No mock result is reported as actual
Core/WebView IPC evidence.

## Native execution

The final release binary was launched from `target/release/cluebench-desktop`
with `GDK_BACKEND=x11` in a temporary `dbus-run-session`. It loaded production
bundled assets; no Vite development server was started for this verification.
A temporary Python GI/AT-SPI and X11 keyboard harness operated the actual
WebKitGTK window and read its accessible text/table, without replacing IPC or
adding a debug command. The harness and synthetic captures/logs remain under
`/tmp`, outside repository artifacts.

The final full smoke suite exited 0 and verified:

- Seeds `aB` and `12`, all three cases, prefixes `""`/`PRE-`, and suffixes
  `""`/`-X`: 24 ordered records. The first four were `aB`, `aB-X`, `PRE-aB`,
  `PRE-aB-X`. Equal digit text remained three case-path records. Seed 2 and
  path `1/0/0/0` were present; all relative points were zero.
- Attempted/emitted/potential paths 24, domain skips/Hard rejections 0, Completed.
- A second Preview returned 24 records again through actual Raw IPC.
- Editing to an internal empty line removed the table and displayed Invalid
  with the fixed second-line error; restoring valid input allowed another run.
- Emission limit 1 returned one record, one attempt, and actual OutputLimit.
- A 64-byte seed with only a nonempty prefix, three cases/two suffixes and attempt
  limit 2 returned zero candidates, two domain skips/two attempts, six potential
  paths, actual AttemptLimit, and simultaneous Empty plus Limited display.
- Clear removed results/reset options; a subsequent default Preview returned
  one candidate with Completed. A native screenshot captured this final state.
- Normal `WM_DELETE_WINDOW` closed the app with exit code 0.

The existing session lacked an accessibility bus. Using a temporary D-Bus
session enabled inspection without changing application or system settings.
Initial harness attempts failed because WebKit did not expose EditableText,
the GI Text call was incorrect, and the capture subprocess selected Wayland
while the app used X11. These verification-tool problems were corrected with
ordinary X11 keyboard input, the explicit AT-SPI Text API and an X11 capture
environment; they are not counted as earlier GUI passes. One intermediate input
attempt also joined lines because X11 text typing did not emit Return, so the
final suite sent explicit Return keys and asserted 24 records. Nonfatal WSLg
EGL/DRI3 graphics warnings did not prevent rendering or IPC.

Current development-mode GUI and standalone-browser runtime were not rerun;
this task's native runtime evidence is the production Linux binary. Frontend
mock tests cover non-Tauri behavior and held-pending stale/Clear/unmount cases;
the brief real operation does not establish visual observation of every pending
transition. No Windows GUI or CI result is implied by Linux WSLg execution.

## Acceptance criteria

Numbering follows the 17 checkboxes in the latest Issue #12 body.

| # | Criterion | Status / evidence |
| --- | --- | --- |
| 1 | Manual seed entry and validation | PASS: exact grammar, fixed line errors, input/UI/adapter tests and production GUI |
| 2 | Case transformations and fixed affixes | PASS: explicit case order and ordered options; tests |
| 3 | Existing Rust Core performs generation | PASS: RawProject → validate → resolve → generate; handler tests and actual production Raw IPC |
| 4 | Core order and duplicate paths | PASS: callback order preserved, path-based records/React keys |
| 5 | Meaningful seed/transformation provenance | PASS: record.seed/path/config/evaluation copied into DTOs |
| 6 | All inputs/outputs bounded | PASS: pre-deserialize Raw cap, DTO/config/attempt/emission/response caps |
| 7 | Responsive Preview execution | PASS: parsing/generation/serialization in spawn_blocking; production native actions complete |
| 8 | Overlap and stale results | PASS: worker-owned single-flight; ID/revision gates for success and error |
| 9 | Accurate cancellation boundary | PASS: no interactive cancellation or manufactured Core stop; Clear only invalidates display |
| 10 | Honest counters/termination | PASS: actual Core stats/stop retained, Empty/Limited coexist, no unsatisfiability claim |
| 11 | Invalid/empty/limited/invocation failure states | PASS: focused input, bridge and UI tests |
| 12 | No logging/persistence/external transmission | PASS by static review: local IPC only, no new data sinks; memory limitations documented |
| 13 | Scoped Tauri permission | PASS: actual generated ACL local main/other window/remote tests, CSP unchanged |
| 14 | UI-independent unchanged Core | PASS: no Core changes, 114 regression tests |
| 15 | Focused Rust/frontend boundary tests | PASS: Desktop 24 and frontend 83 tests |
| 16 | Workspace checks and Linux/Windows CI operational | PARTIAL: workspace/Linux native build and GUI pass, workflow retained; Windows build/Actions not executed |
| 17 | Accurate implemented/remaining documentation | PASS: contract, setup, both READMEs and separate evidence record |

## Dependencies, privacy, and remaining work

The existing pinned `serde_json =1.0.154` moved from dev-dependencies to normal
Desktop dependencies for bounded request parsing and response serialization.
Its license remains MIT OR Apache-2.0. No package/version was added, removed or
updated; Cargo/pnpm lockfiles and frontend manifests are unchanged. No plugins,
new origins or shell/filesystem/HTTP permissions were added. New source files
carry MPL-2.0 headers. This is not a new transitive license or vulnerability audit.
Issue #11's glib investigation remains separate; no resolution is claimed.

Raw IPC cannot cap WebView/transport allocations preceding the adapter check.
A compromised renderer already sees its input. Clear only reduces retained
references, without secure memory erasure. Started workers run to their Core
bounds even after display invalidation or an abandoned caller; no cancellation
service, streaming or jobs framework is introduced. The 4,096 attempt cap also
applies when every path is skipped.

Editors, alternative selection, persistence, canvas, wordlist/clipboard export,
interactive cancellation, full streaming, general jobs, advanced ranking,
deduplication, recursive transforms, CLI/HTTP APIs, runtime AI, remote
authentication, packaging and signing are intentionally separate work. They
are not newly created defects of this Preview.

After independent final review and an explicitly authorized commit/push, verify
both Linux and Windows GitHub Actions jobs on the exact pushed commit. Windows
needs its native no-bundle build and WebView2 GUI/IPC smoke procedure; CI does
not automate GUI tests. Do not upgrade this record's Windows/CI status without
actual execution evidence.

## Files and final local state

| Files | Role / effect |
| --- | --- |
| `src-tauri/src/preview.rs` | Raw boundary, limits, DTOs, fixed errors, worker/single-flight, Core integration and 18 tests |
| `src-tauri/src/lib.rs` | Manage Preview state, register command, generated Raw command ACL test; retain probe |
| `src-tauri/build.rs`, `src-tauri/capabilities/main-probe.json` | Enroll/grant only the additional local-main command |
| `src-tauri/Cargo.toml` | Promote existing serde_json to runtime dependency |
| `src/preview-input.ts`, `src/preview-bridge.ts` | Input grammar/bounds, explicit DTO/Raw encoding and safe response/error checks |
| `src/CandidatePreview.tsx` | Primary Preview UI, state/revision/request ownership, bounded records/provenance/stats |
| `src/CoreProbe.tsx`, `src/App.tsx`, `src/styles.css` | Auxiliary probe, primary Preview composition and small responsive form/table styling |
| `src/App.test.tsx`, `src/preview-input.test.ts`, `src/preview-bridge.test.ts` | Focused frontend input/bridge/lifecycle coverage; existing core-bridge tests retained |
| Both READMEs | Current Preview status, usage, bounds/privacy, remaining MVP |
| `docs/bounded-candidate-preview.md`, this report | Exact implemented contract and new execution evidence |
| `docs/desktop-development.md`, `docs/desktop-verification.md` | Current setup/manual steps and pointer from historical evidence |
| Product scope, Core contract, bounded generation/analysis docs | Remove stale repository-level preview exclusions while keeping Core responsibilities distinct |

Source paths in this table are relative to `apps/desktop`; documentation paths
are relative to the repository root. The final working tree contains 15 modified
tracked files and 9 untracked text files, with 0 staged files. HEAD and branch
are unchanged. No local commit or push was created. Changes await the user's
independent final review.
