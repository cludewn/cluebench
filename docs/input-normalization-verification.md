# Preview DOM input-normalization correction

Limited follow-up to Issue [#12](https://github.com/cludewn/cluebench/issues/12),
2026-10-30 (Asia/Tokyo), after independent final review identified one required
Medium input-boundary defect. This record supplements, rather than rewrites,
the [original implementation verification](candidate-preview-verification.md).

**Verdict: PASS for the limited correction and executed automated checks.**
Native transfer evidence is recorded separately below. Unexecuted Windows/CI
checks do not determine this limited fix's verdict.

## Cause and correction

The pure Seed parser correctly rejects a lone CR and the affix validator rejects
CR/LF. However, HTML changes values before React `onChange`: textarea API values
normalize CR/CRLF to LF, and text inputs strip CR/LF. The previous UI could thus
accept a different valid value after the original invalid text disappeared.

The new `use-raw-text-guard.ts` hook inspects available raw text in React Paste
and Drop handlers and a native `beforeinput` listener. It rejects a Seed lone CR
or affix CR/LF before cancelable insertion. It checks both InputEvent `data` and
`dataTransfer` rather than preferring a possibly empty transfer value. Native
line-break/paragraph input types are also rejected for affixes. Ordinary domain
and size validation remains in `preview-input.ts`, unchanged.

Rejected source text is never interpolated into errors. The previous value
remains; the rejection becomes a fixed validation message and disables Preview.
It synchronously increments Input Revision and invalidates displayed results
without clearing pending ownership. A normal edit or Clear dismisses the
rejection. Stale successes/errors remain discarded until the original Promise
settles. Observed noncancelable invalid beforeinput uses a fallback:
refuse the ensuing change and restore the previous controlled value. This
cannot guarantee the DOM was never transiently mutated.

Valid Seed LF/CRLF transfers retain native insertion behavior. CRLF-to-LF in a
textarea preserves the explicitly allowed line-separator meaning. Spaces,
duplicate Seeds, order, case and valid affix characters remain unchanged.
There is no custom editor, new dependency, manual paste replacement, or Core
algorithm change.

## Guarantee by input route

| Route | Guarantee / limitation |
| --- | --- |
| Paste with exposed `clipboardData` plain text | Screen the exposed text before insertion; prevent forbidden Seed/affix line endings |
| Drop with exposed `dataTransfer` plain text | Same screening before default insertion |
| Cancelable native beforeinput with raw `data`/`dataTransfer` | Prevent observed forbidden line endings, plus affix line-break input types |
| Noncancelable beforeinput with observed forbidden data | Reject the following change and restore controlled value; transient DOM mutation cannot be prevented |
| Valid LF/CRLF Seed transfer | Do not cancel; normal value validation follows insertion |
| Ordinary typing | Existing controlled value validation; additional raw beforeinput screening when data is available |
| Only normalized `event.target.value`, DOM setters, autofill/IME without raw data | Original text cannot be reconstructed; no pre-normalization guarantee |
| OS/browser preprocessing before the event | Clipboard/event text may already be normalized; original source bytes cannot be recovered |

The hook does not claim all input routes expose original data, nor protect
against a compromised renderer. React's onBeforeInput is polyfilled, so the
actual native event is observed directly. The original Privacy/Raw IPC boundary
and its renderer/transport limits still apply.

Primary references inspected:
[HTML textarea](https://html.spec.whatwg.org/multipage/form-elements.html#the-textarea-element),
[HTML text input](https://html.spec.whatwg.org/multipage/input.html#text-(type=text)-state-and-search-state-(type=search)),
[React event behavior](https://react.dev/reference/react-dom/components/common#onbeforeinput),
[Input Events Level 2](https://www.w3.org/TR/input-events-2/) (Working Draft), and
[Clipboard processing](https://www.w3.org/TR/clipboard-apis/#write-content-to-the-clipboard).
Pinned React 19.3.1 and jsdom 30.1.3 were checked locally.

## Executed automated verification

| Command / check | Result |
| --- | --- |
| `pnpm -C apps/desktop typecheck` | PASS |
| `pnpm -C apps/desktop test` | PASS: 112 tests in 5 files; previous 83 + new 29 |
| `pnpm -C apps/desktop exec vitest run src/input-events.test.tsx` | PASS: new 29 tests |
| `pnpm -C apps/desktop build` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS: Core 103 unit/integration + 11 doctests; Desktop 24 |
| `pnpm -C apps/desktop tauri build --no-bundle --ci -- --locked` | PASS: Linux production binary |
| `git diff --check`, new-file and link inspection | PASS |
| Windows native / GitHub Actions | NOT RUN: deferred until authorized push; no new platform claim |

New tests render the actual React App and mock only IPC. They cover Seed lone-CR
paste, Prefix LF, Suffix CR/LF/CRLF, invalid Drop for all three fields, native
beforeinput cancellation, noncancelable fallback, both raw InputEvent sources,
fixed errors/no source disclosure, retained previous values and disabled action.
Valid cases include LF/CRLF, final terminator, duplicates, leading/trailing Space,
64-byte boundaries, affix case/Space, direct edits, explicit empty affixes and
case selection. Rejected transfers invalidate an existing result and discard
both stale success and stale error while Loading remains until settlement.

A separate component test directly demonstrates jsdom DOM setter normalization.
It does not present an injected change event as proof of native Paste/Drop.
jsdom does not execute the native default insertion for synthetic Paste/Drop:
valid-transfer tests first check that the event is not prevented, then explicitly
model a known-valid insertion with change. The final whole suite passed; no
unresolved automated test failures remain.

## Native browser / WebView evidence

**Native Paste/Drop: NOT VERIFIED in this follow-up.** The original Issue #12
production GUI/IPC record does not establish this new guard's native transfer
behavior.

The root's first Tauri/GTK fixture attempt failed to acquire the visible X11
window after accessibility initialization, before transfer operations. Its
cleanup also encountered an unset-window-variable harness error. A separate,
bounded standalone WebKitGTK fixture then loaded the actual new production
bundle from a temporary local HTTP server. Its first native-focus step failed
with X11 `XSetInputFocus` BadMatch. A retry with explicit presentation/window map
timed out after six seconds. These were verification-tool/window setup failures;
no native Paste or Drop was executed and no native transfer PASS is claimed.
The standalone fixture did not modify the clipboard because failure preceded
clipboard setup. This browser render is not Tauri/real IPC evidence.

The synthetic component tests prove event cancellation, retained values and
state transitions for supplied transfer data, not OS clipboard or native drag
insertion. Noncancelable real IME, autofill, other browser engines, prior
OS/browser source normalization and normalized change-only paths remain
unverified or outside the stated guarantee. A manual follow-up can use synthetic
text with actual Ctrl+V and external text drag-and-drop in a working Linux/WSLg
or Windows native GUI; inspect unchanged target text and fixed errors for
forbidden line endings, and preserve LF/CRLF/Space/duplicates for valid input.

Temporary fixture scripts/logs remain under `/tmp`; they are not product
features or repository dependencies. No debug command or test-only privileges
were added to the application.

## Scope, optional improvements, and review status

Required changes: `apps/desktop/src/CandidatePreview.tsx`, new
`use-raw-text-guard.ts` and `input-events.test.tsx`, the input-boundary section in
`docs/bounded-candidate-preview.md`, the pointer in the original verification
report, and this correction record.

Optional small improvements: the two English paragraphs in `README-ja.md` were
translated into Japanese without changing behavior claims; the final
`docs/product-scope.md` verification link now targets the current Preview
record. Serde representation flexibility was not redesigned. No package,
license, manifest or lockfile was added/updated/removed. New source headers
remain MPL-2.0.

This correction makes no changes to Core, Rust adapter, request/response DTO,
16/64 KiB boundaries, Preview resource limits, spawn_blocking, worker guard,
single-flight, Request IDs, Core stats/stops, ACL, CSP, or cancellation policy.
The added rejection path uses the existing revision/stale-result boundary.
The glib investigation remains separate and unresolved by this correction.

| Re-evaluated AC | Result |
| --- | --- |
| #1 Manual Seeds / validation | PASS within the explicit raw-event boundary; original inaccessible data is not reconstructed |
| #11 Clear invalid/failure handling | PASS: fixed validation rejection, preserved values, safe recovery; pending state preserved |
| #15 Focused tests | PASS: 29 new regression cases plus all previous tests |
| #17 Accurate documentation | PASS: observed input routes and unavailable guarantees explicitly separated |

The original 17-AC/platform record remains historical, including its Windows/CI
pending status. This limited follow-up is ready for independent re-review.
All existing uncommitted implementation work is retained. No stage, commit,
push, branch operation, GitHub/Issue/PR mutation, or dependency update occurred.
Final state: branch `feat/bounded-candidate-preview`, unchanged HEAD
`3d13905fd79ffa2705fd674378fd33f5170e5247`, 0 staged files, 15 modified tracked
files and 12 untracked files. The 15 tracked modifications and 9 of the
untracked files predate this correction; existing implementation work is
retained. This correction adds three files (guard, regression tests, record)
and updates five already-present files. No commit or push was created.
