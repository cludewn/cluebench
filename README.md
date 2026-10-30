# Cluebench

Cluebench is an early-stage local-first desktop application for organizing incomplete or uncertain password clues, exploring alternative interpretations, and turning selected hypotheses into constraints and candidate-generation strategies.

Cluebench is intended for CTF challenges, recovery of passwords you own, and explicitly authorized security analysis. It is **not** a password cracking engine, and it will not automate login attempts against remote services.

> **Project status:** The Rust core implements domain validation, hypothesis resolution, candidate evaluation, bounded constraint analysis, and bounded seed-based generation with provenance. The Tauri 2 desktop application provides a bounded manual-seed candidate preview with explicit case and fixed-affix options, provenance, and Core statistics. A synthetic Core connection check remains available. Clue editing, wordlist export, persistence, and the remaining MVP workflow are **not implemented or released yet**. Current platform evidence is recorded in the [Preview verification report](docs/candidate-preview-verification.md); the [earlier shell report](docs/desktop-verification.md) remains historical.

## What makes Cluebench different?

Cluebench focuses on the reasoning workflow before candidate validation:

1. Record clues along with their original source and context.
2. Keep observations separate from interpretations and hypotheses.
3. Preserve alternative hypotheses and activate the ones to explore.
4. Inspect typed constraints, conflicts, and bounded candidate previews.
5. Export candidates for use in authorized, external workflows.

The goal is not to outperform hashcat or John the Ripper in password verification, or to replicate CUPP, Mentalist, or Mr.Pass as feature-for-feature generators.

## Architecture direction

- **Product strategy:** Application-first, library-ready.
- **Core:** Rust, kept independent of the desktop UI.
- **Frontend:** React and TypeScript.
- **Desktop shell:** Tauri 2, connecting the frontend to a small Rust adapter and the existing Core.
- **MVP:** No runtime AI/LLM dependency; later options remain open.
- **CLI / stable public Rust library API / HTTP API:** Outside the MVP scope.

See [product scope](docs/product-scope.md) for the agreed MVP boundaries. The UI-independent [Rust core contract](docs/core-contract.md) documents the implemented semantics and resource limits. The [desktop development guide](docs/desktop-development.md) covers Linux/WSL2 and native Windows prerequisites, architecture, permissions, and verification commands.

With the documented native prerequisites, Node.js version from `apps/desktop/.node-version`, and pnpm version from `apps/desktop/package.json` installed:

```sh
pnpm -C apps/desktop install --frozen-lockfile
pnpm -C apps/desktop tauri dev
```

Enter one printable ASCII manual seed per line, choose case and ordered prefix/suffix options, and explicitly run Preview. Defaults allow 2,048 attempted paths and 50 emitted records; maxima are 4,096 attempts and 100 records. Preview uses local Raw IPC and a single blocking worker; Clear invalidates results without cancelling Rust. See the [preview contract](docs/bounded-candidate-preview.md) for input grammar, resource limits, privacy boundaries, and stale-result handling. `pnpm -C apps/desktop dev` starts only the frontend server, where native operations report that Desktop is unavailable.

The [seed-generation contract](docs/bounded-seed-generation.md) documents case and fixed-affix transformations, deterministic path ordering, independent attempt/output limits, cancellation, and privacy guarantees. Generation is independent of structural analysis; finding no eligible candidates does not prove unsatisfiability.

## Privacy and responsible use

Clues, candidate seeds, and project files may contain sensitive information. Do not post real passwords, secrets, or confidential project material in issues, examples, logs, or pull requests. The application uses local IPC and does not log, persist, or transmit preview content externally. It has no telemetry or automated online authentication attempts. Clearing inputs does not guarantee secure memory erasure.

## Language

English is the primary language for repository artifacts. A [Japanese README](README-ja.md) is also available. The shell currently uses English; full English/Japanese UI support remains planned.

## License

Cluebench is licensed under the [Mozilla Public License, Version 2.0](LICENSE) (`MPL-2.0`). Third-party dependencies, when added, retain their respective licenses.
