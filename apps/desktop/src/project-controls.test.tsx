// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import CandidatePreview from "./CandidatePreview";
import { projectOpen, projectSave, ProjectError } from "./project-bridge";
import type { ProjectResponse } from "./project-bridge";
import { authoringDocument, defaultAuthoringInput } from "./project-document";
import type { ProjectDocumentV1 } from "./project-document";
import { runCandidatePreview, PreviewError } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";

vi.mock("./project-bridge", async () => ({ ...await vi.importActual<typeof import("./project-bridge")>("./project-bridge"), projectOpen: vi.fn(), projectSave: vi.fn() }));
vi.mock("./preview-bridge", async () => ({ ...await vi.importActual<typeof import("./preview-bridge")>("./preview-bridge"), runCandidatePreview: vi.fn() }));
const seedName = "Manual seeds — one per line";
const field = (label: string) => screen.getByLabelText(label, { exact: true }) as HTMLInputElement | HTMLTextAreaElement;
const change = (label: string, value: string) => fireEvent.change(field(label), { target: { value } });
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const status = () => screen.getByRole("status", { name: "Project status" });
const idle = () => waitFor(() => expect(status().getAttribute("aria-busy")).toBe("false"));
const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
let loaded: ProjectDocumentV1;
let selection: number;
let outcome: ProjectResponse;
const writes = () => vi.mocked(projectSave).mock.calls.map(([action]) => action).filter((action) => action.action === "write");
const saved = () => writes().at(-1)!.document;
function response(request: PreviewRequest): PreviewResponse {
  return { requestId: request.requestId, effectiveLimits: request.limits, candidates: [], stats: { attemptedPaths: 1, domainSkips: 1, hardRejections: 0, emittedCandidates: 0 }, potentialPaths: 1, stop: "completed", constraintBindings: [], hypothesisBindings: [] };
}
async function phrase(confirm: boolean) {
  await screen.findByLabelText("Passphrase", { exact: true }); change("Passphrase", " synthetic phrase 🧩 ");
  if (confirm) change("Confirm passphrase", " synthetic phrase 🧩 ");
  click("Continue");
}
async function save(name = "Save", confirm = true) { click(name); await phrase(confirm); await idle(); }
async function open(discard = true) {
  click("Open");
  await waitFor(() => expect(Boolean(screen.queryByRole("dialog", { name: "Unsaved project changes" }) || screen.queryByLabelText("Passphrase", { exact: true }))).toBe(true));
  if (screen.queryByRole("dialog", { name: "Unsaved project changes" })) click(discard ? "Discard changes" : "Save changes");
  await phrase(false); await idle();
}
beforeEach(() => {
  vi.clearAllMocks(); selection = 0; loaded = authoringDocument(defaultAuthoringInput(), new Map()); loaded.manualSeeds.text = "opened"; outcome = { status: "saved" };
  vi.mocked(projectOpen).mockImplementation(async (action) => {
    if (action.action === "select") return { status: "selected", token: `token_${++selection}` };
    if (action.action === "read") return { status: "opened", token: action.token, document: structuredClone(loaded) };
    return { status: action.action === "adopt" ? "adopted" : action.action === "finalize" ? "finalized" : "discarded" };
  });
  vi.mocked(projectSave).mockImplementation(async (action) => action.action === "select" ? { status: "selected", token: `token_${++selection}` } : outcome);
  vi.mocked(runCandidatePreview).mockImplementation(async (request) => response(request));
});

describe("Open and authoring IME regressions (frontend mocks)", () => {
  // Adapt the two independent review reproductions to allow safe refusal of
  // Open. If refusal regresses, the original pending-dialog event order runs.
  it("retains confirmed input when composition ends before Cancel Open", async () => {
    render(<CandidatePreview />); change(seedName, "accepted"); const seed = field(seedName);
    fireEvent.compositionStart(seed); change(seedName, "confirmed input"); click("Open");
    const confirmation = screen.queryByRole("dialog", { name: "Unsaved project changes" });
    fireEvent.compositionEnd(seed);
    if (confirmation) { click("Cancel Open"); await idle(); }
    else expect(status().textContent).toContain("Finish IME composition before opening");
    expect(field(seedName)).toBe(seed); expect(seed.value).toBe("confirmed input");
    await save(); expect(saved().manualSeeds.text).toBe("confirmed input");
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0].seeds).toEqual(["confirmed input"]);
  });
  it("permits Save and Preview after a composition-end event and a cancelled Open phrase prompt", async () => {
    render(<CandidatePreview />); change(seedName, "accepted"); const seed = field(seedName);
    fireEvent.compositionStart(seed); change(seedName, "confirmed input"); click("Open");
    if (screen.queryByRole("dialog", { name: "Unsaved project changes" })) {
      click("Discard changes"); await screen.findByLabelText("Passphrase", { exact: true });
      fireEvent.compositionEnd(seed); click("Cancel"); await idle();
    } else {
      expect(projectOpen).not.toHaveBeenCalled();
      expect(status().textContent).toContain("Finish IME composition before opening");
      fireEvent.compositionEnd(seed);
    }
    expect(seed.value).toBe("confirmed input");
    await save(); expect(saved().manualSeeds.text).toBe("confirmed input");
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
  });
  it.each([seedName, "Prefix 1", "Condition 1 minimum", "Clue 1 original text", "Attempt limit", "Emission limit"])("refuses Open before locking a composing %s and preserves its accepted value", async (name) => {
    render(<CandidatePreview />); change(seedName, "accepted");
    if (name === "Prefix 1") click("Add prefix");
    if (name === "Condition 1 minimum") click("Add condition");
    if (name === "Clue 1 original text") click("Add Clue");
    const value = name.includes("limit") || name === "Condition 1 minimum" ? "0002" : "confirmed input";
    const input = field(name); fireEvent.compositionStart(input); change(name, value); click("Open");
    expect(projectOpen).not.toHaveBeenCalled(); expect(projectSave).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull(); expect(input.closest("fieldset.project-authoring")!.hasAttribute("disabled")).toBe(false);
    expect(field(name)).toBe(input); expect(input.value).toBe(value); expect(status().getAttribute("aria-busy")).toBe("false");
    fireEvent.compositionEnd(input); await save();
    const document = saved();
    const persisted = name === seedName ? document.manualSeeds.text : name === "Prefix 1" ? document.options.prefixes.rows[0].value :
      name === "Condition 1 minimum" ? (document.directConstraints[0].constraint as { min: string }).min : name === "Clue 1 original text" ? document.clues[0].text :
        name === "Attempt limit" ? document.limits.attemptLimit : document.limits.emissionLimit;
    expect(persisted).toBe(value);
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
  });
  it.each(["Open", "Save", "Preview candidates"])("refuses %s when compositionStart and the operation share a React batch", async (operation) => {
    render(<CandidatePreview />); change(seedName, "accepted"); const seed = field(seedName);
    act(() => { fireEvent.compositionStart(seed); change(seedName, "confirmed input"); click(operation); });
    expect(projectOpen).not.toHaveBeenCalled(); expect(projectSave).not.toHaveBeenCalled(); expect(runCandidatePreview).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull(); expect(seed.value).toBe("confirmed input");
    fireEvent.compositionEnd(seed); await save(); expect(saved().manualSeeds.text).toBe("confirmed input");
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
  });
  it("waits for a composition completed in the same batch to publish its authoring state before Open", async () => {
    render(<CandidatePreview />); const seed = field(seedName);
    act(() => { fireEvent.compositionStart(seed); change(seedName, "confirmed input"); fireEvent.compositionEnd(seed); click("Open"); });
    expect(projectOpen).not.toHaveBeenCalled(); expect(screen.queryByRole("dialog")).toBeNull(); expect(seed.value).toBe("confirmed input");
    click("Open"); click("Cancel Open"); await idle();
    await save(); expect(saved().manualSeeds.text).toBe("confirmed input");
  });
  it.each(["confirmation cancel", "phrase cancel", "authentication failure"])("keeps completed composition, baseline and binding after a delayed compositionEnd during %s", async (ending) => {
    render(<CandidatePreview />); change(seedName, "accepted"); await save(); const seed = field(seedName);
    fireEvent.compositionStart(seed); change(seedName, "confirmed input"); click("Open");
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.compositionEnd(seed); click("Open");
    if (ending === "confirmation cancel") { fireEvent.compositionEnd(seed); click("Cancel Open"); }
    else {
      click("Discard changes"); await screen.findByLabelText("Passphrase", { exact: true }); fireEvent.compositionEnd(seed);
      if (ending === "phrase cancel") click("Cancel");
      else {
        vi.mocked(projectOpen).mockImplementationOnce(async () => { throw new ProjectError("PROJECT_AUTHENTICATION_FAILED"); });
        await phrase(false);
      }
    }
    await idle(); expect(field(seedName)).toBe(seed); expect(seed.value).toBe("confirmed input");
    expect(status().textContent).toContain("Unsaved changes"); expect(status().textContent).toContain("Project file associated");
    change(seedName, "accepted"); expect(status().textContent).toContain("No unsaved changes"); change(seedName, "confirmed input");
    await save("Save", false); expect(saved().manualSeeds.text).toBe("confirmed input"); expect(writes().at(-1)!.token).toBe("token_1");
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0].seeds).toEqual(["confirmed input"]);
  });
});
afterEach(cleanup);

describe("Explicit encrypted project file operations (frontend mocks)", () => {
  it("saves incomplete raw fields, seeds, empty variants and affixes, then restores them exactly", async () => {
    render(<CandidatePreview />); change(seedName, "\nseed\t試験\n\n"); fireEvent.click(screen.getByLabelText("Original"));
    click("Add condition"); change("Condition 1 minimum", "0002"); change("Condition 1 maximum", "-");
    click("Add prefix"); click("Add prefix"); change("Prefix 2", "é"); click("Add prefix"); change("Prefix 3", "é");
    change("Attempt limit", ""); change("Emission limit", "invalid");
    await save(); const first = structuredClone(saved());
    expect(first.manualSeeds.text).toBe("\nseed\t試験\n\n"); expect(first.options.caseVariants).toEqual([]);
    expect(first.options.prefixes.rows.map((row) => row.value)).toEqual(["", "é", "é"]);
    expect(first.directConstraints[0].constraint).toEqual({ type: "length", min: "0002", max: "-" });
    expect(status().textContent).toContain("No unsaved changes");
    loaded = first; change(seedName, "changed"); await open();
    expect(field(seedName).value).toBe(first.manualSeeds.text); expect(field("Condition 1 minimum").value).toBe("0002");
    expect(field("Prefix 1").value).toBe(""); expect(field("Prefix 3").value).toBe("é");
    await save("Save", false); expect(saved()).toEqual(first);
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });
  it("keeps UUIDs stable across editing, Save As and regular Save, including zero-effect hypotheses and groups", async () => {
    render(<CandidatePreview />); click("Add Clue"); click("Add hypothesis for clue 1"); click("Add group for clue 1");
    await save(); const first = saved();
    expect(first.hypotheses[0].effects).toEqual([]);
    change("Clue 1 original text", "synthetic"); await save("Save As");
    expect(saved().clues[0].id).toBe(first.clues[0].id); expect(saved().groups[0].id).toBe(first.groups[0].id);
    expect(saved().hypotheses[0].id).toBe(first.hypotheses[0].id);
    await save("Save", false); expect(writes().at(-1)!.confirmation).toBeNull();
    expect(screen.queryByLabelText("Passphrase", { exact: true })).toBeNull();
  });
  it("preserves empty constraint literals and allowed characters on load", async () => {
    loaded.directConstraints = [{ id: uuid(1), enabled: false, applicability: "hard", confidence: "low", constraint: { type: "position", index: "", allowed: "" } }, { id: uuid(2), enabled: false, applicability: "soft", confidence: "high", constraint: { type: "contains", literal: "" } }];
    render(<CandidatePreview />); await open();
    expect(field("Condition 1 position").value).toBe(""); expect(field("Condition 1 allowed characters").value).toBe(""); expect(field("Condition 2 literal").value).toBe("");
    await save("Save", false); expect(saved()).toEqual(loaded);
  });
  it("retains edits made while an immutable Save snapshot is being written", async () => {
    let finish!: (value: ProjectResponse) => void;
    vi.mocked(projectSave).mockImplementation(async (action) => action.action === "select" ? { status: "selected", token: "new_token" } : new Promise((resolve) => { finish = resolve; }));
    render(<CandidatePreview />); change(seedName, "snapshot"); click("Save"); await phrase(true);
    await waitFor(() => expect(writes()).toHaveLength(1)); change(seedName, "later edit");
    await act(async () => finish({ status: "saved" })); await idle();
    expect(saved().manualSeeds.text).toBe("snapshot"); expect(field(seedName).value).toBe("later edit"); expect(status().textContent).toContain("Unsaved changes");
    change(seedName, "snapshot"); expect(status().textContent).toContain("No unsaved changes");
  });
  it("reports an already committed file if finalizing the file association fails", async () => {
    vi.mocked(projectOpen).mockImplementation(async (action) => {
      if (action.action === "adopt") throw new ProjectError("PROJECT_UNAVAILABLE");
      return { status: "discarded" };
    });
    render(<CandidatePreview />); change(seedName, "keep"); await save();
    expect(status().textContent).toContain("already committed"); expect(status().textContent).toContain("Unsaved changes");
    expect(status().textContent).toContain("No project file associated"); expect(field(seedName).value).toBe("keep");
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it.each(["cancel", "failure"] as const)("preserves project, binding and baseline on Save %s", async (failure) => {
    render(<CandidatePreview />); change(seedName, "baseline"); await save(); change(seedName, "dirty");
    click("Save"); await screen.findByLabelText("Passphrase", { exact: true });
    if (failure === "cancel") click("Cancel");
    else { outcome = { status: "notCommitted", code: "PROJECT_IO" }; change("Passphrase", "phrase"); click("Continue"); }
    await idle(); expect(field(seedName).value).toBe("dirty"); expect(status().textContent).toContain("Unsaved changes"); expect(status().textContent).toContain("Project file associated");
    change(seedName, "baseline"); expect(status().textContent).toContain("No unsaved changes");
  });
  it("requires explicit Save, Discard or Cancel for Open and cancels the chain when Save is cancelled", async () => {
    render(<CandidatePreview />); change(seedName, "keep"); click("Open"); click("Cancel Open"); await idle();
    expect(projectOpen).not.toHaveBeenCalled(); expect(field(seedName).value).toBe("keep");
    click("Open"); click("Save changes"); await screen.findByLabelText("Passphrase", { exact: true }); click("Cancel"); await idle();
    expect(vi.mocked(projectOpen).mock.calls.some(([action]) => action.action === "select")).toBe(false);
    await open(); expect(field(seedName).value).toBe("opened");
  });
  it("continues Open only after the unsaved snapshot is saved successfully", async () => {
    render(<CandidatePreview />); change(seedName, "save first"); click("Open"); click("Save changes"); await phrase(true);
    await phrase(false); await idle(); expect(saved().manualSeeds.text).toBe("save first"); expect(field(seedName).value).toBe("opened");
  });
  it("retains the old project and its baseline when a complete load cannot be staged", async () => {
    render(<CandidatePreview />); change(seedName, "old"); await save(); change(seedName, "dirty");
    loaded.schemaVersion = 2 as 1; await open();
    expect(field(seedName).value).toBe("dirty"); expect(status().textContent).toContain("Unsaved changes");
    change(seedName, "old"); expect(status().textContent).toContain("No unsaved changes");
    expect(vi.mocked(projectOpen).mock.calls.filter(([action]) => action.action === "adopt")).toHaveLength(1);
  });
  it("rolls back a lost Open adoption acknowledgement and keeps the old editing project and binding", async () => {
    render(<CandidatePreview />); change(seedName, "old baseline"); await save(); change(seedName, "old dirty");
    let nativeToken = "token_1";
    const implementation = vi.mocked(projectOpen).getMockImplementation()!;
    vi.mocked(projectOpen).mockImplementation(async (action) => {
      if (action.action === "adopt" && action.token === "token_2") { nativeToken = action.token; throw new Error("lost acknowledgement"); }
      if (action.action === "discard" && action.token === "token_2") { nativeToken = "token_1"; return { status: "discarded" }; }
      return implementation(action);
    });
    await open();
    expect(nativeToken).toBe("token_1"); expect(field(seedName).value).toBe("old dirty"); expect(status().textContent).toContain("previous editing project");
    expect(status().textContent).toContain("Unsaved changes");
    expect(vi.mocked(projectOpen).mock.calls.some(([action]) => action.action === "discard" && action.token === "token_2")).toBe(true);
    change(seedName, "old baseline"); expect(status().textContent).toContain("No unsaved changes");
    await save("Save", false); expect(writes().at(-1)!.token).toBe("token_1");
  });
  it("retries an unresolved adoption rollback before starting another Open", async () => {
    render(<CandidatePreview />); change(seedName, "old");
    const implementation = vi.mocked(projectOpen).getMockImplementation()!;
    let adoptionFails = true, rollbackFails = true;
    vi.mocked(projectOpen).mockImplementation(async (action) => {
      if (action.action === "adopt" && adoptionFails) throw new Error("lost acknowledgement");
      if (action.action === "discard" && rollbackFails) throw new Error("unavailable rollback");
      return implementation(action);
    });
    await open(); expect(field(seedName).value).toBe("old"); expect(status().textContent).toContain("rollback could not be confirmed");
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    const selects = vi.mocked(projectOpen).mock.calls.filter(([action]) => action.action === "select").length;
    click("Open"); await idle();
    expect(vi.mocked(projectOpen).mock.calls.filter(([action]) => action.action === "select")).toHaveLength(selects);
    rollbackFails = false; adoptionFails = false; await open();
    expect(field(seedName).value).toBe("opened"); expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
  });
  it("retains an acknowledged Open and retries finalization without rolling back or starting another write", async () => {
    render(<CandidatePreview />);
    const implementation = vi.mocked(projectOpen).getMockImplementation()!;
    let finalizeFails = true;
    vi.mocked(projectOpen).mockImplementation(async (action) => {
      if (action.action === "finalize" && finalizeFails) throw new Error("lost finalization acknowledgement");
      return implementation(action);
    });
    await open(); expect(field(seedName).value).toBe("opened"); expect(status().textContent).toContain("No unsaved changes");
    expect(status().textContent).toContain("cleanup could not be confirmed");
    expect(vi.mocked(projectOpen).mock.calls.some(([action]) => action.action === "discard")).toBe(false);
    click("Save"); await idle(); expect(writes()).toHaveLength(0);
    finalizeFails = false; await save("Save", false); expect(writes()).toHaveLength(1);
    expect(vi.mocked(projectOpen).mock.calls.filter(([action]) => action.action === "finalize").length).toBeGreaterThanOrEqual(3);
  });
  it.each(["saved", "committedDurabilityUncertain", "outcomeUnknown", "notCommitted"] as const)("reports %s with the correct baseline semantics", async (result) => {
    outcome = result === "notCommitted" ? { status: result, code: "PROJECT_IO" } : { status: result };
    render(<CandidatePreview />); change(seedName, "synthetic"); await save();
    const committed = result === "saved" || result === "committedDurabilityUncertain";
    expect(status().textContent).toContain(committed ? "No unsaved changes" : "Unsaved changes");
    expect(status().textContent).toContain(result === "saved" ? "Project saved" : result === "committedDurabilityUncertain" ? "durability is uncertain" : result === "outcomeUnknown" ? "Outcome unknown" : "Not committed");
    expect(writes()).toHaveLength(1);
  });
  it("Clear remains a dirty edit of the associated file and retains monotonically increasing editor identity", async () => {
    render(<CandidatePreview />); change(seedName, "old"); click("Add condition"); await save();
    click("Clear inputs and results"); expect(status().textContent).toContain("Project file associated"); expect(status().textContent).toContain("Unsaved changes");
    click("Add prefix"); change("Prefix 1", "p"); click("Add condition"); change(seedName, "new"); click("Preview candidates");
    await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0].directConstraints[0].editorId).toBe(3);
    await save("Save", false); expect(writes().at(-1)!.token).toBe("token_1");
  });
  it("blocks repeat saves after an unknown outcome until a successful Open resolves it", async () => {
    outcome = { status: "outcomeUnknown" };
    render(<CandidatePreview />); change(seedName, "keep"); await save();
    click("Save"); click("Save As"); expect(writes()).toHaveLength(1);
    click("Open"); expect((screen.getByRole("button", { name: "Save changes" }) as HTMLButtonElement).disabled).toBe(true);
    click("Cancel Open"); await idle(); expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    await open(); expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
    outcome = { status: "saved" }; await save("Save", false); expect(status().textContent).toContain("Project saved");
  });
  it.each(["success", "error"] as const)("keeps the Preview owner single-flight during Open and rejects stale %s", async (result) => {
    let resolve!: (value: PreviewResponse) => void, reject!: (reason: unknown) => void;
    vi.mocked(runCandidatePreview).mockImplementationOnce(() => new Promise((yes, no) => { resolve = yes; reject = no; }));
    render(<CandidatePreview />); change(seedName, "old"); click("Preview candidates"); const request = vi.mocked(runCandidatePreview).mock.calls[0][0];
    await open(); expect(field(seedName).value).toBe("opened"); expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    await act(async () => { if (result === "success") resolve(response(request)); else reject(new PreviewError("PREVIEW_CORE_FAILED")); });
    expect(screen.queryByRole("table")).toBeNull(); expect(screen.queryByText(/The Core could not complete/)).toBeNull();
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledTimes(2));
    const next = vi.mocked(runCandidatePreview).mock.calls[1][0]; expect(next.seeds).toEqual(["opened"]); expect(next.requestId).not.toBe(request.requestId);
  });
  it("rejects Save during authoring IME without discarding intermediate DOM input", () => {
    render(<CandidatePreview />); change(seedName, "accepted"); const seed = field(seedName);
    fireEvent.compositionStart(seed); change(seedName, "中間"); click("Save");
    expect(projectSave).not.toHaveBeenCalled(); expect(seed.value).toBe("中間"); expect(status().textContent).toContain("Finish IME composition");
    fireEvent.compositionEnd(seed); expect(seed.value).toBe("中間");
  });
  it.each(["Attempt limit", "Emission limit"])("tracks %s composition and preserves its confirmed raw draft", async (name) => {
    render(<CandidatePreview />); const limit = field(name); fireEvent.compositionStart(limit); change(name, "0002"); click("Save");
    expect(projectSave).not.toHaveBeenCalled(); expect(limit.value).toBe("0002"); fireEvent.compositionEnd(limit);
    await save(); expect(saved().limits[name === "Attempt limit" ? "attemptLimit" : "emissionLimit"]).toBe("0002");
  });
  it("remounts loaded inputs and ignores an old compositionEnd without restoring old seed text", async () => {
    render(<CandidatePreview />); change(seedName, "accepted"); const old = field(seedName); fireEvent.compositionStart(old); change(seedName, "intermediate");
    fireEvent.compositionEnd(old); expect(old.value).toBe("intermediate");
    await open(); fireEvent.compositionEnd(old); expect(field(seedName)).not.toBe(old); expect(field(seedName).value).toBe("opened");
    field(seedName).value = "old late input"; fireEvent.compositionEnd(field(seedName)); expect(field(seedName).value).toBe("opened");
  });
  it("prevents conflicting file operations and Clear while keeping Save snapshot edits possible", async () => {
    let finish!: (value: ProjectResponse) => void;
    vi.mocked(projectSave).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
    render(<CandidatePreview />); click("Save"); click("Open"); click("Save As"); click("Clear inputs and results"); change(seedName, "later");
    expect(projectSave).toHaveBeenCalledTimes(1); expect(projectOpen).not.toHaveBeenCalled(); expect(field(seedName).value).toBe("later");
    await act(async () => finish({ status: "cancelled" })); await idle(); expect(field(seedName).value).toBe("later");
  });
  it("locks editing through Open and preserves state on authentication failure", async () => {
    render(<CandidatePreview />); change(seedName, "old"); click("Open"); click("Discard changes");
    await screen.findByLabelText("Passphrase", { exact: true });
    expect(field(seedName).closest("fieldset")!.disabled).toBe(true);
    vi.mocked(projectOpen).mockImplementationOnce(async () => { throw new ProjectError("PROJECT_AUTHENTICATION_FAILED"); });
    change("Passphrase", "wrong"); click("Continue"); await idle();
    expect(field(seedName).value).toBe("old"); expect(status().textContent).toContain("could not authenticate");
  });
  it("requires matching passphrases, retains exact spaces and resets inputs after cancellation", async () => {
    render(<CandidatePreview />); click("Save"); await screen.findByLabelText("Passphrase", { exact: true });
    change("Passphrase", " x "); change("Confirm passphrase", "x"); click("Continue");
    expect(screen.getByRole("alert").textContent).toContain("do not match"); expect(writes()).toHaveLength(0);
    click("Cancel"); await idle(); click("Save"); await screen.findByLabelText("Passphrase", { exact: true });
    expect(field("Passphrase").value).toBe(""); expect(field("Confirm passphrase").value).toBe(""); click("Cancel"); await idle();
  });
  it("compares raw numeric strings, trailing LF and empty affix rows against the saved baseline", async () => {
    render(<CandidatePreview />); change(seedName, "seed"); change("Attempt limit", "01"); await save();
    change("Attempt limit", "1"); expect(status().textContent).toContain("Unsaved changes");
    change("Attempt limit", "01"); expect(status().textContent).toContain("No unsaved changes");
    change(seedName, "seed\n"); expect(status().textContent).toContain("Unsaved changes");
    change(seedName, "seed"); expect(status().textContent).toContain("No unsaved changes");
    click("Add prefix"); expect(status().textContent).toContain("Unsaved changes");
    click("Remove prefix 1"); expect(status().textContent).toContain("No unsaved changes");
    const before = status().textContent;
    fireEvent.paste(field(seedName), { clipboardData: { getData: () => "a\rb" } });
    expect(status().textContent).toBe(before);
  });
  it("saves eight affix rows plus includeEmpty without forcing Preview eligibility", async () => {
    render(<CandidatePreview />);
    for (let index = 0; index < 8; index += 1) { click("Add prefix"); change(`Prefix ${index + 1}`, String(index)); }
    expect((screen.getByRole("button", { name: "Add prefix" }) as HTMLButtonElement).disabled).toBe(true);
    await save(); expect(saved().options.prefixes.includeEmpty).toBe(true); expect(saved().options.prefixes.rows).toHaveLength(8);
    expect((screen.getByRole("button", { name: "Preview candidates" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it("restores Group selections, remapped references and affix values in the next Preview payload", async () => {
    loaded.clues = [{ id: uuid(1), text: "synthetic clue", source: "source" }];
    loaded.hypotheses = [{ id: uuid(2), origin: { type: "clue", clueId: uuid(1) }, interpretation: "interpretation", applicability: "soft", confidence: "high", activation: { type: "grouped", groupId: uuid(3) }, effects: [] }];
    loaded.groups = [{ id: uuid(3), anchorClueId: uuid(1), selectedHypothesisId: uuid(2) }];
    loaded.options.prefixes = { includeEmpty: false, rows: [{ id: uuid(4), value: "PRE-" }] };
    render(<CandidatePreview />); click("Add condition"); click("Clear inputs and results"); await open();
    expect(field("Hypothesis 1 interpretation").value).toBe("interpretation"); expect(field("Prefix 1").value).toBe("PRE-");
    const selected = screen.getByRole("radio", { name: /select hypothesis 1/ }) as HTMLInputElement; expect(selected.checked).toBe(true);
    click("Preview candidates"); await waitFor(() => expect(runCandidatePreview).toHaveBeenCalledOnce());
    const request = vi.mocked(runCandidatePreview).mock.calls[0][0];
    expect(request.clues[0].editorId).toBeGreaterThan(1);
    expect(request.hypotheses[0]).toMatchObject({ clueEditorId: request.clues[0].editorId, groupEditorId: request.groups[0].editorId, enabled: false });
    expect(request.groups[0].selectedHypothesisEditorId).toBe(request.hypotheses[0].editorId); expect(request.options.prefixes).toEqual(["PRE-"]);
    await save("Save", false); expect(saved()).toEqual(loaded);
  });
});
