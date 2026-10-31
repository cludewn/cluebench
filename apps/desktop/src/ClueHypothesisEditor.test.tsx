// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, createEvent, fireEvent, render, screen, within } from "@testing-library/react";
import App from "./App";
import { runCandidatePreview } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";

vi.mock("./core-bridge", async (importOriginal) => ({ ...(await importOriginal<typeof import("./core-bridge")>()), runCoreProbe: vi.fn() }));
vi.mock("./preview-bridge", async (importOriginal) => ({ ...(await importOriginal<typeof import("./preview-bridge")>()), runCandidatePreview: vi.fn() }));
afterEach(cleanup);
beforeEach(() => { vi.mocked(runCandidatePreview).mockReset(); });
const button = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;
const start = () => button("Preview candidates");
const text = (name: string) => screen.getByRole("textbox", { name }) as HTMLTextAreaElement;
const change = (name: string, value: string) => fireEvent.change(screen.getByLabelText(name), { target: { value } });
const click = (name: string) => fireEvent.click(button(name));
const seed = () => change("Manual seeds — one per line", "Synthetic1!");
const addClue = () => click("Add Clue");
const addHypothesis = (clue = 1) => click(`Add hypothesis for clue ${clue}`);
const addEffect = (hypothesis = 1) => click(`Add effect to hypothesis ${hypothesis}`);
const status = () => screen.getByRole("status", { name: "Preview status" });
const request = () => vi.mocked(runCandidatePreview).mock.calls.at(-1)![0];
function response(req: PreviewRequest): PreviewResponse {
  let nextId = req.seeds.length + req.directConstraints.length * 2 + req.clues.length + 1;
  const hypothesisBindings = req.hypotheses.map((hypothesis) => {
    const binding = { editorId: hypothesis.editorId, hypothesisId: nextId };
    nextId += 1 + hypothesis.effects.length;
    return binding;
  });
  return {
    requestId: req.requestId, effectiveLimits: req.limits,
    constraintBindings: req.directConstraints.map((row, index) => ({ editorId: row.editorId, hypothesisId: req.seeds.length + 2 * index + 1 })),
    hypothesisBindings,
    candidates: [{ text: req.seeds[0], seedId: 1, path: { seedIndex: 0, caseIndex: 0, caseVariant: req.options.caseVariants[0], prefixIndex: 0, suffixIndex: 0 },
      prefix: req.options.prefixes[0], suffix: req.options.suffixes[0], relativePoints: 0, matchedSoftHypothesisIds: [] }],
    stats: { attemptedPaths: 1, domainSkips: 0, hardRejections: 0, emittedCandidates: 1 }, potentialPaths: 1, stop: "completed",
  };
}
function hold() {
  let resolve: (value: PreviewResponse) => void = () => {};
  let reject: (value: unknown) => void = () => {};
  vi.mocked(runCandidatePreview).mockReturnValueOnce(new Promise((yes, no) => { resolve = yes; reject = no; }));
  return { resolve, reject };
}
function transfer(target: Element, route: "paste" | "drop", raw: string) {
  const data = { types: ["text/plain"], getData: (type: string) => type === "text/plain" ? raw : "" };
  const event = route === "paste" ? createEvent.paste(target, { bubbles: true, cancelable: true, clipboardData: data }) :
    createEvent.drop(target, { bubbles: true, cancelable: true, dataTransfer: data });
  fireEvent(target, event);
  return event;
}
function setup() {
  render(<App />); seed(); addClue(); addHypothesis();
}

// Matching in these tests is a supplied Core response; no frontend matcher is used.
describe("Clue-backed editing (React/jsdom, mock IPC)", () => {
  it("edits separate Unicode Clue/source/interpretation fields and submits exact scalar text", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup();
    const evidence = "  e\u0301\n\t🔑 \u624b\u304c\u304b\u308a  ";
    change("Clue 1 original text", evidence); change("Clue 1 source", "\t  synthetic\nsource  "); change("Hypothesis 1 interpretation", " \u89e3\u91c8\n🧩 ");
    expect(text("Clue 1 original text").tagName).toBe("TEXTAREA");
    expect(text("Clue 1 source").tagName).toBe("TEXTAREA");
    expect(text("Hypothesis 1 interpretation").tagName).toBe("TEXTAREA");
    expect(start().disabled).toBe(false); click("Preview candidates");
    await screen.findByText(/Success: preview/);
    expect(request().clues[0]).toEqual({ editorId: 1, text: evidence, source: "\t  synthetic\nsource  " });
    expect(request().hypotheses[0]).toMatchObject({ editorId: 2, clueEditorId: 1, interpretation: " \u89e3\u91c8\n🧩 ", effects: [] });
  });
  it("keeps empty and whitespace-only evidence and zero-effect hypotheses valid", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup();
    expect(screen.getByText(/Zero effects:/)).toBeTruthy();
    change("Clue 1 source", " \t\n ");
    expect(start().disabled).toBe(false); click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().hypotheses[0].effects).toEqual([]);
    expect(request().hypotheses[0].interpretation).toBe("");
  });
  it("blocks deleting a referenced Clue even for disabled zero-effect drafts and requires explicit reassignment", () => {
    setup(); addClue();
    fireEvent.click(screen.getByLabelText("Enable hypothesis 1"));
    expect(button("Remove clue 1").disabled).toBe(true);
    expect(screen.getByText(/To remove this Clue, first remove its hypotheses/)).toBeTruthy();
    click("Remove clue 1");
    expect(text("Clue 2 original text")).toBeTruthy();
    const secondField = text("Clue 2 original text");
    change("Hypothesis 1 originating Clue", "3");
    expect(button("Remove clue 1").disabled).toBe(false);
    click("Remove clue 1");
    expect(text("Clue 1 original text")).toBe(secondField);
    expect((screen.getByLabelText("Hypothesis 1 originating Clue") as HTMLSelectElement).value).toBe("3");
    expect(button("Remove clue 1").disabled).toBe(true);
    click("Remove hypothesis 1");
    expect(button("Remove clue 1").disabled).toBe(false); click("Remove clue 1");
    expect(screen.queryByLabelText("Clue 1 original text")).toBeNull();
  });
  it("keeps duplicate interpretations/effects distinct, preserves IDs across editing/type/origin changes, and never reuses IDs", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup(); addHypothesis(); addEffect(1); addEffect(1); addEffect(2); addClue();
    const lastField = text("Hypothesis 1 effect 2 minimum");
    click("Remove hypothesis 1 effect 1");
    expect(text("Hypothesis 1 effect 1 minimum")).toBe(lastField);
    change("Hypothesis 1 effect 1 type", "contains"); change("Hypothesis 1 effect 1 literal", "Synthetic");
    change("Hypothesis 1 originating Clue", "7");
    change("Hypothesis 1 applicability", "soft"); change("Hypothesis 1 confidence", "high");
    click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().clues.map((clue) => clue.editorId)).toEqual([1, 7]);
    expect(request().hypotheses.map((hypothesis) => hypothesis.editorId)).toEqual([2, 3]);
    expect(request().hypotheses[0]).toMatchObject({ clueEditorId: 7, enabled: true, applicability: "soft", confidence: "high", effects: [{ editorId: 5, constraint: { type: "contains", literal: "Synthetic" } }] });
    expect(request().hypotheses[1].effects[0].editorId).toBe(6);
    click("Clear inputs and results"); seed(); click("Add condition"); addClue(); addHypothesis(); addEffect();
    click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().directConstraints[0].editorId).toBe(8);
    expect(request().clues[0].editorId).toBe(9);
    expect(request().hypotheses[0].editorId).toBe(10);
    expect(request().hypotheses[0].effects[0].editorId).toBe(11);
  });
  it("retains a hypothesis after its last effect is removed", () => {
    setup(); addEffect(); click("Remove hypothesis 1 effect 1");
    expect(text("Hypothesis 1 interpretation")).toBeTruthy();
    expect(screen.getByText(/Zero effects:/)).toBeTruthy();
    expect(start().disabled).toBe(false);
  });
  it("blocks incomplete disabled effects and preserves numeric drafts without correction", () => {
    setup(); addEffect();
    change("Hypothesis 1 effect 1 minimum", " 2"); fireEvent.click(screen.getByLabelText("Enable hypothesis 1"));
    expect(text("Hypothesis 1 effect 1 minimum").value).toBe(" 2"); expect(start().disabled).toBe(true);
    change("Hypothesis 1 effect 1 minimum", "2.5"); expect(start().disabled).toBe(true);
    change("Hypothesis 1 effect 1 minimum", "2"); expect(start().disabled).toBe(false);
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });
  it.each(["length", "position", "contains", "notContains", "prefix", "suffix", "characterCount"] as const)("reuses %s constraint editing without changing identities", async (type) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup(); addEffect(); change("Hypothesis 1 effect 1 type", type);
    if (type === "position") { change("Hypothesis 1 effect 1 position", "2"); change("Hypothesis 1 effect 1 allowed characters", " AaAA "); }
    else if (type === "length" || type === "characterCount") {
      change("Hypothesis 1 effect 1 minimum", "1"); change("Hypothesis 1 effect 1 maximum", "30");
      if (type === "characterCount") change("Hypothesis 1 effect 1 character class", "symbol");
    } else change("Hypothesis 1 effect 1 literal", " MiXeD ");
    click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().hypotheses[0].effects[0]).toMatchObject({ editorId: 3, constraint: { type } });
    if (type === "position") expect(request().hypotheses[0].effects[0].constraint).toMatchObject({ index: 2, allowed: " AaAA " });
    else if (type !== "length" && type !== "characterCount") expect(request().hypotheses[0].effects[0].constraint).toMatchObject({ literal: " MiXeD " });
  });
  it("enforces 8 Clues, 8 hypotheses, 4 effects per hypothesis and 16 effects total", () => {
    setup(); for (let i = 1; i < 8; i++) { addClue(); addHypothesis(); }
    expect(button("Add Clue").disabled).toBe(true); expect(button("Add hypothesis for clue 1").disabled).toBe(true);
    for (let h = 1; h <= 4; h++) for (let e = 0; e < 4; e++) addEffect(h);
    expect(button("Add effect to hypothesis 1").disabled).toBe(true); expect(button("Add effect to hypothesis 5").disabled).toBe(true);
    click("Remove hypothesis 1 effect 1"); expect(button("Add effect to hypothesis 5").disabled).toBe(false);
  });
  it("retains all drafts while the shared canonical request budget disables Preview", () => {
    render(<App />);
    change("Manual seeds — one per line", Array.from({ length: 64 }, () => '"'.repeat(64)).join("\n"));
    for (const name of ["Prefix", "Suffix"] as const) for (let i = 1; i <= 7; i++) {
      click(`Add ${name.toLowerCase()}`); change(`${name} ${i}`, '"'.repeat(63) + String(i));
    }
    for (let i = 1; i <= 16; i++) {
      click("Add condition"); change(`Condition ${i} type`, "position"); change(`Condition ${i} allowed characters`, '"'.repeat(95));
    }
    addClue(); change("Clue 1 original text", '"'.repeat(512));
    expect(start().disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("The preview request exceeds 16 KiB. Reduce the input.");
    expect(text("Clue 1 original text").value).toBe('"'.repeat(512));
    expect(text("Condition 16 allowed characters").value).toBe('"'.repeat(95));
    expect(text("Manual seeds — one per line").value.split("\n")).toHaveLength(64);
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });
  it("renders submitted multi-effect Soft hypothesis provenance alongside Direct Constraints", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => {
      const res = response(req); res.candidates[0].matchedSoftHypothesisIds = [res.constraintBindings[0].hypothesisId, res.hypothesisBindings[0].hypothesisId];
      res.candidates[0].relativePoints = 5;
      return res;
    });
    setup(); addEffect(); addEffect(); click("Add condition");
    change("Clue 1 original text", "\tOriginal e\u0301\n🧩 "); change("Clue 1 source", "Synthetic source\n  "); change("Hypothesis 1 interpretation", "Interpretation");
    change("Hypothesis 1 applicability", "soft"); change("Condition 1 applicability", "soft"); change("Condition 1 confidence", "high");
    click("Preview candidates"); await screen.findByText(/Success: preview/);
    const table = within(screen.getByRole("table"));
    expect(table.getByText("Interpretation", { selector: "dd" })).toBeTruthy();
    expect(table.getByText("Original é 🧩", { exact: false })).toBeTruthy();
    expect(table.getByText("Synthetic source", { exact: false }).textContent).toBe("Synthetic source\n  ");
    expect(table.getByText("5")).toBeTruthy();
    expect(table.getByText("Length 1–64")).toBeTruthy();
    const submitted = request(); change("Clue 1 original text", "changed");
    expect(screen.queryByRole("table")).toBeNull(); expect(submitted.clues[0].text).toBe("\tOriginal e\u0301\n🧩 ");
  });
  it.each(["clue edit", "source edit", "interpretation", "origin", "enabled", "applicability", "confidence", "add effect", "edit effect", "remove effect", "remove hypothesis", "remove clue", "clear"] as const)("invalidates an existing result on %s", async (action) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup(); addClue(); addEffect(); click("Preview candidates"); await screen.findByText(/Success: preview/);
    edit(action);
    expect(screen.queryByRole("table")).toBeNull(); expect(runCandidatePreview).toHaveBeenCalledTimes(1);
  });
  it.each(["success", "error"] as const)("drops stale %s after pending Clue edits/deletion/Clear and permits retry after settlement", async (outcome) => {
    const pending = hold(); setup(); addClue(); addEffect(); click("Preview candidates"); const req = request();
    change("Clue 1 original text", "new"); click("Remove hypothesis 1"); click("Remove clue 1"); click("Clear inputs and results");
    seed(); addClue(); addHypothesis();
    expect(status().textContent).toContain("State: Loading"); expect(button("Preview running…").disabled).toBe(true);
    await act(async () => { if (outcome === "success") pending.resolve(response(req)); else pending.reject(new Error("Private error")); });
    expect(screen.queryByRole("table")).toBeNull(); expect(status().textContent).not.toContain("Private"); expect(start().disabled).toBe(false);
    vi.mocked(runCandidatePreview).mockImplementation(async (next) => response(next)); click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().clues[0].editorId).toBeGreaterThan(req.hypotheses[0].effects[0].editorId);
  });
});

type Action = "clue edit" | "source edit" | "interpretation" | "origin" | "enabled" | "applicability" | "confidence" | "add effect" | "edit effect" | "remove effect" | "remove hypothesis" | "remove clue" | "clear";
function edit(action: Action) {
  switch (action) {
    case "clue edit": change("Clue 1 original text", "revised"); break;
    case "source edit": change("Clue 1 source", "revised"); break;
    case "interpretation": change("Hypothesis 1 interpretation", "revised"); break;
    case "origin": change("Hypothesis 1 originating Clue", "3"); break;
    case "enabled": fireEvent.click(screen.getByLabelText("Enable hypothesis 1")); break;
    case "applicability": change("Hypothesis 1 applicability", "soft"); break;
    case "confidence": change("Hypothesis 1 confidence", "high"); break;
    case "add effect": addEffect(); break;
    case "edit effect": change("Hypothesis 1 effect 1 minimum", "2"); break;
    case "remove effect": click("Remove hypothesis 1 effect 1"); break;
    case "remove hypothesis": click("Remove hypothesis 1"); break;
    case "remove clue": click("Remove clue 2"); break;
    case "clear": click("Clear inputs and results"); break;
  }
}

describe("Unicode evidence event guards (React/jsdom)", () => {
  const evidenceFields = ["Clue 1 original text", "Clue 1 source", "Hypothesis 1 interpretation"];
  it.each(evidenceFields)("rejects observed CR/CRLF paste/drop and native beforeinput for %s", (name) => {
    setup(); change(name, "kept");
    const field = text(name);
    for (const raw of ["Private\rPayload", "Private\r\nPayload", "Private\ud800Payload", "Private\udfffPayload"]) {
      for (const route of ["paste", "drop"] as const) {
        expect(transfer(field, route, raw).defaultPrevented).toBe(true); expect(field.value).toBe("kept");
        expect(screen.getByRole("alert").textContent).not.toContain("Private");
      }
      const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertText", data: raw });
      fireEvent(field, event); expect(event.defaultPrevented).toBe(true); expect(field.value).toBe("kept");
      expect(start().disabled).toBe(true);
    }
  });
  it.each(evidenceFields)("accepts LF/TAB/Unicode transfer and rejects lone surrogate final values for %s", (name) => {
    setup(); const field = text(name); const accepted = "  e\u0301\n\t🧩 ";
    expect(transfer(field, "paste", accepted).defaultPrevented).toBe(false);
    expect(transfer(field, "drop", accepted).defaultPrevented).toBe(false);
    // jsdom does not insert transferred text; model the valid default insertion.
    change(name, accepted); expect(field.value).toBe(accepted); expect(start().disabled).toBe(false);
    change(name, "Private\ud800Payload"); expect(field.value).toBe(accepted); expect(start().disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).not.toContain("Private");
  });
  it.each([["Clue 1 original text", 512], ["Clue 1 source", 256], ["Hypothesis 1 interpretation", 256]] as const)("retains prior accepted %s at its exact UTF-8 byte limit", (name, maxBytes) => {
    setup(); const accepted = "🧩".repeat(maxBytes / 4); change(name, accepted);
    expect(text(name).value).toBe(accepted); expect(start().disabled).toBe(false);
    change(name, accepted + "a"); expect(text(name).value).toBe(accepted); expect(start().disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("previous value was retained");
  });
  it("restores a controlled value after noncancelable raw beforeinput without accepting a sanitized replacement", () => {
    setup(); change("Clue 1 original text", "kept"); const field = text("Clue 1 original text");
    const event = new InputEvent("beforeinput", { bubbles: true, cancelable: false, inputType: "insertText", data: "Private\r\nPayload" });
    fireEvent(field, event); expect(event.defaultPrevented).toBe(false);
    change("Clue 1 original text", "Private\nPayload"); expect(field.value).toBe("kept"); expect(start().disabled).toBe(true);
    change("Clue 1 original text", "valid"); expect(start().disabled).toBe(false);
  });
  it("inspects native dataTransfer while permitting evidence line-break events", () => {
    setup(); const field = text("Clue 1 source");
    const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertText", data: "safe" });
    Object.defineProperty(event, "dataTransfer", { value: { getData: () => "Private\r\nPayload" } });
    fireEvent(field, event); expect(event.defaultPrevented).toBe(true);
    change("Clue 1 source", "valid");
    const lineBreak = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertLineBreak" });
    fireEvent(field, lineBreak); expect(lineBreak.defaultPrevented).toBe(false);
  });
  it("blocks Preview throughout IME composition and commits only validated completion text", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => response(req));
    setup(); change("Clue 1 original text", "accepted"); click("Preview candidates"); await screen.findByText(/Success: preview/);
    const field = text("Clue 1 original text"); fireEvent.compositionStart(field);
    expect(screen.queryByRole("table")).toBeNull(); expect(start().disabled).toBe(true);
    change("Clue 1 original text", "\u4e2d\u9593"); expect(field.value).toBe("\u4e2d\u9593");
    expect(screen.getByRole("alert").textContent).toContain("Finish IME composition");
    fireEvent.compositionEnd(field, { data: "\u78ba\u5b9a" });
    expect(start().disabled).toBe(false); expect(field.value).toBe("\u4e2d\u9593");
    click("Preview candidates"); await screen.findByText(/Success: preview/);
    expect(request().clues[0].text).toBe("\u4e2d\u9593");
  });
  it("rejects an oversized composition completion and retains the last accepted evidence", () => {
    setup(); change("Clue 1 source", "kept"); const field = text("Clue 1 source");
    fireEvent.compositionStart(field); change("Clue 1 source", "🧩".repeat(65));
    expect(field.value).toBe("🧩".repeat(65)); expect(start().disabled).toBe(true);
    fireEvent.compositionEnd(field); expect(field.value).toBe("kept"); expect(start().disabled).toBe(true);
    change("Clue 1 source", "fixed"); expect(start().disabled).toBe(false);
  });
  it("restores noncancelable rejected composition input before a later valid beforeinput can erase the rejection marker", () => {
    setup(); change("Clue 1 source", "kept"); const field = text("Clue 1 source");
    fireEvent.compositionStart(field);
    fireEvent(field, new InputEvent("beforeinput", { bubbles: true, cancelable: false, inputType: "insertCompositionText", data: "Private\rPayload" }));
    change("Clue 1 source", "Private\nPayload"); expect(field.value).toBe("kept");
    fireEvent(field, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertCompositionText", data: "safe" }));
    change("Clue 1 source", "keptsafe"); fireEvent.compositionEnd(field);
    expect(field.value).toBe("keptsafe"); expect(start().disabled).toBe(false);
  });
  it.each(["seed", "affix", "direct", "effect"] as const)("also blocks Preview during %s composition and commits the completed ASCII draft", (domain) => {
    setup();
    let name = "Manual seeds — one per line";
    if (domain === "affix") { click("Add prefix"); name = "Prefix 1"; change(name, "kept"); }
    if (domain === "direct") { click("Add condition"); name = "Condition 1 minimum"; }
    if (domain === "effect") { addEffect(); name = "Hypothesis 1 effect 1 minimum"; }
    const field = text(name); fireEvent.compositionStart(field); expect(start().disabled).toBe(true);
    const completed = domain === "direct" || domain === "effect" ? "2" : "finished";
    change(name, completed); expect(field.value).toBe(completed); expect(start().disabled).toBe(true);
    fireEvent.compositionEnd(field); expect(start().disabled).toBe(false); expect(field.value).toBe(completed);
  });
  it("does not leave Preview blocked after removing a composing field or Clear", () => {
    setup(); fireEvent.compositionStart(text("Hypothesis 1 interpretation")); expect(start().disabled).toBe(true);
    click("Remove hypothesis 1"); expect(start().disabled).toBe(false);
    fireEvent.compositionStart(text("Clue 1 original text")); click("Clear inputs and results"); seed(); expect(start().disabled).toBe(false);
  });
});
