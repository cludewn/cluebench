# Encrypted project files

Cluebench's Desktop adapter supports explicit **Save**, **Save As**, and **Open**
for the current bounded authoring project. There is no autosave, recent-file
list, automatic backup, plaintext fallback, cloud synchronization, or candidate
result persistence. File dialogs, validation, cryptography, and filesystem
operations run in Rust. The UI-independent Core has no persistence dependency.

## Authoring document and identity

The authenticated age payload is a UTF-8 JSON `ProjectDocumentV1` object with
`format: "cluebench-project"` and `schemaVersion: 1`. Only version 1 is supported.
Missing, invalid, older, and newer versions are rejected; no migration is
attempted. All fields are required, including explicit `null` for None selection.
Unknown and duplicate fields, positional record arrays, malformed JSON,
ill-formed Unicode, and inconsistent references are rejected.

The document contains these ordered authoring collections and settings:

| Field | Contents |
| --- | --- |
| `clues` | `id`, original `text`, and separate `source` |
| `hypotheses` | `id`, `origin: {type: "clue", clueId}`, `interpretation`, `applicability`, `confidence`, `activation`, and ordered `effects` |
| `hypotheses[].effects` | `id` and a typed `constraint` draft |
| `groups` | `id`, `anchorClueId`, and `selectedHypothesisId` or `null` |
| `directConstraints` | `id`, `enabled`, `applicability`, `confidence`, and `constraint` |
| `manualSeeds` | One `text` string, including empty lines, TAB, and trailing LF |
| `options` | Ordered `caseVariants`, `prefixes`, and `suffixes` |
| `options.prefixes` / `options.suffixes` | `includeEmpty` and ordered `{id, value}` rows, including empty and duplicate rows |
| `limits` | Raw string `attemptLimit` and `emissionLimit` |

`applicability` is `hard` or `soft`; `confidence` is `low`, `medium`, or `high`.
Activation is exactly `{type: "independent", enabled: boolean}` or
`{type: "grouped", groupId}`. Membership exists only on Hypotheses; groups do
not store a second member list. Origins and anchors must identify Clues;
membership must identify a same-Clue group. Selection must identify one member
of that group or be `null`. Empty groups and zero-effect Hypotheses are valid.
Case variants are `original`, `asciiLowercase`, and `asciiUppercase`, without
duplicates; an empty selection and the user's selection order are preserved.

Constraint drafts retain numeric strings rather than converting them:

| `type` | Additional required fields |
| --- | --- |
| `length` | String `min`, `max` |
| `position` | String `index`, `allowed` |
| `contains`, `notContains`, `prefix`, `suffix` | String `literal` |
| `characterCount` | `class` (`lowercase`, `uppercase`, `digit`, `space`, `symbol`), string `min`, `max` |

Persisted IDs are canonical lowercase UUID v4, assigned when each entity is
created. They are unique across Clues, Hypotheses, effects, groups, Direct
Constraints, and both sides' affix rows. Editing, reordering, saving, Save As,
opening, and resaving preserve UUIDs. The document contains no session editor
IDs, request-local Core IDs, request IDs, React/DOM state, active compositions,
worker state, or generated results.

Session editor IDs are separately allocated, nonzero, monotonically increasing
`u32` values across all entity kinds, including affix rows. Deletion, Clear, and
Open never reuse them. Open verifies capacity for the complete project, builds
its mapping and drafts in staging, then applies one replacement. Exhaustion
rejects the complete load. Preview continues to allocate its existing Core IDs
independently for each submitted request.

## Persistence validity and Preview eligibility

Saving preserves the accepted authoring state without trimming, normalizing,
clamping, repairing, filling, or dropping values. Empty evidence, empty literals,
inverted ranges, numeric drafts such as `""`, `"0002"`, `"-"`, and invalid number
text, inactive or unselected incomplete effects, empty case selection, non-ASCII
drafts, and empty or duplicate affix rows can round-trip. Seed text stays one
string. `includeEmpty` and every affix row remain separate authoring values.

Persistence validates structure, field domains, identities, references, and its
own budgets. It does **not** require `RawProject::validate()`, Preview eligibility,
the Preview evidence aggregate, or the Preview request budget. After Open,
editing can resume even when Preview rejects the draft. Only an eligible Preview
input is converted to the existing bounded request and passed through Core
validation, resolution, and generation. Constraint evaluation remains in Core.

Evidence and Seeds require Unicode scalar text without CR. Affix rows additionally
reject LF; constraint and numeric inputs reject ASCII controls and DEL. No file
CR/CRLF normalization occurs. Existing observable paste/drop/beforeinput guards
and accepted-value restoration remain in the UI. Browser normalization that
occurs before an observable event cannot recover the original source bytes.

## Independent resource budgets

| Resource | Maximum |
| --- | --- |
| Encrypted file | 384 KiB, with bounded limit-plus-one reading |
| Decrypted JSON | 256 KiB |
| Decoded JSON strings, including field names | 96 KiB total |
| JSON depth / value nodes / objects | 16 / 4,096 / 256 |
| Decoded field name | 64 UTF-8 bytes |
| Entities across every kind | 72 |
| Clues / Hypotheses / groups | 8 each |
| Direct Constraints | 16 |
| Clue-backed effects | 16 total, 4 per Hypothesis |
| Affix rows | 8 per side, independently of `includeEmpty` |
| Numeric draft / individual affix row | 256 UTF-8 bytes |
| Seed textarea | 16 KiB |
| Clue text / source / interpretation | 512 / 256 / 256 UTF-8 bytes |
| Constraint literal / allowed | 0–64 / 0–95 UTF-8 bytes |
| age header / scrypt stanzas | 4 KiB / exactly 1 |
| Passphrase | 1–1,024 UTF-8 bytes |
| Crypto workers / waiting queue | 1 / 0 |

Reads, serialization, decryption, and parsing are bounded separately. Parsing
counts structure and decoded strings before typed deserialization, without a
generic `serde_json::Value` tree that could hide duplicate keys. Both writes and
reads perform complete structural and reference validation. Individual maxima
are not a promise that every maximum can be combined. A persistence-valid
project can exceed Preview's 16 KiB request or 2,048-byte evidence budgets; those
are enforced only when running Preview. Eight affix rows plus `includeEmpty`
can be saved even though nine resulting options exceed Preview's eight options.

## Encryption and passphrases

Files use the standard **binary age** passphrase format through the Rust `age`
library. The initial write/read profile is scrypt `logN=18`, `r=8`, `p=1`, with
an explicit maximum read work factor of 18. Other profiles, armor, other
recipient types, and multiple scrypt stanzas are rejected before expensive KDF
work. The scrypt scratch requirement is approximately 256 MiB plus library and
process overhead. Existing-file authentication and replacement encryption run
sequentially under one admitted worker, with no waiting queue.

Initial Save and Save As require a passphrase and matching confirmation. Open
asks once. Regular Save asks again and fully authenticates the associated file
before encrypting its replacement. Empty phrases, CR/LF, ill-formed Unicode,
and values over 1,024 UTF-8 bytes are rejected. Spaces and Unicode are neither
trimmed nor normalized. There is no session phrase cache. UI fields reset after
submission or cancellation; Rust uses secret types and zeroizing plaintext
buffers. Complete erasure of JavaScript, transport, library, or OS memory is not
guaranteed. Successful decryption requires authenticated EOF; encryption must
finish before its bytes reach the filesystem.

**A lost passphrase cannot be recovered.** Encryption does not prevent offline
guessing of a weak passphrase. Wrong phrases, authentication failures, and
damaged ciphertext produce fixed nonsecret errors. Paths, contents, phrases,
keys, and raw parser/OS diagnostics are not included in application errors,
URLs, or new logs. There is no plaintext project format or fallback.

## Filesystem boundary and commit semantics

The initial backend admits **Linux local ext4** and **Windows local NTFS** with
the required safe handle operations. Other filesystems and OSes are rejected.
WSL-mounted Windows filesystems are not Linux ext4. Network/UNC locations,
symlinks or Windows reparse components, alternate data streams, and nonregular
targets are rejected. Linux additionally requires a parent owned by the effective
user or root. Group/other write is rejected unless that trusted owner has set
the sticky bit; the POSIX ACL effective write mask is included in the mode
check. Admission is rechecked before creation, install, and cleanup. Lack of the
required rename API is a failure, never a
request to fall back to weaker I/O. Native Windows execution and GUI/IME/IPC
verification remain separate from Linux tests or cross-target type checking.

The parent directory is pinned by a handle. A random same-directory temporary
file is created exclusively, receives ciphertext only, is flushed and synced,
and is installed in one native rename. New destinations use no-replace;
regular Save replaces only the current associated object after comparing
identity and bounded ciphertext. Checks do not rely only on timestamps or size.
Linux uses handle-relative `openat` and `renameat2`, then syncs the parent
directory. Windows uses handle-relative `NtCreateFile` and `FileRenameInfoEx`;
it does not fall back to `ReplaceFileW`. Linux checks temp identity immediately before its name-based rename and
cleanup. The parent admission rule prevents substitution by another ordinary
user; a malicious process with the same UID or root privileges is outside this
guarantee. Windows renames and cleans up the owned temp handle itself. There is no copy/delete replacement, in-place truncate,
delete-before-rename, plaintext temp, or automatic backup.

| Outcome | Meaning and UI behavior |
| --- | --- |
| `Cancelled` | No project replacement or baseline change |
| `NotCommitted` | Failure before commit; old target is retained, edits stay unsaved |
| `Saved` | Rename committed and Linux parent sync succeeded; snapshot becomes the baseline |
| `CommittedDurabilityUncertain` | Rename committed but directory durability was not established; acknowledge the committed snapshot and display the uncertainty |
| `OutcomeUnknown` | Commit outcome cannot be established; preserve edits and baseline, block automatic retry, and require reopening/inspection |

Windows reports committed writes as `CommittedDurabilityUncertain`: the file is
synced, but this implementation does not establish directory-entry power-loss
durability. A Linux post-rename directory-sync failure has that same outcome.
No automatic rollback occurs after commit. A process interruption can leave a
ciphertext temp; it is never automatically promoted or used as a recovery copy.
Cooperative external-change detection is not a complete compare-and-swap against
a noncooperating writer, nor an unconditional atomicity/power-loss guarantee.

## Save, Open, dirty state, and lifecycle

First Save selects a new destination. Regular Save uses the current association.
Save As selects a new destination and refuses existing files, including the
associated path. The UI sends only session-scoped opaque tokens; arbitrary paths
are not command arguments. The registry contains at most the current binding
and one pending binding. A new selection invalidates the old pending token.
Open must validate and stage the complete UI state before explicitly adopting
its token. Adoption temporarily retains the previous binding in the second
registry slot, so a failed acknowledgment can roll back. After the UI applies
the staged state, an idempotent finalize releases the previous binding. An
unresolved adoption blocks other operations; abandoned tokens cannot later
activate themselves. If IPC prevents reconciliation, saving stays blocked until
Open recovers the association rather than asserting an unchanged native binding.

Dirty state compares the full authoring document to the last successful Save or
Open baseline, including UUIDs, raw strings, order, selections, trailing LF,
leading numeric zeros, and empty rows. It is independent of Preview feedback.
Save uses an immutable snapshot: edits made during its worker stay dirty after
that snapshot is saved. Clear is an edit of the current project and retains its
file association. Clear does not mean New Project.

Open offers Save, Discard, or Cancel for unsaved changes. Failed or cancelled
Save stops Open. Discard leaves the old project present until the new document
has fully passed file, crypto, parse, structural/reference, and UI capacity
checks. Failed loads preserve the old project, baseline, and dirty state; an
unacknowledged adoption rolls back the retained old binding. Unavailable IPC
during rollback requires explicit association recovery. File operations are serialized; Open locks editing and Clear. Saving
during authoring IME composition is rejected with input retained. Numeric
generation-limit fields also participate in composition tracking.

Open replaces only the form subtree using an epoch and rejects callbacks from
older epochs. It increments the Preview revision and invalidates earlier
results/errors. The worker owner, active request, pending state, request sequence,
and editor allocator remain mounted. A pending Preview continues holding its
single-flight slot until its Promise settles, even after Open or Clear. Neither
operation cancels Rust or permits an overlapping Preview. Affix rows use stable
editor identities, and membership changes preserve the existing flat card keys.

## Permissions, residual risks, and verification

Only `project_open` and `project_save` are added to the custom command manifest
and individual allow permissions. The capability remains `local: true` and
`windows: ["main"]`. Native dialogs are called through Rust; no JS filesystem,
generic filesystem, shell, or network permission is granted. The dialog crate
depends transitively on `tauri-plugin-fs` for shared path types, but that plugin
is not initialized and receives no permissions. Production CSP is unchanged.

Encryption protects project payload confidentiality and integrity under age's
assumptions. It does not hide filenames, file sizes, native dialog history,
filesystem journal activity, swap, process dumps, or OS backup/sync copies. It
does not protect an already compromised running session. No telemetry or remote
communication is introduced. Use synthetic data for verification.

Run the reusable checks in [Desktop development](desktop-development.md).
The Rust tests distinguish real filesystem operations, injected I/O failures,
process termination, and mock-runtime command/ACL checks; frontend tests mock
IPC and DOM behavior. Simulated write failure is not a real full-disk test;
process kill is not a power-loss test. GitHub Actions runs the persistence Rust
tests on both Linux and Windows, while native GUI/IME/real WebView IPC require
separate manual verification. A workflow definition or cross-compile does not
establish a successful Windows run.

The complete multi-project Workspace, autosave, cloud sync, candidate-result
persistence, export, structure canvas, automatic schema migration, interactive
cancellation, and remaining MVP work are outside this feature.

The platform mechanisms follow the [Linux renameat2 contract](https://man7.org/linux/man-pages/man2/rename.2.html),
[Windows relative NtCreateFile API](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile),
and [FILE_RENAME_INFO API](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info).
These APIs do not establish a universal power-loss or concurrent-writer CAS guarantee.
