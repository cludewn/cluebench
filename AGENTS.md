# Agent instructions

## Project and status

Cluebench is an early-stage, local-first desktop workbench for reasoning about password clues and candidate exploration. Its product strategy is **application-first, library-ready**. Do not claim unimplemented behavior works.

## Language

- Use clear, concise, idiomatic **English** for committed files, code, identifiers, comments, documentation, commit messages, Issues, and PRs.
- Respond to the developer and provide review reports in **Japanese**, unless instructed otherwise.
- The canonical README is in English; keep `README-ja.md` aligned for material changes.
- Private Japanese design notes may be kept locally under the Git-ignored `.local/` directory. They are **not repository artifacts**: never stage, commit, or publish them, their logs, actual credentials, or private clue data.

## Confirmed product decisions

- Product name: **Cluebench**.
- Tagline: **A visual workbench for password clues and candidate exploration.**
- License: **MPL-2.0**. Use appropriate source-file license headers for new code and respect dependency licenses.
- Rust core, React + TypeScript frontend, and Tauri 2 desktop adapter. Keep the core UI-independent; the [Issue #9 desktop verification report](docs/desktop-verification.md) preserves historical local environment evidence, not current CI status. Check [GitHub Actions](https://github.com/cludewn/cluebench/actions) for CI run outcomes.
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

## Git and GitHub operations

- The developer normally runs `git add`, `git commit`, and `git push` in WSL bash, and creates and merges pull requests through GitHub Web.
- Implementation or file-editing requests do not authorize staging, committing, or pushing. Do not automatically include routine commits in Codex implementation tasks.
- AI agents, including Codex, may stage or commit only when there is a reasonable need to delegate the operation (for example, complex partial staging) and the developer explicitly authorizes that specific operation. Permission to commit does not authorize pushing.
- Amending commits, rewriting history, pushing (including force pushes), or modifying any GitHub state (including branches, commits, issues, pull requests, and settings) each requires explicit authorization for the specific operation. This also applies to PR creation and merging.
- Keep Git command guidance proportionate to the task; do not add large sets of unnecessary inspection commands for simple operations.

## GitHub titles and templates

- **Commits, issues, and pull requests authored by maintainers or coding agents must use Conventional Commits-style titles**, normally with a scope: `<type>(<scope>): <description>`. Although the scope is syntactically optional, Cluebench permits `<type>: <description>` only when no suitable scope exists, such as for repository-wide initialization (`chore: initialize repository`). Do not invent a scope just to fill the field.
- Use a meaningful type such as `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, or `chore`. Use a concise English description that reflects the change or issue. Example titles:
  - `feat(core): implement candidate evaluation`
  - `fix(ui): correct constraint selection`
  - `docs(github): define issue and pull request conventions`
  - `ci(rust): add core verification workflow`
- Conventional Commits formally defines **commit messages**; applying the same title convention to issues and PRs is a **Cluebench project rule**.
- **Always consult and follow the current repository templates** when preparing issues or pull requests; do not rely on remembered or example templates. Choose issue types by purpose, not by author. Do not force unrelated work into the wrong template.
  - Bug report (`.github/ISSUE_TEMPLATE/01-bug-report.yml`): report a defect in existing functionality.
  - Feature request (`.github/ISSUE_TEMPLATE/02-feature-request.yml`): propose a new feature or improvement.
  - Development task (`.github/ISSUE_TEMPLATE/03-development-task.md`): track a development task with a defined implementation scope and acceptance criteria, such as implementing a new feature, refactoring, or maintenance.
  - Blank issue: use a free-form issue for research, design discussions, or similar work.
- PR descriptions must follow `.github/pull_request_template.md`, preserve its required structure, link relevant issues, and report only verification that actually occurred (including checks not run and why). Remove optional sections only when they do not apply.
- When a tool does not present the GitHub forms automatically, reproduce the relevant template sections manually.
- Do not reject good-faith external contributions solely for a title-format discrepancy; maintainers can help normalize titles before merging.

## Development workflow

- Prefer small, reviewable work items and focused pull requests.
- Before implementing a disputed technical detail, identify the unresolved decision and present alternatives with tradeoffs.
- The earlier, unverified Rust PoC is reference material, **not approved source code**. Review it before adopting any implementation.
- Do not add large dependency stacks, node editors, a CLI, external API guarantees, or a full desktop scaffold solely to make the repository look complete.

## Verification

- Run the checks needed for the change with the available development environment. Do not add commands or verification steps merely to make the report look more thorough.
- Report `PASS` only for checks actually executed successfully. Distinguish static review findings from executed compilation, tests, linting, and other checks.
- Disclose failed checks and unresolved issues. For important checks that could not be run, state what was not run, why, and what remains unverified.
- Never include real passwords, credentials, or sensitive clue data in test data or reports.

## Verification evidence management

- Technical documents describe current specifications, design, implemented behavior, constraints, and reusable verification procedures. Update them when the implementation or those procedures change.
- PR descriptions should record change-specific verification: local automated tests, manual checks, checks not run and why, and known limitations. Include the same evidence in the final report.
- GitHub Actions is authoritative for CI run outcomes. Inspect the target commit and each job directly; do not infer success from workflow configuration or another commit's results.
- Do not create a separate verification Markdown report for every issue by default. Record change-specific results in the final report and PR; keep reusable specifications and procedures in `docs/`. Create an exceptional long-lived verification record only with an explicit purpose explaining why it needs to be retained.
- Historical reports are immutable records of what was actually verified at that time. Preserve their original PASS/FAIL results, unexecuted checks, and other recorded state; do not rewrite them after later verification.
- Current specifications must not describe historical verification as current status. Label links to past reports as historical evidence and direct readers to GitHub Actions for CI outcomes.
- Do not update repository documents solely to reflect a later CI success or create an additional commit just to copy CI results into Markdown.
- Distinguish native compilation, actual GUI execution, and IPC smoke verification. Evidence for one does not establish the others.

## Final reports

- At task completion, report in Japanese according to the language policy above, with enough detail for a third party to understand the change without rereading every diff. Explain what significant changes do and their effects; file lists or vague claims such as "implemented" or "all PASS" are insufficient.
- Normally include the verdict (such as `PASS` or `CHANGES REQUIRED`), each changed file's main changes and effects, added/updated/removed dependencies (explicitly state when none), and the main commands run.
- Report successful checks and their results, failed checks and their impact, and important checks not run with reasons and remaining unknowns, following the verification rules above.
- Explain material design decisions, constraints, remaining work, known risks, and the precise Git state, distinguishing uncommitted changes, local commits, and pushed commits.
- Scale the report to the task. For small changes, combine nonapplicable items and avoid an excessive fixed template.
