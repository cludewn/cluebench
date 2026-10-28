# Bounded constraint analysis

`cluebench-core` implements the limited, sound deductions described in
[Issue #5](https://github.com/cludewn/cluebench/issues/5). This is an internal
application boundary, not a complete solver or a stable external Rust API.
The [core contract](core-contract.md) remains authoritative for validation,
resolution, and candidate matching.

## Entry point, domain, and snapshot

Call `ResolvedModel::analyze` after validation and resolution:

```rust
use cluebench_core::{AnalysisConfig, AnalysisOutcome, RawProject};

let project = RawProject::default().validate().unwrap();
let model = project.resolve();
let result = model.analyze(AnalysisConfig::default(), |_| false);
if let AnalysisOutcome::Satisfiable(witness) = result.outcome() {
    assert!(model.evaluate(witness.as_str()).unwrap().hard_eligible);
}
```

There is no public analyzer accepting arbitrary unchecked constraints. Analysis
uses the complete initial domain: printable ASCII `0x20` through `0x7E`, including
Space, and lengths 1 through 64. Seeds and generation alphabets cannot narrow it.

The result borrows the actual `ResolvedModel`. Its `model()` accessor identifies
the immutable resolved scenario rather than attempting to identify a revision by
project-local effect IDs alone. A new project revision can reuse the same IDs
with different content. The result must remain associated with its borrowed
snapshot; asynchronous job and revision identifiers remain a future GUI concern.

Only resolved constraints whose owner has `Applicability::Hard` are analyzed.
Resolution has already selected enabled independent hypotheses and selected group
members. Analysis does not reinterpret activation. Soft constraints, seeds,
inactive hypotheses, and unselected group members impose no Hard requirements.
Equal constraints remain separate effects with separate origins.

## Result guarantees

`AnalysisOutcome` is an enum, so the analyzer returns one logical outcome:

| Outcome | Guarantee |
| --- | --- |
| `Satisfiable(VerifiedWitness)` | The concrete witness is in the declared domain and has passed every active Hard constraint. |
| `Unsatisfiable(Contradiction)` | The supporting effects and domain establish a sound contradiction over the complete candidate domain. |
| `Unknown(UnknownReason)` | Neither a verified witness nor a completed contradiction proof is available. |

`UnknownReason` distinguishes `InferenceLimit`, `WorkBudgetExhausted`, and
`Cancelled`. Ordinary inconclusiveness is not an interruption, and interruption
is not evidence of a contradiction. A candidate failing validation is never an
UNSAT proof. The result also exposes its domain, work used, and active Hard
constraints. These identities refer to the borrowed snapshot.

## Contradiction rules

Rules run in a fixed order. The first completed proof is sufficient; analysis
does not enumerate all conflicts or minimize an UNSAT core.

| Category | Deduction and supporting effects |
| --- | --- |
| `LengthIntersection` | Intersect Hard Length ranges with domain lengths 1–64. A greatest lower bound exceeding the least upper bound is impossible. The effects establishing those bounds support the proof. |
| `PositionBeyondMaximumLength` | A required one-based Position index greater than the maximum allowed length cannot exist. Evidence includes the Position effect and any Length effect needed to reduce the maximum. Equality is permitted. |
| `PositionIntersection` | Intersect every allowed set for the same Position. An empty intersection leaves no possible character at that required position. Evidence retains the contributing effects, including intersections that become empty only after three or more sets. |
| `ContainsNotContains` | A required literal containing the forbidden literal necessarily forces its occurrence. Evidence includes the Contains and NotContains effects. The direction is significant: Contains `ab` conflicts with NotContains `a`, while NotContains `cab` is not a contradiction by this rule. |
| `IncompatiblePrefixes` | For two required prefixes, the shorter must prefix the longer. If it does not, both effects prove a conflict. |
| `IncompatibleSuffixes` | For two required suffixes, the shorter must suffix the longer. If it does not, both effects prove a conflict. |
| `CharacterClassMinimums` | Retain the greatest minimum for each class, then sum across the five disjoint classes. If the sum exceeds the maximum length, it is impossible. Evidence includes the effects establishing the selected minima and any Length effect needed for the maximum. |

Position alternatives are normalized into a 95-character bit set. Order and
duplicates have no effect on intersection. Every input byte is still processed:
an allowed string may contain up to 4,096 bytes despite the 64-byte candidate
limit. Lowercase, Uppercase, Digit, Space, and Symbol are disjoint; Space is not
Symbol. Repeated minima within one class are never added together.

Comparisons remain case-sensitive. Compatible Prefix values such as `ab` and
`abc` are not conflicts. Prefix and Suffix regions may overlap, so their lengths
are not added to infer a conflict. The domain supplies its own initial bounds;
evidence never invents an effect ID for them.

## Witness construction and matching

After deductions, analysis attempts one deterministic witness: Space repeated to
the intersected minimum length. This guarantees a witness for no-Hard scenarios
and compatible Length-only scenarios, including lengths 1 and 64, provided work
and cancellation allow completion. Other scenarios may also accept that witness.

The analyzer validates the candidate domain and checks every active Hard
constraint through the same internal matcher used by public `evaluate()`.
Matching is charged to the analysis budget and observes cancellation. Soft
scoring is not run during this verification. An interrupted verification cannot
produce `Satisfiable`, and failure of this one witness produces `Unknown` unless
a deduction already established a contradiction.

Public `evaluate()` retains its candidate-error ordering, Hard filtering, Soft
scoring, and seed independence. It uses the shared matcher without analysis
budget enforcement; analysis does not change its public behavior.

## Work budget

`AnalysisConfig::work_budget` is an inclusive `u64` allowance. Its default is
1,000,000 units, not a promise that every valid project finishes within that
allowance. `work_used()` reports reservations successfully made and never exceeds
the supplied budget. A failed reservation performs none of its protected work.
Conservative reservations may charge more units than the path eventually needs.

A work unit represents one bounded structural operation, examined input byte,
byte comparison, or reserved storage/traversal slot. It is not an instruction,
CPU-time, or elapsed-time measurement. The implementation reserves work before
bulk bounded operations and charges incremental scans before each step:

| Work | Accounting |
| --- | --- |
| Hard extraction | Reserve `1 + R` units before allocating capacity for `R` resolved constraint entries, then one unit before inspecting each entry. |
| Preprocessing | One unit per Hard entry; one additional unit before processing each Position allowed byte into the normalized set. |
| Deductions | One unit per visited outer entry and per visited comparison partner. Shared matching helpers charge one unit on entry and one per examined byte or byte pair. Position-state initialization reserves 64 units; class-state initialization reserves five, followed by one per class when summing. |
| Evidence | Reserve `1 + 2*C` before allocation and lazy support traversal, where `C` bounds both inspected source entries and copied supports. `C` is at most 2,048. |
| Witness construction | Reserve `1 + L` before allocating `L` bytes, then one unit before appending each Space. |
| Witness verification | One initial domain-check unit, one per candidate byte, one per Hard entry, and the shared matcher's entry and byte costs. |
| Result preparation | One unit reserved before final witness verification or evidence copying. |

For matching, Length needs only its matcher-entry unit. Position scans allowed
bytes incrementally and stops at the first match. CharacterCount charges each
candidate byte. Prefix and Suffix charge helper entry and each compared byte.
Contains and NotContains use a bounded naive substring matcher: helper entry,
one prefix-helper entry for each attempted offset, and each byte comparison.
There is no hidden uncharged bulk string search or external solver call.

For an allowance `B`, charged work is at most `B`; core execution is bounded by
those reservations plus fixed call/return and cleanup overhead. No candidate
space is enumerated. The only pairwise deduction loops are over already bounded
Hard effects, and every visited partner is charged before inspection. Dynamic
storage consists of prepared capacity for at most 2,048 effects, at most 2,048
support handles for one proof, and one witness of at most 64 bytes, with fixed
Position and class arrays. Source strings are borrowed.

Validation and resolution happen before analysis and retain their existing
resource guarantees; they are not charged again. Active Soft effects affect `R`
and the extraction scan, so they can exhaust a fixed budget sooner, but never
change a completed Hard logical conclusion. Seeds are already in a separate
resolved collection and are not inspected. Inactive and unselected effects are
absent from resolution's active constraints.

Allocator internals, allocation failure, caller callback execution, subsequent
result formatting, accessor traversal, and caller use of exposed data are
outside the work-unit guarantee. In particular, `hard_constraints()` traverses
the borrowed scenario for presentation; it is not part of analysis execution.
Fixed outcome construction and release of the bounded temporary buffers remain
necessary even after budget exhaustion. These exclusions mean the allowance is
a deterministic bound on core algorithm work, not a hard real-time deadline or
a process-wide memory limit.

## Cancellation and determinism

The cancellation argument is a synchronous `FnMut(AnalysisCheckpoint) -> bool`.
Return `true` to stop. A future GUI can close over an `AtomicBool` and load it at
each checkpoint; a test can cancel deterministically from checkpoint metadata.
The callback must return promptly and must not expose sensitive inputs through
its own logging.

Every reservation first invokes the callback with its phase, previously used
work, and proposed next cost. This includes the initial extraction reservation,
before any analysis allocation. Checkpoints cover extraction, preprocessing,
deduction, witness construction, witness verification, and result preparation.
If both cancellation and insufficient allowance are observed at one checkpoint,
`Cancelled` wins; neither condition authorizes the protected operation.

Most scans poll before each byte or effect. The longest bulk core interval
between checkpoints is evidence preparation: one reserved block can examine at
most 2,048 prepared effects and copy at most 2,048 supporting handles. Its largest
reservation is 4,097 units. Prepared-vector allocation reserves at most 2,049
units; fixed array initialization covers at most 64 Position entries. The
allocator and caller callback have no wall-clock bound, so these limits do not
promise cancellation within a fixed number of milliseconds.

`AnalysisPhase::ResultPreparation` reserves the outcome's final administrative
work before the last proof-producing work begins. For witnesses it precedes
domain and complete Hard verification; for contradictions it precedes the
reserved evidence copy. A completed witness means the last required validation
operation has succeeded; a completed proof means its evidence is ready. There
are no further cancellation callbacks or budget reservations after either
boundary. Cancellation arising afterward cannot downgrade the completed result.
If cancellation or exhaustion occurs before completion, provisional witness or
evidence state is discarded and only `Unknown` is returned.

For the same immutable resolved input, budget, and sequence of cancellation
observations, the outcome, witness, evidence order, and work used are
deterministic. The analyzer uses source-order traversal, fixed rule ordering,
fixed character normalization, and a fixed witness. A real concurrently updated
atomic flag can naturally be observed at different checkpoints on different
runs; determinism does not prescribe thread scheduling.

Evidence ordering follows each rule's deterministic construction; it is not
necessarily project order. Class-minimum evidence follows fixed class order,
for example, followed by the supporting Length upper bound when present.

## Evidence and safe diagnostics

`Contradiction` exposes a machine-readable category and supporting
`ResolvedConstraint` values. Their effect IDs are real active Hard effects.
Following `ResolvedConstraint.hypothesis.origin` reaches either `Manual` or a
Clue ID in the validated snapshot. Evidence need not be minimal, but the supplied
effects together with the domain must suffice for the stated deduction.

Witnesses are explicit result data available through `VerifiedWitness::as_str()`.
Diagnostic `Debug` output redacts witness and input contents; it does not format
the borrowed model or the full supporting constraints. Explicitly inspecting
source data can still reveal sensitive content. No logging, telemetry, or
external transmission is added.

## Limits of inference and scope

The rules do not solve arbitrary interactions between constraints. For example,
a Prefix and Suffix that require different characters at a single permitted
position can remain `Unknown`, because this implementation does not reason about
their overlap and its Space witness fails. Likewise, character-class upper-bound
interactions and general substring placement are not solved. Absence of a known
conflict is not a satisfiability proof.

This feature adds no candidate generator, seed transformation, preview, wordlist
export, cardinality calculation, minimum UNSAT core, repair engine, persistence,
GUI, background job manager, CLI, HTTP API, Unicode domain, or SAT/SMT framework.
