# Agent instructions

## Project and status

Cluebench is an early-stage, local-first desktop workbench for reasoning about password clues and candidate exploration. Its product strategy is **application-first, library-ready**. Do not claim unimplemented behavior works.

## Language

- Use clear, concise, idiomatic **English** for committed files, code, identifiers, comments, documentation, commit messages, Issues, and PRs.
- Respond to the developer and provide review reports in **Japanese**, unless instructed otherwise.
- The canonical README is in English; keep `README.ja.md` aligned for material changes.
- Private Japanese design notes are deliberately **outside this repository**. Do not copy or commit them, logs from them, actual credentials, or private clue data.

## Confirmed product decisions

- Product name: **Cluebench**.
- Tagline: **A visual workbench for password clues and candidate exploration.**
- License: **MPL-2.0**. Use appropriate source-file license headers for new code and respect dependency licenses.
- Rust core, React + TypeScript frontend. Tauri 2 is a candidate, **not yet approved**.
- The MVP must not depend on runtime AI/LLMs. This restriction is not a permanent product-wide ban on future AI features.
- Manual candidate seeds, limited transformations, and bounded candidate preview **are MVP requirements**.
- CLI, HTTP API, and stable external Rust API are **out of MVP scope**.
- The 30% time-reduction target is an **unverified product evaluation goal**, not a technical acceptance criterion or achieved result.

## Architecture and correctness

- Keep clue evidence, human interpretation, hypotheses, typed constraints, and generation strategies conceptually distinct. Validate the actual Rust model against documented scenarios rather than imposing a premature generic solver framework.
- Prefer a reusable UI-independent Rust core, but do not commit to external API stability.
- Do not duplicate authoritative constraint evaluation in TypeScript.
- Never turn a soft hypothesis into a hard rejection condition.
- Do not label an unsupported/incomplete satisfiability check as `Unsatisfiable` or `Satisfiable`. Distinguish solver analysis from candidates found by a chosen seed strategy.
- Do not generate unbounded dictionaries, block the UI on large computations, or log passwords/seeds.
- No remote login automation, credential stuffing, CAPTCHA bypass, or nonconsensual personal-data collection.

## Development workflow

- Prefer small, reviewable work items and focused pull requests.
- Before implementing a disputed technical detail, identify the unresolved decision and present alternatives with tradeoffs.
- Record only tests and validations that actually ran; never report a pass without execution.
- The earlier, unverified Rust PoC is reference material, **not approved source code**. Review it before adopting any implementation.
- Do not add large dependency stacks, node editors, a CLI, external API guarantees, or a full desktop scaffold solely to make the repository look complete.
