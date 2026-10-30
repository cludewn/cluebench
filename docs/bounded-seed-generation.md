# Bounded seed-based candidate generation

`cluebench-core` implements the deterministic generation contract from
[Issue #7](https://github.com/cludewn/cluebench/issues/7). This synchronous Core
API powers the [Desktop candidate preview](bounded-candidate-preview.md);
wordlist export remains unimplemented. Neither UI nor export belongs to the Core. The API is internal application infrastructure;
external Rust API stability is not promised.

The [core contract](core-contract.md) defines validation, resolution, the
candidate domain, and Hard/Soft evaluation. Generation preserves those semantics
and is independent of [bounded constraint analysis](bounded-constraint-analysis.md).

## Entry point and ownership

```rust
use cluebench_core::{GenerationConfig, GenerationStop, Id, RawProject, Seed};
use std::ops::ControlFlow;

let project = RawProject {
    manual_seeds: vec![Seed { id: Id(1), text: "Synthetic1!".into() }],
    ..RawProject::default()
}.validate().unwrap();
let model = project.resolve();
let config = GenerationConfig::default();
let result = model.generate(
    &config,
    |_| false,
    |candidate| {
        assert!(candidate.evaluation().hard_eligible);
        // A preview or export consumer can process candidate.text() here.
        ControlFlow::Continue(())
    },
).unwrap();
assert_eq!(result.reason(), GenerationStop::Completed);
assert_eq!(result.stats().emitted_candidates, 1);
```

Only `ResolvedModel::generate` starts generation; raw input cannot bypass
validation or resolution. `GenerationConfig` contains caller-owned ordered
lists and run allowances. Generation borrows the immutable model and config and
does not mutate source entities or options. The returned `GenerationResult`
borrows both through `model()` and `config()`, keeping its reason, counters, and
path count associated with the actual scenario and options rather than IDs that
could be reused in a different revision.

The consumer receives one `GeneratedCandidate` at a time. Its text and evaluation
borrow the current path's temporary data and cannot outlive the delivery.
Consumers can explicitly copy data they need; caller-side accumulation is outside
the generator's memory guarantee. The consumer returns `ControlFlow::Continue(())`
or `ControlFlow::Break(())` to request an immediate stop. There is no implicit
collection, output file, or background job.

## Seed scope and provenance

Generation uses `ResolvedModel::seeds()` unchanged: project-level manual seeds
first, followed by active hypothesis seed effects in existing source order.
Both Hard and Soft hypotheses can supply seeds. Resolution has already included
enabled independent hypotheses and the selected member of each alternative
group, and excluded disabled independent and unselected grouped hypotheses.
Generation does not implement selection again.

Clue text, interpretation notes, and constraint literals are not seed sources.
An empty seed set is valid. Equal seed text with different IDs remains separate,
and the original source data is preserved.

Each delivered record exposes:

- `text()`: the generated printable ASCII candidate.
- `seed()`: the existing `ResolvedSeed`, retaining the Seed ID and its owner.
  `hypothesis == None` identifies a project-level manual seed. A hypothesis-owned
  seed retains the Hypothesis ID and `HypothesisOrigin`, distinguishing a manual
  hypothesis from a Clue-origin hypothesis and retaining the Clue ID when present.
- `path()`: the seed index, case index and variant, prefix index, and suffix index.
  Affix indices refer to `config()`; they are not persistent project entity IDs.
- `evaluation()`: Hard eligibility, matched Soft Hypothesis IDs, and relative
  points. The seed's owner and a matching Soft hypothesis are independent roles.
- `model()` and `config()`: the borrowed inputs for this record.
- `stats()`: classified counters including the current delivery.

No provenance graph or aggregation across equal candidate strings is built.

## Transformations and ordering

Every path applies one fixed, nonrecursive stage:

```text
prefix + case_variant(seed_text) + suffix
```

`Original` preserves all seed bytes. `Lowercase` changes only ASCII `A-Z` to
`a-z`; `Uppercase` changes only `a-z` to `A-Z`. Digits, Space, and symbols are
unchanged. Fixed prefix and suffix bytes are never case-converted. For example,
Lowercase with seed `aB`, prefix `PRE-`, and suffix `-X` yields `PRE-ab-X`.
Generated text never becomes a seed for another transformation.

The lists `case_variants`, `prefixes`, and `suffixes` must be ordered and
nonempty. An empty affix means no addition and must be included explicitly when
desired. Duplicate case variants, duplicate prefixes within their list, and
duplicate suffixes within their list are invalid. The same value can appear
once in each affix list. Each affix is either empty or printable ASCII
`0x20..=0x7E`, with at most 64 bytes.

Traversal uses this nesting, with suffix varying fastest:

1. Resolved seed order.
2. Case option order.
3. Prefix option order.
4. Suffix option order.

There is no sorting, Soft-score ranking, or implicit deduplication. A digit-only
seed selected with all three case variants produces three equal strings with
different paths. Identical inputs, config, and cancellation observations produce
identical records, ordering, counters, and termination.

## Domain and evaluation

Every started path checks its transformed length with checked arithmetic before
building a candidate. Supported candidates contain 1–64 printable ASCII bytes,
including Space. Out-of-domain paths are domain skips, never truncated, evaluated,
or delivered. Validated seeds and affixes already ensure the character domain;
the assembled candidate is checked before evaluation as well.

In-domain candidates are passed to the existing `ResolvedModel::evaluate()`.
A Hard mismatch counts as a Hard rejection. A Hard-eligible candidate is delivered
with the unchanged evaluation, including its matched Soft IDs and relative
points. Soft mismatches never reject a candidate; scores do not change order.
No constraint matcher is duplicated.

Generation does not call analysis or require `Satisfiable`. An analysis result of
`Unknown` does not prohibit generation. Zero delivered candidates, whether after
completion or partial termination, never establishes structural `Unsatisfiable`.
`Completed` exhausts only the selected seed/option paths, not the full candidate
domain.

## Configuration validation and absolute bounds

All values are validated before traversal or callbacks, even with no seeds.
Errors identify a code, field, and optional option index; they do not copy option
or source strings. Lists are checked for size before scanning their contents.
Invalid config returns `GenerationConfigError` rather than a generation outcome.

Public inclusive limits in `cluebench_core::limits` are:

| Constant | Limit |
| --- | ---: |
| `MAX_CASE_OPTIONS` | 3 |
| `MAX_PREFIX_OPTIONS` | 128 |
| `MAX_SUFFIX_OPTIONS` | 128 |
| `MAX_AFFIX_BYTES` | 64 |
| `MAX_GENERATION_CONFIG_BYTES` | 8,192 |
| `MAX_GENERATION_ATTEMPTS` | 1,000,000 |
| `MAX_GENERATION_EMISSIONS` | 1,000,000 |

The config-byte measure counts one logical byte per case option, every affix's
UTF-8 byte length, and 16 bytes for the two `u64` allowances. It excludes Rust
container metadata and caller-chosen spare capacities; list counts independently
bound the metadata relevant to traversal. The default config selects only
`Original`, an explicit empty prefix and suffix, 100,000 attempts, and 10,000
emissions.

Attempt and emission allowances are independent inclusive `u64` values. Both
accept zero. A potential path count above either allowance is valid; per-run
allowances limit traversal, whereas configuration limits bound its input.
Path multiplication and resource addition use checked arithmetic. The result's
`potential_paths()` counts the configured Cartesian product, including duplicate,
overlong, and Hard-rejected paths. It is not candidate-space cardinality or a
count of eligible or distinct strings.

At most 4,096 resolved seeds and 3 × 128 × 128 option combinations give
201,326,592 potential paths. Even such a configuration does not allocate a path
table and cannot start more paths than its attempt allowance.

## Termination and accounting

Before starting a path, generation applies this exact precedence:

| Priority | Condition | Reason |
| --- | --- | --- |
| 1 | Every configured path has been classified | `Completed` |
| 2 | Cancellation callback returns true | `Cancelled` |
| 3 | Attempt allowance has been reached | `AttemptLimit` |
| 4 | Emission allowance has been reached | `OutputLimit` |

Exhaustion wins even at exact allowances or after cancellation is requested
during the final path. Empty seeds with valid config complete with zero counters,
including when both allowances are zero. With remaining paths, cancellation wins
over either limit, and the attempt limit wins over the emission limit. With
nonempty seeds, zero attempts yields `AttemptLimit` unless cancellation wins;
positive attempts with zero emissions yields `OutputLimit`.

An explicit consumer stop is handled inside delivery: that delivery counts as
emitted, and `ConsumerStopped` is committed immediately. This also applies on the
final path and when cancellation is requested concurrently. No completion,
cancellation, or allowance check can replace that reason. Every terminal result
is final; there are no callbacks after it has been committed.

`GenerationStats` has four `u64` fields:

| Counter | Update boundary |
| --- | --- |
| `attempted_paths` | Increment once when a path starts, after all stop checks. |
| `domain_skips` | Increment when that path fails the candidate domain. |
| `hard_rejections` | Increment when its in-domain evaluation fails Hard eligibility. |
| `emitted_candidates` | Increment before invoking the consumer for an eligible record. |

Each started path reaches exactly one of the final three categories. At every
externally observable boundary, including cancellation callbacks, delivery
callbacks, and returned results:

```text
attempted_paths = domain_skips + hard_rejections + emitted_candidates
```

No callback observes a started but unclassified path. A consumer that stops sees
its own delivery already included in `stats()`. After an allowance prevents the
next path, generation does not evaluate more paths to look for another eligible
output. Non-`Completed` reasons identify partial or consumer-stopped traversal;
the final-path consumer-stop case remains `ConsumerStopped` even though that
delivery happens to exhaust the selected paths.

## Cancellation and bounded work

Cancellation is a synchronous `FnMut(GenerationStats) -> bool`, called only
between paths after the exhaustion check and before allowance checks. It can
load a caller-owned atomic flag; tests can decide from classified counters.
Once a path starts, it finishes as a domain skip, Hard rejection, or delivery
before cancellation is observed again. Consumer stop takes effect at that
delivery without waiting for another checkpoint.

Config validation precedes the first cancellation checkpoint. It checks at most
3 case entries and 256 affix entries, rejects oversized strings before inspecting
contents, scans bounded accepted payloads, and compares duplicate options through
bounded pairwise checks within each list. It creates no growing index or set.

The longest per-path interval without a cancellation check contains length and
domain checks, construction of one at-most-64-byte candidate, one complete
existing Hard/Soft evaluation, provenance/result preparation, and delivery. The
existing project limits permit at most 1,024 active hypotheses and 2,048 effects
in total. A Position matcher scans at most 4,096 allowed bytes per effect;
substring matching examines at most 64 offsets with at most 64 byte comparisons
per offset. Other matchers examine bounded fields or at most 64 candidate bytes.
At most 1,024 matched Soft IDs are prepared. These bounds make a complete Core
path finite without adding cancellation inside `evaluate()`.

These are algorithmic bounds, not a wall-clock or millisecond cancellation
promise. Consumer and cancellation callback execution time, allocator internals,
and caller-side processing are outside that guarantee. Callbacks should return
promptly if responsive cancellation matters.

## Peak additional memory and privacy

Generation borrows source seeds, hypotheses, constraints, and config strings.
It retains one candidate buffer of at most 64 logical bytes, one evaluation with
at most 1,024 matched Soft IDs, and a fixed amount of path, counter, and result
state. Validation uses bounded traversal and pairwise duplicate checks without
copying affixes or building a set. Thus peak additional logical storage is
`64 + 1,024 * size_of::<Id>()` bytes plus constant state, independent of potential
path count, attempt allowance, and delivered output count. With the current
four-byte `Id`, that payload is at most 4,160 bytes. Vector/string spare capacity,
allocator metadata and behavior, allocation failure, preexisting project/config
allocations, and caller-owned copies are excluded from this logical storage
bound; it is not a process-wide heap-byte limit.

New generation `Debug` implementations redact candidate and affix text and avoid
formatting borrowed models, seeds, hypotheses, Clues, or constraint literals.
Config diagnostics retain no sensitive strings. Explicit text, config, and source
accessors intentionally expose data to the caller; independently formatting
existing source domain types can still reveal it. Their general `Debug` redesign
is outside this change.

The generator adds no logging, telemetry, external requests, implicit file writes,
runtime AI dependency, or remote authentication. Tests use synthetic data.

## Exclusions

The Core generation feature adds no brute-force enumeration, recursive pipeline, custom
substitution, profile generation, statistical ranking, global deduplication,
candidate preview UI, wordlist/file output, persistence or migration, search-space
cardinality, complete SAT/SMT solver, GUI, async jobs, CLI, HTTP API, stable external
Rust API, remote login automation, credential stuffing, spraying, or hash cracking.
Desktop bounded preview now uses this same Core generator through a separate
adapter; plaintext export remains future MVP work.
