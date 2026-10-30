# Initial Rust core contract

The `cluebench-core` crate implements the domain validation, scenario resolution,
and deterministic candidate evaluation defined by [Issue #3](https://github.com/cludewn/cluebench/issues/3),
plus [bounded constraint analysis](bounded-constraint-analysis.md) from
[Issue #5](https://github.com/cludewn/cluebench/issues/5) and
[bounded seed-based generation](bounded-seed-generation.md) from
[Issue #7](https://github.com/cludewn/cluebench/issues/7).
It has no UI or third-party dependencies. Its Rust API is an internal application
boundary, with no external stability guarantee.

## Data flow and ownership

```text
RawProject -> validate() -> ValidatedProject -> resolve() -> ResolvedModel
ResolvedModel -> evaluate(candidate)
ResolvedModel -> analyze(config, cancel)
ResolvedModel -> generate(config, cancel, consumer) -> incremental eligible candidates
```

`RawProject` contains editable input. Validation consumes it, checks all saved
entities, and returns either bounded diagnostics or a `ValidatedProject` with
private state. Validated data has read-only access; changing input requires a new
validation. Resolution borrows the validated data and builds the active scenario.
`ResolvedModel` also has private state and can only be created through resolution.
Candidate evaluation, constraint analysis, and generation are available only on
this model.
No unchecked constructor, mutable validated-data accessor, or public stand-alone
matcher bypasses validation.

The root Cargo workspace contains `crates/cluebench-core` and the desktop adapter
at `apps/desktop/src-tauri`, sharing the root `Cargo.lock`. The Core still has
no third-party dependencies. Core-only verification needs no frontend assets
or desktop system libraries:

```sh
cargo test -p cluebench-core --locked
```

Full workspace checks also compile the Tauri adapter and require the native
dependencies in the [desktop guide](desktop-development.md) and built frontend
assets. From the repository root, run:

```sh
pnpm -C apps/desktop install --frozen-lockfile
pnpm -C apps/desktop build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## Domain relationships

- A `Clue` retains original text and source information. Both may be Unicode.
- A `Hypothesis` retains a separate interpretation, confidence, Hard/Soft
  applicability, activation, origin, and a list of effects. Its origin is either
  manual or a reference to one Clue. A manual-origin hypothesis supports direct
  constraints without requiring a Clue.
- Each effect is either a `ConstraintEffect` or a `Seed`. A hypothesis may contain
  either kind, both kinds, or no effects. Manual seeds also exist at project level.
- An `AlternativeGroup` has an optional selected hypothesis ID. Membership is
  stored only in each hypothesis's activation, so membership cannot disagree
  between two independently editable lists.
- `Id` wraps a `u32`. Zero is invalid. All clues, hypotheses, groups, constraint
  effects, and seeds share one project-wide unique ID namespace. IDs are supplied
  by the caller; validation does not create, rewrite, or merge them.

Independent hypotheses carry `Independent { enabled }`. Grouped hypotheses carry
`Grouped { group_id }`, with no independent enabled flag. This enum makes
simultaneous membership in multiple groups unrepresentable. A grouped hypothesis
is active only when selected by its group. Each group selects one of its members
or `None`; an empty group with no selection is valid. Selection describes the
scenario to evaluate, rather than a logical OR of all member constraints.

## Validation and resolution

Validation checks nonzero globally unique IDs, references to the correct entity
kind, group selection membership, all constraint values, seed text, and resource
limits. It checks inactive and unselected hypotheses too. Broken references and
invalid selections are errors; nothing is silently repaired or truncated.
Effect-free hypotheses and an empty project are valid.

Validation checks each constraint in isolation. For example, two valid Hard
Length constraints requiring exactly 2 and exactly 3 characters can coexist in a
validated project. Validation does not report structural satisfiability.

Resolution includes enabled independent hypotheses and selected grouped
hypotheses. It includes every manual seed and only the effects of active
hypotheses. It does not change the saved input. To explore A, B, or None, change a
raw copy's group selection, validate it, and resolve that version. Source IDs and
payloads are retained, including equal constraint values or seed text from
different origins. Derived effects retain their owner hypothesis; its origin
identifies the original Clue when present. Manual seeds remain identifiable as
manual. Resolution does not deduplicate or rank seeds.

## Candidate and seed domain

Candidates contain 1–64 printable ASCII bytes, `0x20` through `0x7E` inclusive.
Space is supported. ASCII characters and bytes have the same length and position
in this domain. Evaluation does not trim, change case, or normalize Unicode.
An empty, overlong, control-character, or non-ASCII candidate returns a
`CandidateError`, separately from a supported candidate's Hard mismatch.
Checks run in that order: empty input, length over 64 bytes, then unsupported
bytes. An unsupported-byte diagnostic uses a zero-based byte offset; Position
constraints use one-based positions.

Initial seed text and constraint literals also contain 1–64 printable ASCII
bytes. Generation adds only the explicitly configured case and affix options;
see its [contract](bounded-seed-generation.md). Clue
text, source information, and interpretation may use Unicode within the UTF-8
resource limits below.

## Constraint semantics

| Constraint | Meaning and valid input |
| --- | --- |
| `Length` | Inclusive byte/character range, `1 <= min <= max <= 64`. |
| `Position` | One-based index, `1..=64`, and nonempty printable ASCII alternatives. A position beyond the candidate length does not match. All 95 supported characters may be allowed. Order and duplicates do not affect matching. |
| `Contains` | Candidate contains the literal as a contiguous, case-sensitive substring. No occurrence-count condition. |
| `NotContains` | Candidate does not contain the literal as a contiguous, case-sensitive substring. |
| `Prefix` | Candidate starts with the case-sensitive literal. |
| `Suffix` | Candidate ends with the case-sensitive literal. |
| `CharacterCount` | Count all characters in one selected class; inclusive range `0 <= min <= max <= 64`. |

Literals must be nonempty printable ASCII and at most 64 bytes. Matching regions
may overlap: `aba` satisfies both Prefix `ab` and Suffix `ba`, and contains `aba`
as well as `ba`. A literal longer than the candidate cannot be contained, a
prefix, or a suffix; it therefore satisfies `NotContains`.

The five classes are disjoint and exhaustive over supported candidate bytes:

| Class | Characters |
| --- | --- |
| `Lowercase` | `a`–`z` |
| `Uppercase` | `A`–`Z` |
| `Digit` | `0`–`9` |
| `Space` | `0x20` only |
| `Symbol` | All remaining supported printable ASCII bytes |

Space is never counted as Symbol.

## Hard filtering and Soft points

Applicability belongs to the hypothesis. All active Hard constraint effects are
combined with AND. Any mismatch sets `hard_eligible` to false; with no active Hard
constraints it is true. Seed effects never filter candidates.

An active Soft hypothesis earns its weight once if it has at least one constraint
effect and all its constraints match. Multiple matching constraints in one
hypothesis do not multiply its weight. Seed-only and effect-free hypotheses earn
no points. Soft mismatches never affect Hard eligibility, and Soft matches are
still reported when a candidate fails Hard constraints.

| Confidence | Fixed relative points |
| --- | ---: |
| `Low` | 1 |
| `Medium` | 2 |
| `High` | 3 |

Evaluation returns `hard_eligible`, `matched_soft_hypotheses`, and
`relative_points` separately. The maximum of 1,024 hypotheses and weight 3 bounds
the total to 3,072, safely within `u32`. Points express relative preference;
they are not success probabilities. Seeds and candidate text are not logged.

## Resource limits and guarantees

All limits are inclusive and fixed by public constants in the crate.

| Resource | Limit |
| --- | ---: |
| Total entities (including constraint effects and seeds) | 4,096 |
| Hypotheses | 1,024 |
| Hypothesis effects, constraint and seed combined | 2,048 |
| Reference edges | 4,096 |
| Any single stored string, UTF-8 bytes | 4,096 |
| All stored strings combined, UTF-8 bytes | 1,048,576 |
| Returned semantic diagnostics | 64 |
| Candidate, seed, or constraint literal bytes | 64 |

Reference edges count each hypothesis-to-Clue origin, grouped membership, non-None
group selection, and hypothesis-to-effect ownership. Root container ownership
(including project-to-manual-seed ownership) is not counted. The string total
counts all stored string fields, including clue text, source, interpretation,
Position alternatives, literals, and seed text. Duplicate strings count again.
Position alternatives may repeat characters but still obey the generic string
limit. An oversized string is rejected before its contents are inspected.

Validation first performs size preflight with checked arithmetic, before building
ID/reference indexes or copying payloads into derived models. An exceeded limit
returns an explicit error. Semantic errors are bounded by the diagnostic limit;
a truncation indicator reports whether further errors were omitted. Truncation
never converts invalid input into a valid project. Diagnostics identify fields,
codes, and entity IDs without copying sensitive source strings.

Validation moves the caller's `RawProject`; it does not clone its payloads.
Indexes and diagnostics are bounded by these limits. Resolution borrows payloads
and allocates bounded vectors of references for the active hypotheses and effects.
It cannot exceed the already validated entity, effect, edge, or string limits.
Evaluation inspects at most 64 candidate bytes against the bounded active effects
and returns at most 1,024 matched Soft IDs.

These guarantees cover additional work inside validation, resolution, and
evaluation. They do not bound allocations the caller already made while building
`RawProject`, process-wide memory, allocator overhead, or allocator failure. A
loader or IPC boundary must enforce its own limits before constructing raw
input. The [Desktop Preview adapter](bounded-candidate-preview.md) supplies its
own Raw IPC limits; no IPC implementation is included in the Core crate.

## Current exclusions

Structural analysis provides only the bounded deductions and verified witnesses
documented in [bounded constraint analysis](bounded-constraint-analysis.md).
Generation provides bounded, nonrecursive case and affix transformations with
incremental Hard-eligible delivery; its completion does not imply exhaustive
candidate-domain exploration. There is no complete constraint solver, wordlist
export, serialization, migration, persistence, CLI, HTTP API, or general SAT/SMT framework. The crate does not test passwords against hashes
or services. Later MVP work can use this validated core without treating these
missing capabilities as implemented. The separate [desktop shell](desktop-development.md)
adds bounded manual-seed preview and a synthetic Core probe through Tauri IPC;
these UI/IPC responsibilities remain outside the Core.
