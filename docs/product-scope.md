# Product scope

**Status:** Product-level decisions confirmed on 2026-10-26; implementation details remain subject to technical validation. This document is not a claim that features already exist.

## Purpose

Cluebench is a local-first visual workbench that helps users preserve and interpret incomplete password clues, work with alternative hypotheses and constraints, and explore candidate-generation strategies. The desktop application is the primary product; the Rust core must remain UI-independent. A stable external API is not part of the MVP.

## Intended users

- CTF participants working with challenge-provided hints.
- Individuals recovering passwords for data they own.
- Professionals performing explicitly authorized security analysis.

No automated remote authentication attempts, credential stuffing, CAPTCHA bypass, or collection of third-party personal data without authorization.

## MVP scenarios

| ID | User task | Required capability |
| --- | --- | --- |
| S1 | Combine clues about length, positions, substrings, and character classes | Structured constraints and password-structure visualization |
| S2 | Keep and switch between interpretations of the same clue | Clue provenance, alternative hypotheses, and active effects |
| S3 | Explore uncertain memories without prematurely eliminating candidates | Hard filtering vs. soft relative evaluation |
| S4 | Resolve incompatible conditions | Sound conflict detection with `Unknown` when not proven |
| S5 | Stop and resume work, then use candidates externally | Versioned local project persistence, bounded preview, plaintext export |
| S6 | Handle large search spaces and sensitive hints safely | Work limits, cancellation, no telemetry, explicit privacy handling |

## MVP feature boundary

The MVP includes manual candidate seeds, limited case changes and fixed prefix/suffix additions, hard-constraint filtering, relative soft scoring, bounded previews, and plaintext wordlist export. Clues must be editable and retain source information; the user manually records interpretations. A simple constraint may be added directly without a mandatory multi-step clue workflow.

The implemented Rust Core supports Length, Position alternatives, Contains, NotContains, Prefix, Suffix, and CharacterCount constraints over printable ASCII candidates. The Desktop supports these seven types as manually edited direct conditions in its [bounded preview](bounded-candidate-preview.md). Each condition maps to an independent manual-origin hypothesis, with Hard/Soft applicability, Low/Medium/High confidence, and an enabled state. This is a first constraint workflow, not the full Clue/Hypothesis workspace. The [Core contract](core-contract.md) defines the domain semantics; the Preview contract defines the tighter Desktop bounds.

**Outside the MVP:** automatic natural-language interpretation, runtime AI/LLMs, node-based generation pipelines, CUPP-style profile generation, advanced statistical ranking, a full hypothesis graph, all hashcat/JtR export formats, CLI, stable public Rust API, and HTTP API.

## Analysis guarantees

Do not confuse structural satisfiability with candidates found by one generation strategy. An analyzer must distinguish proven satisfiable, proven unsatisfiable, and unknown results, within clearly declared bounds. Soft assertions must not silently become hard filters. Displayed scores must not be presented as calibrated success probabilities.

## Evaluation

**Technical MVP completion:** the supported scenarios and acceptance tests pass, including constraint correctness, provenance and selection, persistence, bounded export, cancellation, and privacy-sensitive error handling.

**Product-value challenge target:** for representative complex clue workflows, aim for a **30% reduction in median completion time** versus a manual notes workflow, under a documented comparison protocol. This target is not yet measured, is not an asserted performance claim, and is separate from technical MVP completion. Error rates and the ability to explain candidate provenance also matter.

## Open technical decisions

Tauri 2 is adopted for the initial [desktop shell](desktop-development.md), with a small Rust adapter to the UI-independent Core. The Desktop provides a [bounded manual-seed candidate preview with direct constraints](bounded-candidate-preview.md) and a synthetic Core probe. Direct conditions support Hard filtering and Soft relative scoring with matched-condition provenance. Full clue/source and hypothesis editing, alternative selection, structural analysis UI, persistence, export, and interactive cancellation remain incomplete. Exact persistence and encryption policy, platform packaging, broader product UI layout, further solver guarantees, process-wide memory limits, and release process remain open.

The [Preview verification report](candidate-preview-verification.md) preserves historical local evidence from its creation, not current platform or CI status. [GitHub Actions](https://github.com/cludewn/cluebench/actions) is authoritative for CI run outcomes; inspect the target commit and each job. Change-specific local tests, manual checks, unexecuted checks, and limitations belong in the relevant PR. Current specifications and reusable verification procedures remain in the technical documents.
