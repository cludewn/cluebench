# Bounded desktop candidate preview

The React interface accepts manual seeds, explicit case/prefix/suffix options,
and up to 16 manually edited direct constraints. Issue
[#14](https://github.com/cludewn/cluebench/issues/14) extends the original
user-driven preview from Issue [#12](https://github.com/cludewn/cluebench/issues/12).
The existing UI-independent Rust Core performs generation, Hard filtering,
and Soft evaluation; the frontend does not implement a generator or evaluator.
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
text, or forbidden controls in single-line constraint fields, rejects the entire
insertion with a fixed message that contains no source text. Affix transfers
reject CR/LF. Constraint literal, allowed-character, and numeric fields screen
observable raw controls before single-line sanitization; numeric fields are
text inputs with a numeric input-mode hint, not coercing number inputs.
Valid Seed LF/CRLF transfers use
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
renderer. The [normalization-fix verification record](input-normalization-verification.md)
preserves historical local React/jsdom and browser evidence, including paths
unverified at its creation; it does not describe current verification status.

Case choices appear in the fixed order Original, ASCII Lowercase, ASCII
Uppercase. Only selected choices are sent; Original is not implicit. Prefixes
and suffixes are ordered lists. Explicit **No prefix** / **No suffix** options
represent `""`; an empty list is invalid. Space and the empty string differ.
Affixes retain their case. Lists cannot contain duplicate options.

The Core applies `prefix + case_variant(seed) + suffix`, without recursion or
truncation. Traversal is Seed → Case → Prefix → Suffix, with suffix varying
fastest. Paths longer than the 64-byte candidate domain are counted as domain
skips. Equal text from different paths remains separate records.

## Direct constraints

The initial condition list is empty, preserving the manual-seed workflow.
Users explicitly add, edit, enable, disable, or remove rows, choose Hard or
Soft applicability, and select Low, Medium, or High confidence. Disabled rows
still count toward the 16-row limit. Each row remains distinct even when its
values duplicate another row.

| Type | Fields and inclusive bounds | Core meaning |
| --- | --- | --- |
| Length | `1 <= min <= max <= 64` | Candidate length is within the range. |
| Position | `index` 1–64; `allowed` 1–95 printable ASCII bytes | The one-based position contains any character in `allowed`, not a substring or regular expression. |
| Contains | `literal` 1–64 printable ASCII bytes | Contains the case-sensitive literal. |
| NotContains | `literal` 1–64 printable ASCII bytes | Does not contain the case-sensitive literal. |
| Prefix | `literal` 1–64 printable ASCII bytes | Starts with the case-sensitive literal. |
| Suffix | `literal` 1–64 printable ASCII bytes | Ends with the case-sensitive literal. |
| CharacterCount | `class`: lowercase, uppercase, digit, space, or symbol; `0 <= min <= max <= 64` | The number of characters in the class is within the range. Space is excluded from symbol. |

Leading/trailing spaces and case are preserved. Repeated characters in
`allowed` are permitted. Numeric drafts remain strings until strict whole-string
integer validation; incomplete or invalid drafts are not trimmed, clamped,
normalized, or replaced with a previous valid value. Every row must be valid
before Preview, including disabled rows. A disabled invalid row must be corrected
or removed. Contradictory but individually valid conditions are permitted.

An enabled Hard condition filters candidates through the Core. Soft mismatches
do not reject candidates; each matched enabled Soft hypothesis contributes
its confidence weight: Low 1, Medium 2, High 3. Duplicate-valued Soft rows can
both match and contribute separately. Hard confidence is retained but has no
evaluation effect. Switching applicability preserves the chosen confidence.
Relative points are not calibrated probabilities, and output is never sorted
by points. Disabled conditions remain validated in the project but do not enter
the resolved active scenario. The generator does not deliver Hard-rejected
candidates or their individual rejection reasons to the preview consumer.

The frontend owns stable, nonzero, monotonically allocated `u32` editor IDs.
Rows use these IDs as React keys; edits and type changes preserve identity.
Removal and Clear do not reset allocation or reuse IDs, and allocation stops
before wraparound. A new duplicate-valued row receives a new ID. These are
session-local editor identities, not a persistent project schema.

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
| Direct constraints, including disabled rows | 0–16 |
| Constraint literal | 1–64 printable ASCII bytes |
| Position allowed characters | 1–95 printable ASCII bytes |
| Combined constraint string payload | At most 1,520 bytes (16 × 95) |
| Matched Soft hypothesis IDs per candidate | At most 16 |
| Relative points per candidate | 0–48 |
| Attempt allowance | Default 2,048; range 1–4,096 |
| Emission allowance | Default 50; range 1–100 |
| Returned candidate records | 100 |
| Serialized UTF-8 JSON response | 64 KiB |
| Active Preview workers / waiting requests | 1 / 0 |

Logical config bytes follow the Core contract: 16 bytes for two allowances,
one byte per case, plus all affix bytes. Attempt and emission allowances are
independent. Attempts bound visited paths even when every path is a domain skip.
The additional matching workload is bounded by 16 conditions and their bounded
strings; an attempt allowance alone does not bound per-attempt evaluation cost.
A response over 64 KiB fails with a fixed error; candidates are never removed to
fit the response budget.

JSON escaping can amplify string sizes, so logical byte limits do not replace
the final encoded request and serialized response checks. Size estimates are
not execution evidence or latency guarantees. DTO tests exercise synthetic
escaping and workload boundaries; change-specific measured results belong in
the implementation report and PR. The Raw limit does not bound prior
WebView/Tauri allocations or Serde allocation metadata to the same size.

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

The canonical camelCase request contains `requestId`, `seeds`, `options`
(`caseVariants`, `prefixes`, `suffixes`), `limits` (`attemptLimit`,
`emissionLimit`), and `directConstraints`. The frontend always constructs this
explicit shape. Each direct condition contains `editorId`, `enabled`,
`applicability` (`hard` or `soft`), `confidence` (`low`, `medium`, or `high`),
and one tagged `constraint` object:

```ts
type Constraint =
  | { type: "length"; min: number; max: number }
  | { type: "position"; index: number; allowed: string }
  | { type: "contains" | "notContains" | "prefix" | "suffix"; literal: string }
  | {
      type: "characterCount";
      class: "lowercase" | "uppercase" | "digit" | "space" | "symbol";
      min: number;
      max: number;
    };
```

Rust independently validates all rows, including disabled ones, before Core
construction. Editor IDs must be nonzero unique `u32` values. Omitted
`directConstraints` defaults to `[]`; explicit `null` is invalid. The existing
limit defaults apply to omitted fields within `limits`. Other fields are
required. Unknown fields/variants, duplicate JSON fields, malformed JSON,
invalid scalar types/ranges, and duplicate generation options are rejected
with fixed non-sensitive error codes. Direct Serde deserialization preserves
duplicate-field rejection; it does not pass through `serde_json::Value` first.
The frontend's canonical object/string representation is not a claim that
derived Serde DTOs reject every equivalent positional or enum representation.

For `N` seeds and zero-based condition index `i`, the adapter assigns seed IDs
`1..=N`, hypothesis ID `N + 2*i + 1`, and constraint-effect ID `N + 2*i + 2`.
The maximum entity ID is 96. Each condition becomes one manual-origin
hypothesis, independent activation carrying its enabled state, its chosen
applicability/confidence, and exactly one constraint effect. Disabled hypotheses
remain present for `RawProject::validate()` before `resolve()` excludes them.
Duplicate-valued rows receive distinct Core IDs. No placeholder clue or
alternative group is created, and the request accepts no client-supplied Core IDs.

The response contains `requestId`, `effectiveLimits`, `candidates`, `stats`,
`potentialPaths`, `stop`, and `constraintBindings`, an array of
`{ editorId, hypothesisId }` for all submitted rows, including disabled ones.
During each borrowed Core callback, the adapter copies candidate text, seed
identity, generation path, selected affixes, relative points, and
`matchedSoftHypothesisIds` into owned DTOs. Seed IDs come from `record.seed()`;
paths come from `record.path()`; affixes come from the referenced config; points
and Soft match IDs come from the actual Core evaluation. Core IDs are assigned
for this request only, never persistent project IDs. React keys identify paths
rather than candidate text alone.

The frontend checks the response shape, request identity, serialized byte bound,
seed/path/case/affix provenance, counter/termination consistency, exact submitted
editor-to-hypothesis bindings, and bounded unique Soft match IDs. Referenced IDs
must resolve through the validated bindings to enabled Soft rows in the
submitted request snapshot. Points must be integers in 0–48 and equal the sum
of those rows' confidence weights: Low 1, Medium 2, High 3. No matched IDs means
zero points, including when there are no active Soft conditions. Distinct
hypotheses with duplicate-valued conditions contribute independently.
This arithmetic consistency check does not evaluate candidate text or prove
that the Core actually matched the candidate correctly. The Rust Core remains
authoritative for constraint matching and Soft evaluation. Each displayed
result retains the request's condition snapshot to explain matches; later
draft edits or reordered rows cannot reinterpret it.

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
either and the UI does not manufacture them. Hard rejections and relative points
can be nonzero. Zero candidates, including an all-Hard-rejected preview, never
means Unsatisfiable. Points are not success probabilities.

## Frontend state and privacy

The interface distinguishes Idle, Invalid, Loading, Success, Empty, Limited,
and Error. Empty and Limited can coexist; the actual termination reason remains
visible. Execution requires a button action. Every input edit increments a
revision and invalidates displayed results. Each run has a non-sensitive request
ID and captures its input revision. Only a still-current request/revision may
publish success or error. Unmount cleanup prevents late state updates. Request
ownership also prevents an old `finally` from clearing another run's Loading.
Adding, editing, removing, enabling/disabling, changing type, switching
applicability, and changing confidence all invalidate results and advance the
revision. Editing during Loading is permitted, without automatic regeneration.
Changing a type resets its fields' transient raw-input guard state.

Clear removes displayed input/results and the condition list, retaining the
editor ID allocation counter, and invalidates their revision. It does
not cancel Rust, resolve an IPC Promise, or release the worker slot. While that
Promise is pending, the UI continues to report work in progress and disables
another run. Busy failures have a fixed, actionable message. No Cancel button
is provided.

Seeds, condition literals, and candidates cross only required local IPC. No
content logging, console logging, telemetry, persistence, URL parameters,
network requests, automatic
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

The [desktop development guide](desktop-development.md) provides reusable
verification commands and manual procedures. Record change-specific local
tests, manual checks, unexecuted checks, and limitations in the relevant PR.
[GitHub Actions](https://github.com/cludewn/cluebench/actions) is authoritative
for CI run outcomes; inspect the target commit and each job directly. Native
compilation, actual GUI execution, and IPC smoke verification are distinct;
a successful build alone does not establish GUI or IPC execution.

The [Issue #12 verification record](candidate-preview-verification.md),
[normalization-fix record](input-normalization-verification.md), and
[Issue #9 record](desktop-verification.md) preserve historical local evidence.
Their results, test counts, and unexecuted checks describe their creation time,
not current CI status. Do not rewrite these records or update this specification
solely to reflect a later CI success. New change-specific results belong in the
PR by default, rather than a new verification report for every issue.

The full Clue/Hypothesis workspace, alternative selection, persistence,
structure canvas, exports, interactive cancellation, streaming/jobs, advanced
ranking, deduplication, recursive transforms, CLI/HTTP APIs, runtime AI, remote
authentication, release packaging, and signing remain separate work. The
unresolved glib investigation in Issue #11 is not addressed here.
