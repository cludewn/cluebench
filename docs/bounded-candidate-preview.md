# Bounded desktop candidate preview

Issue [#12](https://github.com/cludewn/cluebench/issues/12) adds the first
user-driven Desktop preview. The React interface accepts manual seeds and
explicit case/prefix/suffix options. The existing UI-independent Rust Core
performs generation; the frontend does not implement a generator or evaluator.
This is a bounded exploration of configured paths, not a password-space search,
password verifier, or structural satisfiability analysis.

## Input and transformations

Manual seeds use one line per seed, preserving order and duplicate lines.
LF and CRLF delimit lines. One final line terminator is ignored: `A\n` is one
seed, while `A\n\n` contains an invalid empty second seed. Empty overall input
cannot run. Nothing trims spaces, normalizes Unicode, changes case, or removes
duplicate seeds. A seed containing only spaces is valid. Each seed must contain
1–64 printable ASCII bytes (`0x20..=0x7E`); tabs, lone CR, controls, and non-ASCII
text are invalid. Fixed validation messages identify positions without echoing
input. Byte limits use UTF-8 encoding, not JavaScript string length alone.

### DOM text entry boundary

HTML textarea values normalize CR/CRLF to LF, and single-line text inputs
strip CR/LF. Validation of `onChange` values alone cannot detect the original
forbidden line endings. The Preview screens text exposed by
`paste.clipboardData`, `drop.dataTransfer`, and native `beforeinput.data` or
`beforeinput.dataTransfer` before value sanitization. A lone CR in Seed transfer
text, or any CR/LF in affix transfer text, rejects the entire insertion with a
fixed message that contains no source text. Valid Seed LF/CRLF transfers use
the browser's default insertion; permitted CRLF-to-LF conversion preserves the
seed meaning. Other domain/size errors remain the existing value validator's
responsibility.

Rejected operations retain the previous field value, invalidate results and
the input revision, and leave an outstanding request Loading until its actual
Promise settles. A later ordinary edit or Clear clears the rejection. Native
`beforeinput` is observed directly because React's `onBeforeInput` is polyfilled.
Cancelable invalid events are prevented. For an observed noncancelable invalid
`beforeinput`, the next change is refused and the controlled value restored;
this cannot prevent a transient DOM mutation before restoration.

These checks protect source text actually exposed by those events. They cannot
recover text normalized before the clipboard/drop event, an event with no raw
text, autofill/IME that exposes no usable raw data, or a programmatic assignment
followed only by a normalized change event. OS/browser clipboard processing
may itself normalize text before the application sees it. This is not a promise
to observe every input's original bytes, or a defense against a compromised
renderer. See the separate [normalization-fix verification record](input-normalization-verification.md)
for React/jsdom and actual browser evidence and limits.

Case choices appear in the fixed order Original, ASCII Lowercase, ASCII
Uppercase. Only selected choices are sent; Original is not implicit. Prefixes
and suffixes are ordered lists. Explicit **No prefix** / **No suffix** options
represent `""`; an empty list is invalid. Space and the empty string differ.
Affixes retain their case. Lists cannot contain duplicate options.

The Core applies `prefix + case_variant(seed) + suffix`, without recursion or
truncation. Traversal is Seed → Case → Prefix → Suffix, with suffix varying
fastest. Paths longer than the 64-byte candidate domain are counted as domain
skips. Equal text from different paths remains separate records.

## Resource limits

These inclusive Preview limits are defined in the Desktop adapter independently
of the Core's unchanged absolute limits. The frontend mirrors them for feedback.
Invalid values are rejected; no clamping occurs.

| Resource | Preview limit |
| --- | ---: |
| Raw request body, before application DTO deserialization | 16 KiB |
| Non-sensitive request ID (`A-Z`, `a-z`, `0-9`, `-`, `_`) | 1–32 ASCII bytes |
| Manual seeds | 1–64 |
| Individual seed | 1–64 printable ASCII bytes |
| Combined seed text | 4,096 bytes |
| Selected cases, without duplicates | 1–3 |
| Prefixes / suffixes, each list without duplicates | 1–8 each |
| Individual affix | 0–64 printable ASCII bytes |
| Logical generation config | 1,043 bytes |
| Attempt allowance | Default 2,048; range 1–4,096 |
| Emission allowance | Default 50; range 1–100 |
| Returned candidate records | 100 |
| Serialized UTF-8 JSON response | 64 KiB |
| Active Preview workers / waiting requests | 1 / 0 |

Logical config bytes follow the Core contract: 16 bytes for two allowances,
one byte per case, plus all affix bytes. Attempt and emission allowances are
independent. Attempts bound work even when every path is a domain skip. A
response over 64 KiB fails with a fixed error; candidates are never removed to
fit the response budget.

## Raw IPC and execution lifetime

`candidate_preview` uses Tauri 2 Raw IPC. The bridge builds an explicit request,
encodes JSON with `TextEncoder`, checks its byte length, and invokes the command
with a `Uint8Array`. The Rust entry accepts `tauri::ipc::Request`, rejects
ordinary JSON bodies, checks the raw byte length, and copies only bounded bytes.
DTO deserialization and validation occur inside
`tauri::async_runtime::spawn_blocking`, followed by
`RawProject → validate() → resolve() → ResolvedModel::generate()`.

The async command returns `Result`, following the pinned Tauri 2.12.5 command
macro's support for borrowed request/state arguments. It copies bounded bytes
before spawning the blocking worker, then awaits its JoinHandle. Generation
never runs on an async runtime worker or the UI thread. The pinned source and
[official Raw IPC/async command documentation](https://v2.tauri.app/develop/calling-rust/)
were checked; compilation verifies this exact signature. Android is outside
this implementation's scope.

An atomic non-waiting acquisition returns `PREVIEW_BUSY` immediately when the
single slot is occupied. An RAII guard moves into the blocking closure and
holds the slot until the worker actually exits, including unwinding. Dropping
the IPC Future or aborting an awaiting task cannot release the running worker's
slot. Started blocking computations are not stopped by aborting a JoinHandle.
Normal completion, validation failure, and worker panic permit a later request;
join failures become a fixed safe error. Process aborts cannot promise recovery.

There is one owned response, no channel, streaming, job manager, or waiting
queue. There is no interactive cancellation. The Core cancellation callback
always returns false, and the consumer returns `ControlFlow::Continue(())`;
the Core's configured emission limit stops delivery.

## DTO and provenance

The camelCase request contains `requestId`, `seeds`, `options` (`caseVariants`,
`prefixes`, `suffixes`), and `limits` (`attemptLimit`, `emissionLimit`). Unknown
fields, unknown case variants, malformed JSON, invalid types, duplicate options,
and out-of-range numbers are rejected with fixed non-sensitive error codes.
There is no user-defined hypothesis or constraint input in this command.

The response contains `requestId`, `effectiveLimits`, `candidates`, `stats`,
`potentialPaths`, and `stop`. During each borrowed Core callback, the adapter
copies only candidate text, seed identity, the generation path, selected
affixes, and relative points into owned DTOs. Seed IDs come from `record.seed()`;
paths come from `record.path()`; affixes come from the referenced config; points
come from the actual evaluation. Seed IDs are nonzero positions assigned for
this request only, never persistent project IDs. No clue or hypothesis origin
is invented. React keys identify paths rather than candidate text alone.

## Statistics and termination

The adapter preserves the Core counters and termination reason:

```text
attemptedPaths = domainSkips + hardRejections + emittedCandidates
candidates.length = emittedCandidates
```

`completed` means all selected generation paths were visited. `attemptLimit`
and `outputLimit` mean the corresponding Core allowance stopped work. Exhausting
the final path can return `completed` even when an allowance is reached exactly.
The frontend does not derive a stop from the candidate count. `cancelled` and
`consumerStopped` are Core concepts, but this one-shot consumer does not request
either and the UI does not manufacture them. Zero candidates never means
Unsatisfiable. This manual-only model has no user Hard/Soft constraints, so
Hard rejections and relative points are normally zero. Points are not success
probabilities.

## Frontend state and privacy

The interface distinguishes Idle, Invalid, Loading, Success, Empty, Limited,
and Error. Empty and Limited can coexist; the actual termination reason remains
visible. Execution requires a button action. Every input edit increments a
revision and invalidates displayed results. Each run has a non-sensitive request
ID and captures its input revision. Only a still-current request/revision may
publish success or error. Unmount cleanup prevents late state updates. Request
ownership also prevents an old `finally` from clearing another run's Loading.

Clear removes displayed input/results and invalidates their revision. It does
not cancel Rust, resolve an IPC Promise, or release the worker slot. While that
Promise is pending, the UI continues to report work in progress and disables
another run. Busy failures have a fixed, actionable message. No Cancel button
is provided.

Seeds/candidates cross only required local IPC. No content logging, console
logging, telemetry, persistence, URL parameters, network requests, automatic
clipboard use, or automatic file output is added. Spellcheck/autocorrection are
disabled on sensitive inputs, and candidates render as React text nodes.
Arbitrary IPC/Serde/Join diagnostics never become UI messages. Clear reduces
references; it does not securely erase JavaScript, Rust, allocator, WebView, or
transport memory. No zeroization guarantee or encryption framework is claimed.

Raw IPC bounds application copying and DTO parsing after the transport hands
the request to Rust. It cannot prevent WebView/Tauri transport allocations made
before that point, guarantee process-wide memory bounds, or protect secrets
from a compromised renderer that can already access its own input. Single-flight
bounds generation workers, not transport-level request reception.

The command is enrolled in build-time custom-command ACL and granted only to
local `main`; the existing `core_probe` permission remains. Other windows and
remote origins are denied. Production CSP is unchanged, with no new origins,
plugins, shell/filesystem/HTTP grants, or development exceptions.

## Verification and remaining scope

See the separate [Issue #12 verification record](candidate-preview-verification.md)
for executed checks, test counts, native GUI/IPC evidence, and unverified
Windows/CI work. The [Issue #9 record](desktop-verification.md) is historical.

Clue/Hypothesis/Constraint editors, alternative selection, persistence,
structure canvas, exports, interactive cancellation, streaming/jobs, advanced
ranking, deduplication, recursive transforms, CLI/HTTP APIs, runtime AI, remote
authentication, release packaging, and signing remain separate work. The
unresolved glib investigation in Issue #11 is not addressed here.
