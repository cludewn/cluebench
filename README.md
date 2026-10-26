# Cluebench

Cluebench is a planned local-first desktop application for organizing incomplete or uncertain password clues, exploring alternative interpretations, and turning selected hypotheses into constraints and candidate-generation strategies.

Cluebench is intended for CTF challenges, recovery of passwords you own, and explicitly authorized security analysis. It is **not** a password cracking engine, and it will not automate login attempts against remote services.

> **Project status:** Early design and technical validation. The desktop application and described MVP features are **not implemented or released yet**.

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
- **Desktop shell:** Tauri 2 is under evaluation, not yet a final choice.
- **MVP:** No runtime AI/LLM dependency; later options remain open.
- **CLI / stable public Rust library API / HTTP API:** Outside the MVP scope.

See [product scope](docs/product-scope.md) for the agreed MVP boundaries. No runnable application has been committed at this stage.

## Privacy and responsible use

Clues, candidate seeds, and project files may contain sensitive information. Do not post real passwords, secrets, or confidential project material in issues, examples, logs, or pull requests. The planned application is local-first and does not include telemetry or automated online authentication attempts.

## Language

English is the primary language for repository artifacts. A [Japanese README](README.ja.md) is also available. The user interface is planned to support English and Japanese.

## License

Cluebench is licensed under the [Mozilla Public License, Version 2.0](LICENSE) (`MPL-2.0`). Third-party dependencies, when added, retain their respective licenses.
