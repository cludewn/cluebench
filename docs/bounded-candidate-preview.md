# Bounded desktop candidate preview

The React interface accepts manual seeds, explicit case/prefix/suffix options,
independent Direct Constraints, Clue-backed Hypotheses, and Alternative Groups.
Original Clue evidence and multiple interpreted effects remain separate from
explicit exclusive selection of interpretations in a group.
The UI-independent Rust Core performs generation, Hard filtering, and Soft
evaluation; the frontend contains no generator or constraint matcher. This is
bounded exploration of configured paths, not password verification or structural
satisfiability analysis.

## Input and transformations

Manual seeds use one line per seed, preserving order and duplicate lines.
LF and CRLF delimit lines. One final line terminator is ignored: `A\n` is one
seed, while `A\n\n` contains an invalid empty second seed. Empty overall input
cannot run. Spaces and case are preserved. A seed containing only spaces is
valid. Each seed must contain 1–64 printable ASCII bytes (`0x20..=0x7E`);
tabs, lone CR, controls, and non-ASCII text are invalid. Fixed messages identify
invalid positions without echoing input.

Case choices appear in the fixed order Original, ASCII Lowercase, ASCII
Uppercase. Only selected choices are sent; Original is not implicit. Prefixes
and suffixes are ordered lists. Explicit **No prefix** / **No suffix** options
represent `""`; an empty list is invalid. Space and the empty string differ.
Affixes retain their case. Lists cannot contain duplicate options.

The Core applies `prefix + case_variant(seed) + suffix`, without recursion or
truncation. Traversal is Seed → Case → Prefix → Suffix, with suffix varying
fastest. Paths longer than the 64-byte candidate domain count as domain skips.
Equal candidate text from different paths remains separate records.

## Clues, interpretations, and effects

A Clue stores original `text` and `source` in separate fields. Neither field
needs to be nonempty. A Clue-backed Hypothesis references exactly one Clue and
stores a separate `interpretation`, which may also be empty. Multiple
Hypotheses can reference the same Clue without merging equal interpretations.
Users explicitly add, edit, or remove Hypotheses, change their originating
Clue while independent, enable or disable independent hypotheses, choose Hard/Soft applicability and confidence,
and add, edit, change type, or remove their constraint effects.

A Clue cannot be deleted while any Hypothesis or group anchor refers to it.
This includes disabled Hypotheses, incomplete drafts, Hypotheses with zero
effects, and empty groups. The user must first remove those references or
explicitly reassign independent Hypotheses to another Clue. There is no cascade
deletion or automatic origin reassignment.
An unreferenced Clue can be deleted; deletion invalidates existing results.

Removing the final effect leaves its Hypothesis in place. A zero-effect
Hypothesis neither filters candidates nor contributes Soft points. This is
shown in the editor. Disabled Independent Hypotheses do not enter the resolved scenario,
but all their effects remain subject to validation; an invalid disabled draft
blocks Preview until corrected or removed. The same validation applies to
unselected grouped Hypotheses.

### Alternative Groups and editing transitions

Each group is anchored to exactly one existing Clue. Only Hypotheses from that
Clue may join it, and each Hypothesis belongs to at most one group. Membership
is stored only as `groupEditorId` on the Hypothesis; groups store no member list.
Groups have stable editor-ID-derived labels and no editable name or description.
Creating a group leaves it empty with explicit `None` selection. Empty groups
remain valid and continue protecting their anchor Clue from deletion.

Independent activation uses `enabled`. Grouped activation uses only the group's
explicit selection: one member or `None`. None activates no members. This is one
selected scenario, not logical OR, automatic exclusivity inference, or a search
over group combinations. All groups' selections are resolved together with
enabled independent Hypotheses and Direct Constraints.

| Edit | Result |
| --- | --- |
| Independent → Grouped | Join the specified same-Clue group, set `enabled=false`, retain its selection. |
| Grouped → Independent | Remove membership and set `enabled=false`; clear the old selection if it selected that member. |
| Move between groups | Clear the source selection only if moving its selected member; retain destination selection and `enabled=false`. |
| Specify the same group again | No-op; retain selection. |
| Select a member or None | Replace only that group's selection explicitly. |
| Delete a Hypothesis | Remove it and clear its group's selection if selected; never select a replacement. |
| Remove the final member | Keep the empty group with None selection. |
| Delete a group | Keep all members and their effects as Independent, `enabled=false`. |
| Change a grouped Hypothesis's Clue | Blocked; explicitly Ungroup first. |

Transitions preserve remaining Hypothesis/effect IDs, interpretation, confidence,
applicability, and constraint values. Disabling when joining or leaving prevents
unexpected Hard activation and never restores an earlier enabled value. Pure
editing helpers return memberships and groups together for a single revision
update. Invalid references or selections are rejected rather than repaired.

Direct Constraints remain a separate optional list. Each row is one independent
manual-origin Hypothesis with one effect, its own enabled state, applicability,
and confidence. The initially empty lists preserve the seed-only workflow.
Equal Clues, Hypotheses, effects, or direct rows stay separate entities.

### Constraint semantics

Both editing workflows share the same seven constraint fields and Core meaning:

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
integer validation; invalid drafts are not trimmed, clamped, truncated, or
silently repaired. Contradictory but individually valid constraints are allowed.

Every constraint effect of every active Hard Hypothesis must match. Soft
mismatches never reject candidates. An active Soft Hypothesis with at least
one effect contributes Low 1, Medium 2, or High 3 points only when **all** its
effects match, and contributes that weight **once per Hypothesis**. A Medium
Hypothesis with two matching effects adds 2 points; one matching effect adds 0.
Different matched Hypotheses contribute independently, including duplicate-valued
ones. Hard confidence is retained but has no evaluation effect. Relative points
are not probabilities and do not reorder candidates. The generator does not
deliver individually Hard-rejected candidates or their reasons to the consumer.

## Unicode evidence and DOM text entry

Clue text, source, and interpretation use `textarea`. The application preserves
accepted Unicode scalar text, LF, TAB, empty values, surrounding whitespace,
combining characters, and Emoji through React state, JSON, Rust `String`, and
the Core. It does not trim, normalize Unicode, change case, or substitute text.
CR is forbidden in final evidence values. Lone UTF-16 surrogates are detected
before `TextEncoder`, avoiding its silent replacement with U+FFFD. Field-byte
overflow rejects the whole operation and retains the previous accepted value;
there is no truncation. Combined Unicode or request-budget overflow retains
the draft, invalidates results, and prevents Preview.

HTML textarea values normalize CR/CRLF to LF, and single-line inputs strip
CR/LF. The raw-text guard distinguishes Unicode evidence from ASCII constraint,
affix, and seed input contracts. It screens source text exposed by
`paste.clipboardData`, `drop.dataTransfer`, and native `beforeinput.data` or
`beforeinput.dataTransfer` before DOM sanitization. Observable CR or CRLF in
evidence rejects the entire operation. Constraint fields reject observable
controls; affixes reject CR/LF. Seed LF/CRLF transfers remain permitted because
they delimit seed lines, while a lone CR is rejected. Numeric fields remain
text inputs with a numeric input-mode hint.

IME composition uses transient input rather than accepted evidence. Preview
is blocked during composition, and the completed value is validated before
acceptance. Rejected operations advance the input revision and invalidate
results. Native `beforeinput` is observed directly because React's
`onBeforeInput` is polyfilled. Cancelable invalid events are prevented. For an
observed noncancelable invalid event, the controlled accepted value is restored;
a transient DOM change may already have occurred before restoration.

These protections apply to source text actually exposed by events. Browser or
OS processing may normalize text before the app receives it, and an event may
expose no usable original text. The application cannot recover lost source
bytes or promise byte-exact clipboard/file preservation. The
[normalization-fix verification record](input-normalization-verification.md)
is historical evidence, not current native-path verification.

## Identity and provenance

One session-local monotonically increasing nonzero `u32` editor allocator
serves Direct Constraints, Clues, Hypotheses, effects, and groups. Every such record has
a unique `editorId`, used for React keys. Editing values, changing effect type,
changing membership or selection, or changing an independent Hypothesis origin preserves IDs. Removal and Clear never reuse
IDs or reset allocation; exhaustion prevents allocation before wraparound.
These identities are distinct from request-local Core IDs and from any future
persistent project identity. Manual Seeds keep their existing request-local IDs.

Rust accepts no client-supplied Core IDs. Starting at 1 for every request, it
allocates in this order:

1. All Manual Seeds.
2. Each Direct Constraint's Hypothesis, then its single effect.
3. All Clues.
4. Each Clue-backed Hypothesis, then all its effects.
5. All Alternative Groups.

For `N` seeds and zero-based direct-row index `i`, existing direct allocation
remains Hypothesis `N + 2*i + 1`, effect `N + 2*i + 2`. After allocating all
Clues, the adapter maps `clueEditorId` to the assigned Core Clue ID and constructs
`HypothesisOrigin::Clue`. Separate editor-to-Core maps resolve Hypothesis and
group references. Independent Hypotheses use `Activation::Independent { enabled }`;
grouped ones use `Activation::Grouped { group_id }`. Groups get independent Core
IDs and `selected: Option<Id>`. Allocating groups last preserves existing
Hypothesis binding order. Bindings are recalculated from the submitted request,
including Seed counts. The maximum Core entity ID is 136: 64 seeds + 32 direct
entities + 8 Clues + 8 Clue-backed Hypotheses + 16 Clue-backed effects + 8 groups.
Inactive and zero-effect Hypotheses still receive IDs and bindings. The complete
project, including unselected effects and empty groups, is validated before
Core resolution constructs the active scenario.

A result explanation follows a matched Core Hypothesis ID through a validated
binding, the submitted Hypothesis, its originating Clue, and that Clue's original
text/source. The frontend retains the submitted request snapshot, including
groups, memberships, selections, Clues, effects, Direct Constraints, Seeds,
options, and limits. Results display the selection and selected interpretation
used for evaluation, including None. They never interpret an old response from mutable editor state. Clue
strings and interpretations are not copied into every candidate response.

## Resource limits

These inclusive Desktop limits are independent of the unchanged Core absolute
limits. The frontend mirrors them for feedback; invalid input is rejected.

| Resource | Preview limit |
| --- | ---: |
| Raw request, before application DTO deserialization | 16,384 bytes |
| Canonical serialized typed request | 16,384 bytes |
| Request ID (`A-Z`, `a-z`, `0-9`, `-`, `_`) | 1–32 ASCII bytes |
| Manual Seeds | 1–64 |
| Individual seed / combined seed text | 1–64 printable ASCII bytes / 4,096 bytes |
| Selected cases, without duplicates | 1–3 |
| Prefixes / suffixes, each list without duplicates | 1–8 each |
| Individual affix | 0–64 printable ASCII bytes |
| Logical generation config | 1,043 bytes |
| Direct Constraints, including disabled | 0–16 |
| Clues / Clue-backed Hypotheses | 0–8 / 0–8 |
| Alternative Groups / memberships | 0–8 / 0–8 |
| Request-local Core entities | 136 |
| Effects per Clue-backed Hypothesis | 0–4 |
| Clue-backed effects / all effects including Direct | 0–16 / 0–32 |
| Individual Clue text / source | 512 / 256 UTF-8 bytes |
| Individual interpretation | 256 UTF-8 bytes |
| Combined Clue text, source, and interpretation | 2,048 UTF-8 bytes |
| Constraint literal / Position allowed characters | 1–64 / 1–95 printable ASCII bytes |
| Combined constraint strings | 3,040 bytes |
| Matched Soft Hypothesis IDs per candidate | 24 |
| Relative points per candidate | 0–72 |
| Attempt allowance | Default 2,048; range 1–4,096 |
| Emission allowance | Default 50; range 1–100 |
| Returned candidate records | 100 |
| Serialized UTF-8 JSON response | 65,536 bytes |
| Active Preview workers / waiting requests | 1 / 0 |

The whole request shares the 16 KiB budget in addition to every individual
bound. All individual maxima cannot necessarily be used simultaneously. JSON
escaping can amplify payloads; logical string limits do not replace serialized
byte checks. The frontend encodes the actual canonical JSON before invoking and
rejects overflow with a fixed message without deleting input. Rust checks Raw
bytes before deserialization, then serializes the typed request through a bounded
counting writer. This canonical check prevents shorter Serde-equivalent
representations from bypassing the budget without allocating another large JSON
buffer solely for counting. A response over 64 KiB fails with a fixed error;
candidates are never removed to fit.

Logical config bytes follow the Core contract: 16 bytes for allowances, one byte
per case, plus all affix bytes. Attempts and emissions are independent. Attempts
bound visited paths even when every path is skipped. Per-attempt matching work
is separately bounded by 32 effects and their string lengths. Static workload
or response-size estimates are not measured latency or serializer evidence.
Actual boundary tests and measured synthetic performance belong in the change's
implementation report and PR. Raw limits do not bound prior WebView/Tauri
transport allocations, Serde metadata, or process-wide memory to the same size.

## Raw IPC, DTOs, and worker lifetime

`candidate_preview` remains the only Preview command. The bridge explicitly
constructs canonical camelCase objects, encodes JSON with `TextEncoder`, checks
its UTF-8 size, and sends `Uint8Array` Raw IPC. Rust accepts
`tauri::ipc::Request`, rejects ordinary JSON bodies, checks the Raw byte count,
and copies only bounded bytes. Inside `spawn_blocking`, it deserializes and
validates the typed DTO, then uses:

`RawProject → validate() → resolve() → ResolvedModel::generate()`.

The request keeps `requestId`, `seeds`, `options` (`caseVariants`, `prefixes`,
`suffixes`), `limits` (`attemptLimit`, `emissionLimit`), and `directConstraints`.
The new arrays have these canonical record shapes:

```ts
type Clue = { editorId: number; text: string; source: string };
type Hypothesis = {
  editorId: number;
  clueEditorId: number;
  interpretation: string;
  enabled: boolean;
  groupEditorId?: number;
  applicability: "hard" | "soft";
  confidence: "low" | "medium" | "high";
  effects: { editorId: number; constraint: Constraint }[];
};
type AlternativeGroup = {
  editorId: number;
  clueEditorId: number;
  selectedHypothesisEditorId: number | null;
};
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

Each Direct Constraint keeps `editorId`, `enabled`, `applicability`, `confidence`,
and one `constraint`. The request also includes `groups: AlternativeGroup[]`.
Omitted `directConstraints`, `clues`, `hypotheses`, or `groups`
default to `[]`; explicit `null` is invalid. Except for `groupEditorId`, all record fields are required,
including `effects`, where `[]` is valid, and `selectedHypothesisEditorId`,
where explicit `null` means None but omission is rejected. Optional
`groupEditorId` must be a nonzero `u32` when present; explicit null is rejected.
Omission means independent activation using `enabled`. Membership requires
`enabled=false` as a compatibility placeholder; `true` is rejected. Core Grouped
activation has no enabled flag. Existing defaults for omitted fields
within `limits` remain. Unknown/duplicate fields, unknown enum variants,
malformed JSON, invalid types/numbers/Unicode, duplicate editor IDs, missing or
wrong-kind Clue/group/Hypothesis references, cross-Clue membership, nonmember
selection, empty-group selection, and invalid inactive records are rejected with fixed
non-sensitive errors. Direct Serde deserialization preserves duplicate-field
detection without passing through `serde_json::Value`. Frontend canonical
objects do not imply that Serde rejects every equivalent positional or enum
representation; the canonical byte check also applies to accepted equivalents.

The response keeps `requestId`, `effectiveLimits`, `candidates`, `stats`,
`potentialPaths`, `stop`, and `constraintBindings`. It adds `hypothesisBindings`.
Both binding arrays contain `{ editorId, hypothesisId }`; the first includes
all Direct Constraints and the second all Clue-backed Hypotheses, including
disabled and zero-effect ones. Candidate records keep text, seed ID, generation
path, selected affixes, relative points, and shared `matchedSoftHypothesisIds`
for both Hypothesis kinds. Values come from actual Core callback records.

The frontend validates exact response fields, serialized size, request identity,
complete unique bindings, exact Core allocation, and no collisions between
Hypothesis kinds. Match IDs must be unique, at most 24, and refer to submitted
active Soft Hypotheses with nonzero effects. Independent activity is
`enabled === true`; grouped activity means the submitted group's selection
equals that Hypothesis's editor ID. This same check determines whether active
Hard effects exist; `enabled=false` does not make a selected member inactive.
Relative points must be an integer
from 0–72 equal to the sum of those Hypotheses' confidence weights, once each.
It considers active Hard effects from both workflows and retains seed/path,
affix, counter, and stop consistency checks. This validates provenance and score
arithmetic without matching candidate text. Inconsistencies produce a fixed
non-sensitive error.

An atomic non-waiting acquisition returns `PREVIEW_BUSY` immediately when the
single slot is occupied. A worker-owned RAII guard holds it until the blocking
closure exits, including unwinding. Dropping an IPC Future or aborting an
awaiting task cannot release a still-running worker's slot. Normal completion,
validation failure, and worker panic permit retry; join failures become fixed
safe errors. There is no queue, streaming, job manager, or interactive
cancellation. Core cancellation always returns false and the consumer continues
until completion or an allowance stops delivery.

## Statistics, lifecycle, and privacy

The adapter preserves actual Core counters and stop reasons:

```text
attemptedPaths = domainSkips + hardRejections + emittedCandidates
candidates.length = emittedCandidates
```

`completed` means all selected paths were visited. `attemptLimit` or
`outputLimit` means the corresponding allowance stopped work. Reaching an
allowance exactly on the final path can still return `completed`. The frontend
does not infer stops from candidate count, or manufacture `cancelled` or
`consumerStopped`. Zero candidates never proves Unsatisfiable.

CandidatePreview owns all drafts, editor allocation, request identity, input
revision, active request, pending state, submitted snapshot, and result/error.
Every meaningful edit, add, remove, origin change, enable/applicability/confidence
change, effect type change, group creation/deletion, membership transition,
member/None selection, rejected raw operation, and Clear advances the
revision and invalidates results. Edits remain allowed while Loading. Only the
still-current request/revision can publish success or error; unmount prevents
late updates. Pending continues until the real Promise settles. Clear empties
input, groups, and display without resetting editor allocation, request sequence,
or active worker ownership, or cancelling Rust. A→B→A selection changes still
advance the revision and reject both obsolete successes and errors. Hypothesis
cards remain flat siblings keyed by editor ID instead of being reparented into
groups, preserving in-progress IME inputs across group edits. Clear remounts
SeedInput; guard cleanup, rejection of unstarted composition-end events, and
stable affix row IDs remain required. No
automatic Preview is added.

Clue evidence, interpretations, constraint literals, Seeds, and candidates are
sensitive and remain in memory and required local IPC. There is no content
logging, telemetry, persistence, external request, URL payload, automatic
clipboard operation, or file export. Inputs disable spellcheck/autocorrection;
output uses React text nodes. Raw Serde/Core/Join errors are never shown. New
sensitive DTOs do not unconditionally derive `Debug`; Core `Debug` output is
also treated as sensitive. Clear reduces references without secure-erasure or
zeroization guarantees.

The existing generated command ACL remains local-`main` only, with no new command
or permission. Other windows and remote origins are denied. Production CSP is
unchanged; no new origin, plugin, shell/filesystem/network grant, or development
exception is introduced. Raw IPC checks do not protect a compromised renderer
or prevent allocations made before Rust receives the body.

## Verification and remaining scope

The [desktop development guide](desktop-development.md) provides reusable
commands and manual procedures. Record change-specific tests, measurements,
manual checks, failures, and unexecuted checks in the implementation report and
PR. [GitHub Actions](https://github.com/cludewn/cluebench/actions) is authoritative
for CI outcomes; inspect the target commit and each job. Native compilation,
actual GUI execution, mock IPC, and real WebView Raw IPC are distinct evidence.

The [Issue #12 report](candidate-preview-verification.md),
[normalization-fix report](input-normalization-verification.md), and
[Issue #9 report](desktop-verification.md) preserve historical local evidence.
Their results and unexecuted checks describe their creation time, not current
CI status. Do not rewrite them or create an Issue-specific report by default.

The complete Project Workspace, cross-Clue grouping, automatic exclusivity,
logical OR, batch scenario comparison, Hypothesis-owned Seed effects, persistence/migration/encryption,
structure analysis UI, canvas, export, interactive cancellation, streaming/jobs,
advanced ranking, deduplication, recursive transforms, CLI/HTTP APIs, stable
external Rust API, runtime AI, remote authentication, release packaging, and
signing remain separate work. Issue #11's glib investigation is unchanged.
