# Agent instructions

## Project and status

Cluebench is an early-stage, local-first desktop workbench for reasoning about password clues and candidate exploration. Its product strategy is **application-first, library-ready**. Do not claim unimplemented behavior works.

## Language

- Use clear, concise, idiomatic **English** for committed files, code, identifiers, comments, documentation, commit messages, Issues, and PRs.
- Respond to the developer and provide review reports in **Japanese**, unless instructed otherwise.
- The canonical README is in English; keep `README.ja.md` aligned for material changes.
- Private Japanese design notes may be kept locally under the Git-ignored `.local/` directory. They are **not repository artifacts**: never stage, commit, or publish them, their logs, actual credentials, or private clue data.

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

## GitHub titles and templates

- GitHub operations are normally performed by the developer. AI agents must not modify branches, commits, issues, pull requests, settings, or other state on GitHub without explicit authorization for each specific operation.
- **Commits, issues, and pull requests authored by maintainers or coding agents must use Conventional Commits-style titles**, normally with a scope: `<type>(<scope>): <description>`. Although the scope is syntactically optional, Cluebench permits `<type>: <description>` only when no suitable scope exists, such as for repository-wide initialization (`chore: initialize repository`). Do not invent a scope just to fill the field.
- Use a meaningful type such as `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, or `chore`. Use a concise English description that reflects the change or issue. Example titles:
  - `feat(core): implement candidate evaluation`
  - `fix(ui): correct constraint selection`
  - `docs(github): define issue and pull request conventions`
  - `ci(rust): add core verification workflow`
- Conventional Commits formally defines **commit messages**; applying the same title convention to issues and PRs is a **Cluebench project rule**.
- **Always consult and follow the current repository templates** when preparing issues or pull requests; do not rely on remembered or example templates. Use `.github/ISSUE_TEMPLATE/01-bug-report.yml` for bugs, `02-feature-request.yml` for features, and a blank issue for other work such as design, research, or internal tasks. Do not force unrelated work into the wrong form.
- PR descriptions must follow `.github/pull_request_template.md`, preserve its required structure, link relevant issues, and report only verification that actually occurred (including checks not run and why). Remove optional sections only when they do not apply.
- When a tool does not present the GitHub forms automatically, reproduce the relevant template sections manually.
- Do not reject good-faith external contributions solely for a title-format discrepancy; maintainers can help normalize titles before merging.

## Development workflow

- Prefer small, reviewable work items and focused pull requests.
- Before implementing a disputed technical detail, identify the unresolved decision and present alternatives with tradeoffs.
- Record only tests and validations that actually ran; never report a pass without execution.
- The earlier, unverified Rust PoC is reference material, **not approved source code**. Review it before adopting any implementation.
- Do not add large dependency stacks, node editors, a CLI, external API guarantees, or a full desktop scaffold solely to make the repository look complete.
