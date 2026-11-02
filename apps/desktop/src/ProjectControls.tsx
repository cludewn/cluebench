// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useEffect, useRef, useState } from "react";
import { isProjectDocument } from "./project-document";
import type { ProjectDocumentV1 } from "./project-document";
import { ProjectError, projectErrorFromCode, projectOpen, projectSave, safeProjectError, validPassphrase } from "./project-bridge";
import { useRawTextGuard } from "./use-raw-text-guard";

interface Props {
  document: ProjectDocumentV1;
  isComposing: () => boolean;
  onBusyChange: (busy: boolean, opening: boolean) => void;
  stageLoad: (document: ProjectDocumentV1) => { document: ProjectDocumentV1; apply: () => void };
}
interface PhrasePrompt { confirm: boolean; title: string }
function PhraseInput({ label, value, onChange, onRejected, onCompositionChange }: {
  label: string; value: string; onChange: (value: string) => void; onRejected: (message: string) => void; onCompositionChange: (active: boolean) => void;
}) {
  const guard = useRawTextGuard<HTMLInputElement>("affix", value, onChange, () => onRejected("Passphrases cannot contain CR, LF, or invalid Unicode."), { maxBytes: 1024, onCompositionChange });
  return <label>{label}<input {...guard} type="password" autoComplete="off" autoCapitalize="off" autoCorrect="off" spellCheck={false} /></label>;
}

export default function ProjectControls({ document, isComposing, onBusyChange, stageLoad }: Props) {
  const [baseline, setBaseline] = useState(() => JSON.stringify(document));
  const [binding, setBinding] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [recoveryRequired, setRecoveryRequired] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [prompt, setPrompt] = useState<PhrasePrompt | null>(null);
  const [phrase, setPhrase] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [phraseComposing, setPhraseComposing] = useState(false);
  const [confirmationComposing, setConfirmationComposing] = useState(false);
  const [phraseError, setPhraseError] = useState<string | null>(null);
  const [unsavedPrompt, setUnsavedPrompt] = useState(false);
  const running = useRef(false);
  const alive = useRef(false);
  const phraseResolver = useRef<((value: { passphrase: string; confirmation: string | null } | null) => void) | null>(null);
  const unsavedResolver = useRef<((value: "save" | "discard" | "cancel") => void) | null>(null);
  const pendingFinalization = useRef<string | null>(null);
  const pendingRollback = useRef<string | null>(null);
  const latest = useRef({ document, isComposing, stageLoad });
  latest.current = { document, isComposing, stageLoad };
  const dirty = JSON.stringify(document) !== baseline;
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; phraseResolver.current?.(null); unsavedResolver.current?.("cancel"); };
  }, []);
  function finishPhrase(value: { passphrase: string; confirmation: string | null } | null) {
    const resolve = phraseResolver.current;
    phraseResolver.current = null;
    setPhrase(""); setConfirmation(""); setPhraseError(null); setPrompt(null); setPhraseComposing(false); setConfirmationComposing(false);
    resolve?.(value);
  }
  async function askPhrase(title: string, confirm: boolean) {
    if (!alive.current) return null;
    setPhrase(""); setConfirmation(""); setPhraseError(null); setPrompt({ title, confirm });
    return new Promise<{ passphrase: string; confirmation: string | null } | null>((resolve) => { phraseResolver.current = resolve; });
  }
  async function discard(token: string) {
    try { return (await projectOpen({ action: "discard", token })).status === "discarded"; } catch { return false; }
  }
  async function adopt(token: string): Promise<boolean> {
    // An acknowledgement can be lost after the native tentative swap. Keep
    // enough information to restore the previous binding before another action.
    pendingRollback.current = token;
    try {
      if ((await projectOpen({ action: "adopt", token })).status !== "adopted") throw new ProjectError("INVALID_RESPONSE");
      pendingRollback.current = null; pendingFinalization.current = token;
      return true;
    } catch {
      if (await discard(token)) pendingRollback.current = null;
      else setRecoveryRequired(true);
      return false;
    }
  }
  async function finalize(): Promise<boolean> {
    const token = pendingFinalization.current;
    if (token === null) return true;
    try {
      if ((await projectOpen({ action: "finalize", token })).status !== "finalized") return false;
      pendingFinalization.current = null;
      return true;
    } catch { return false; }
  }
  async function save(as: boolean): Promise<boolean> {
    if (recoveryRequired) { setMessage("Open a project to resolve the previous uncertain file operation before saving again."); return false; }
    if (latest.current.isComposing()) { setMessage("Finish IME composition before saving; the current input was retained."); return false; }
    const snapshot = latest.current.document;
    if (!isProjectDocument(snapshot)) throw new ProjectError("PROJECT_INVALID_DOCUMENT");
    const isNew = as || binding === null;
    let target = binding;
    let adoptionAttempted = false;
    try {
      if (isNew) {
        const selection = await projectSave({ action: "select" });
        if (selection.status === "cancelled") { setMessage("Save cancelled. The current project was retained."); return false; }
        if (selection.status !== "selected") throw new ProjectError("INVALID_RESPONSE");
        target = selection.token;
      }
      if (target === null) throw new ProjectError("INVALID_RESPONSE");
      let secret = await askPhrase(isNew ? "Encrypt new project" : "Authenticate and save project", isNew);
      if (!secret) { setMessage("Save cancelled. The current project was retained."); return false; }
      let outcome;
      try { outcome = await projectSave({ action: "write", token: target, document: snapshot, ...secret }); }
      finally { secret = null; }
      if (!alive.current) return false;
      if (outcome.status === "notCommitted") {
        setMessage(`Not committed. ${safeProjectError(projectErrorFromCode(outcome.code))}`);
        return false;
      }
      if (outcome.status === "outcomeUnknown") {
        setRecoveryRequired(true);
        setMessage("Outcome unknown. The file may have changed. No automatic retry was made; open the file to inspect it before saving again. The current editing project and unsaved changes were retained.");
        return false;
      }
      if (outcome.status !== "saved" && outcome.status !== "committedDurabilityUncertain") throw new ProjectError("INVALID_RESPONSE");
      adoptionAttempted = true;
      if (!(await adopt(target))) {
        setRecoveryRequired(true);
        setMessage(`The project file was already committed${outcome.status === "committedDurabilityUncertain" ? ", with uncertain durability" : ""}, but its association could not be adopted. The current editing project and unsaved changes were retained. ${pendingRollback.current ? "File association rollback could not be confirmed. " : "The previous file association was restored. "}Open the saved file to inspect it before saving again.`);
        return false;
      }
      setBinding(target); setBaseline(JSON.stringify(snapshot));
      const finalized = await finalize();
      setMessage((outcome.status === "saved" ? "Project saved." : "Project committed, but durability is uncertain. The replacement happened; do not assume power-loss durability.") + (finalized ? "" : " File association cleanup could not be confirmed. It will be retried before another file operation."));
      return true;
    } finally {
      if (isNew && target !== null && !adoptionAttempted) await discard(target);
    }
  }
  async function open(): Promise<void> {
    if (dirty) {
      setUnsavedPrompt(true);
      const choice = await new Promise<"save" | "discard" | "cancel">((resolve) => { unsavedResolver.current = resolve; });
      if (choice === "cancel") { setMessage("Open cancelled. The current project was retained."); return; }
      if (choice === "save" && !(await save(false))) return;
    }
    if (pendingFinalization.current !== null && !(await finalize())) { setMessage("The current project was saved, but file association cleanup remains unconfirmed. Open was not started."); return; }
    const selection = await projectOpen({ action: "select" });
    if (selection.status === "cancelled") { setMessage("Open cancelled. The current project was retained."); return; }
    if (selection.status !== "selected") throw new ProjectError("INVALID_RESPONSE");
    let adoptionAttempted = false;
    try {
      let secret = await askPhrase("Open encrypted project", false);
      if (!secret) { setMessage("Open cancelled. The current project was retained."); return; }
      let loaded;
      try { loaded = await projectOpen({ action: "read", token: selection.token, passphrase: secret.passphrase }); }
      finally { secret = null; }
      if (!alive.current) return;
      if (loaded.status !== "opened" || loaded.token !== selection.token) throw new ProjectError("INVALID_RESPONSE");
      let staged;
      try { staged = latest.current.stageLoad(loaded.document); }
      catch { throw new ProjectError("PROJECT_INVALID_DOCUMENT"); }
      adoptionAttempted = true;
      if (!(await adopt(selection.token))) {
        setMessage(pendingRollback.current ? "The project could not be adopted and file association rollback could not be confirmed. The current editing project was retained. Resolve the file association through Open before saving." : "Open failed. The previous editing project, file association and unsaved changes were retained.");
        return;
      }
      staged.apply(); setBinding(selection.token); setBaseline(JSON.stringify(staged.document)); setRecoveryRequired(false);
      const finalized = await finalize();
      setMessage("Project opened. Editing can resume; Preview validates its input separately." + (finalized ? "" : " File association cleanup could not be confirmed. It will be retried before another file operation."));
    } finally { if (!adoptionAttempted) await discard(selection.token); }
  }
  async function perform(action: "save" | "saveAs" | "open") {
    if (running.current) return;
    if (latest.current.isComposing()) {
      setMessage(action === "open" ? "Finish IME composition before opening a project; the current input was retained." : "Finish IME composition before saving; the current input was retained.");
      return;
    }
    running.current = true; setBusy(true); onBusyChange(true, action === "open"); setMessage(null);
    try {
      if (pendingRollback.current !== null) {
        if (!(await discard(pendingRollback.current))) { setMessage("File association rollback could not be confirmed. The current editing project was retained; Open can retry recovery."); return; }
        pendingRollback.current = null;
      }
      if (pendingFinalization.current !== null && !(await finalize())) { setMessage("File association cleanup remains unconfirmed. No new file operation was started."); return; }
      if (action === "open") await open(); else await save(action === "saveAs");
    }
    catch (error: unknown) { if (alive.current) setMessage(safeProjectError(error)); }
    finally { running.current = false; if (alive.current) { setBusy(false); onBusyChange(false, false); } }
  }
  function chooseUnsaved(choice: "save" | "discard" | "cancel") {
    const resolve = unsavedResolver.current; unsavedResolver.current = null; setUnsavedPrompt(false); resolve?.(choice);
  }
  return <section aria-label="Project files">
    <div className="actions"><button type="button" disabled={busy || recoveryRequired} onClick={() => void perform("save")}>Save</button><button type="button" disabled={busy || recoveryRequired} onClick={() => void perform("saveAs")}>Save As</button><button type="button" disabled={busy} onClick={() => void perform("open")}>Open</button></div>
    <p role="status" aria-label="Project status" aria-busy={busy}>{dirty ? "Unsaved changes" : "No unsaved changes"}. {binding ? "Project file associated." : "No project file associated."}{message && <> {message}</>}</p>
    <p className="hint">Projects use passphrase encryption in standard age format. A lost passphrase cannot be recovered. Weak passphrases remain vulnerable to offline guessing. No passphrase is cached between operations; clearing JavaScript references does not guarantee memory erasure.</p>
    {unsavedPrompt && <div role="dialog" aria-modal="true" aria-label="Unsaved project changes"><p>Save the current project before opening another file?</p>{recoveryRequired && <p>Saving is unavailable until a successful Open resolves the previous uncertain operation. Discard keeps this editing project until another file opens successfully.</p>}<div className="actions"><button type="button" disabled={recoveryRequired} onClick={() => chooseUnsaved("save")}>Save changes</button><button type="button" onClick={() => chooseUnsaved("discard")}>Discard changes</button><button type="button" onClick={() => chooseUnsaved("cancel")}>Cancel Open</button></div></div>}
    {prompt && <div role="dialog" aria-modal="true" aria-label={prompt.title}><form onSubmit={(event) => {
      event.preventDefault();
      if (phraseComposing || confirmationComposing) { setPhraseError("Finish passphrase composition before continuing."); return; }
      if (!validPassphrase(phrase)) { setPhraseError(new ProjectError("PROJECT_INVALID_PASSPHRASE").message); return; }
      if (prompt.confirm && phrase !== confirmation) { setPhraseError("The passphrases do not match."); return; }
      finishPhrase({ passphrase: phrase, confirmation: prompt.confirm ? confirmation : null });
    }}><h3>{prompt.title}</h3><PhraseInput label="Passphrase" value={phrase} onChange={setPhrase} onRejected={setPhraseError} onCompositionChange={setPhraseComposing} />
      {prompt.confirm && <PhraseInput label="Confirm passphrase" value={confirmation} onChange={setConfirmation} onRejected={setPhraseError} onCompositionChange={setConfirmationComposing} />}
      {phraseError && <p role="alert">{phraseError}</p>}<div className="actions"><button type="submit">Continue</button><button type="button" onClick={() => finishPhrase(null)}>Cancel</button></div></form></div>}
  </section>;
}

