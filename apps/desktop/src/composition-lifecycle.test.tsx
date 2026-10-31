// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import CandidatePreview from "./CandidatePreview";
import type { PreviewCandidate, PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
afterEach(cleanup);
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockReset().mockImplementation(async (_command, payload) => response(decode(payload)));
});

const field = (name: string) => screen.getByRole("textbox", { name }) as HTMLInputElement | HTMLTextAreaElement;
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const change = (name: string, value: string) => fireEvent.change(field(name), { target: { value } });
const seedName = "Manual seeds — one per line";
const decode = (payload: unknown): PreviewRequest => JSON.parse(new TextDecoder().decode(payload as Uint8Array));
const requests = () => vi.mocked(invoke).mock.calls.map(([, payload]) => decode(payload));
const start = () => screen.getByRole("button", { name: "Preview candidates" }) as HTMLButtonElement;

// Use the real bridge and canonical encoder; only the native transport is mocked.
// These synthetic fixtures use Original and default Length effects, without a matcher.
function response(request: PreviewRequest): PreviewResponse {
  const candidates: PreviewCandidate[] = [];
  request.seeds.forEach((seed, seedIndex) => request.options.prefixes.forEach((prefix, prefixIndex) => request.options.suffixes.forEach((suffix, suffixIndex) => {
    candidates.push({ text: prefix + seed + suffix, seedId: seedIndex + 1,
      path: { seedIndex, caseIndex: 0, caseVariant: "original", prefixIndex, suffixIndex },
      prefix, suffix, relativePoints: 0, matchedSoftHypothesisIds: [] });
  })));
  let nextId = request.seeds.length + request.directConstraints.length * 2 + request.clues.length + 1;
  return { requestId: request.requestId, effectiveLimits: request.limits, candidates,
    stats: { attemptedPaths: candidates.length, emittedCandidates: candidates.length, domainSkips: 0, hardRejections: 0 },
    potentialPaths: candidates.length, stop: "completed",
    constraintBindings: request.directConstraints.map((row, index) => ({ editorId: row.editorId, hypothesisId: request.seeds.length + index * 2 + 1 })),
    hypothesisBindings: request.hypotheses.map((hypothesis) => {
      const binding = { editorId: hypothesis.editorId, hypothesisId: nextId };
      nextId += 1 + hypothesis.effects.length;
      return binding;
    }),
  };
}

describe("Composition lifetime and canonical Preview payloads", () => {
  it("does not submit a cleared seed after a delayed compositionEnd", async () => {
    render(<CandidatePreview />); change(seedName, "accepted");
    const oldSeed = field(seedName);
    fireEvent.compositionStart(oldSeed); change(seedName, "intermediate");
    click("Clear inputs and results"); fireEvent.compositionEnd(oldSeed);
    click("Preview candidates");
    expect(requests().map((request) => request.seeds)).toEqual([]);
    expect(field(seedName).value).toBe("");
    expect(start().disabled).toBe(true);
    change(seedName, "fresh"); click("Preview candidates");
    await screen.findByText(/Success: preview results received/);
    expect(requests().map((request) => request.seeds)).toEqual([["fresh"]]);
  });

  it.each(["Prefix", "Suffix"] as const)("keeps the remaining %s value when its composing first row is removed", async (name) => {
    render(<CandidatePreview />); change(seedName, "seed");
    click(`Add ${name.toLowerCase()}`); change(`${name} 1`, "first");
    click(`Add ${name.toLowerCase()}`); change(`${name} 2`, "second");
    const remaining = field(`${name} 2`);
    fireEvent.compositionStart(field(`${name} 1`)); change(`${name} 1`, "composing");
    click(`Remove ${name.toLowerCase()} 1`); fireEvent.compositionEnd(field(`${name} 1`));
    click("Preview candidates"); await screen.findByText(/Success: preview results received/);
    const request = requests()[0];
    expect(name === "Prefix" ? request.options.prefixes : request.options.suffixes).toEqual(["", "second"]);
    expect(field(`${name} 1`)).toBe(remaining);
    expect(field(`${name} 1`).value).toBe("second");
  });

  it("discards the old seed guard, ignores an unmatched end on the new field, and accepts fresh composition", async () => {
    render(<CandidatePreview />); change(seedName, "accepted");
    const oldSeed = field(seedName);
    fireEvent.compositionStart(oldSeed); change(seedName, "intermediate");
    click("Clear inputs and results");
    const newSeed = field(seedName);
    expect(newSeed).not.toBe(oldSeed);
    fireEvent.compositionEnd(oldSeed);
    // An unmatched end must also restore an already-mutated current DOM value.
    newSeed.value = "intermediate"; fireEvent.compositionEnd(newSeed);
    expect(newSeed.value).toBe(""); expect(start().disabled).toBe(true);
    click("Preview candidates"); expect(requests()).toEqual([]);
    fireEvent.compositionStart(newSeed); change(seedName, "fresh");
    expect(start().disabled).toBe(true); expect(requests()).toEqual([]);
    fireEvent.compositionEnd(newSeed); click("Preview candidates");
    await screen.findByText(/Success: preview results received/);
    expect(requests().map((request) => request.seeds)).toEqual([["fresh"]]);
  });

  it.each(["Prefix", "Suffix"] as const)("preserves %s row identities after deleting a composing middle row and accepts new IME input", async (name) => {
    render(<CandidatePreview />); change(seedName, "seed");
    for (const [index, value] of ["first", "middle", "third"].entries()) {
      click(`Add ${name.toLowerCase()}`); change(`${name} ${index + 1}`, value);
    }
    const first = field(`${name} 1`), removed = field(`${name} 2`), third = field(`${name} 3`);
    fireEvent.compositionStart(removed); change(`${name} 2`, "composing");
    click(`Remove ${name.toLowerCase()} 2`); fireEvent.compositionEnd(removed);
    expect(field(`${name} 1`)).toBe(first); expect(field(`${name} 2`)).toBe(third);
    third.value = "composing"; fireEvent.compositionEnd(third);
    expect(third.value).toBe("third");
    fireEvent.compositionStart(third); change(`${name} 2`, "confirmed-third");
    expect(start().disabled).toBe(true);
    fireEvent.compositionEnd(third); click("Preview candidates");
    await screen.findByText(/Success: preview results received/);
    const request = requests()[0];
    expect(name === "Prefix" ? request.options.prefixes : request.options.suffixes).toEqual(["", "first", "confirmed-third"]);
  });

  it.each(["Prefix", "Suffix"] as const)("retains a surviving %s row's own active composition when another row is removed", async (name) => {
    render(<CandidatePreview />); change(seedName, "seed");
    for (const [index, value] of ["first", "middle", "third"].entries()) {
      click(`Add ${name.toLowerCase()}`); change(`${name} ${index + 1}`, value);
    }
    const third = field(`${name} 3`);
    fireEvent.compositionStart(third); change(`${name} 3`, "own-draft");
    click(`Remove ${name.toLowerCase()} 2`);
    expect(field(`${name} 2`)).toBe(third); expect(third.value).toBe("own-draft");
    expect(start().disabled).toBe(true); expect(requests()).toEqual([]);
    fireEvent.compositionEnd(third); click("Preview candidates");
    await screen.findByText(/Success: preview results received/);
    const request = requests()[0];
    expect(name === "Prefix" ? request.options.prefixes : request.options.suffixes).toEqual(["", "first", "own-draft"]);
  });

  it.each(["Prefix", "Suffix"] as const)("discards cleared %s composition and starts a fresh row with the original No-affix behavior", async (name) => {
    render(<CandidatePreview />); change(seedName, "seed");
    click(`Add ${name.toLowerCase()}`); change(`${name} 1`, "accepted");
    const removed = field(`${name} 1`);
    fireEvent.compositionStart(removed); change(`${name} 1`, "discarded");
    click("Clear inputs and results"); change(seedName, "fresh-seed");
    click(`Add ${name.toLowerCase()}`); change(`${name} 1`, "fresh-affix");
    const fresh = field(`${name} 1`);
    expect(fresh).not.toBe(removed); fireEvent.compositionEnd(removed);
    fireEvent.compositionStart(fresh); change(`${name} 1`, "confirmed-affix");
    expect(start().disabled).toBe(true); fireEvent.compositionEnd(fresh);
    fireEvent.click(screen.getByLabelText(`No ${name.toLowerCase()}`));
    click("Preview candidates"); await screen.findByText(/Success: preview results received/);
    const request = requests()[0];
    expect(request.seeds).toEqual(["fresh-seed"]);
    expect(name === "Prefix" ? request.options.prefixes : request.options.suffixes).toEqual(["confirmed-affix"]);
  });

  it.each(["clue", "hypothesis", "direct", "effect"] as const)("keeps stable %s guards separate after deletion and ignores old composition endings", async (kind) => {
    render(<CandidatePreview />); change(seedName, "seed");
    let firstName: string, secondName: string, remove: string;
    if (kind === "direct") {
      click("Add condition"); click("Add condition");
      firstName = "Condition 1 minimum"; secondName = "Condition 2 minimum"; remove = "Remove condition 1";
    } else {
      click("Add Clue");
      if (kind === "clue") {
        click("Add Clue"); change("Clue 1 original text", "first"); change("Clue 2 original text", "second");
        firstName = "Clue 1 original text"; secondName = "Clue 2 original text"; remove = "Remove clue 1";
      } else {
        click("Add hypothesis for clue 1");
        if (kind === "hypothesis") {
          click("Add hypothesis for clue 1"); change("Hypothesis 1 interpretation", "first"); change("Hypothesis 2 interpretation", "second");
          firstName = "Hypothesis 1 interpretation"; secondName = "Hypothesis 2 interpretation"; remove = "Remove hypothesis 1";
        } else {
          click("Add effect to hypothesis 1"); click("Add effect to hypothesis 1");
          firstName = "Hypothesis 1 effect 1 minimum"; secondName = "Hypothesis 1 effect 2 minimum"; remove = "Remove hypothesis 1 effect 1";
        }
      }
    }
    const removed = field(firstName), remaining = field(secondName), accepted = remaining.value;
    fireEvent.compositionStart(removed); change(firstName, kind === "direct" || kind === "effect" ? "22" : "discarded");
    click(remove); fireEvent.compositionEnd(removed); fireEvent.compositionEnd(field(firstName));
    expect(field(firstName)).toBe(remaining); expect(remaining.value).toBe(accepted);
    click("Preview candidates"); await screen.findByText(/Success: preview results received/);
    const request = requests()[0];
    if (kind === "clue") expect(request.clues.map((clue) => clue.text)).toEqual(["second"]);
    else if (kind === "hypothesis") expect(request.hypotheses.map((hypothesis) => hypothesis.interpretation)).toEqual(["second"]);
    else if (kind === "direct") expect(request.directConstraints[0].constraint).toEqual({ type: "length", min: 1, max: 64 });
    else expect(request.hypotheses[0].effects[0].constraint).toEqual({ type: "length", min: 1, max: 64 });
  });

  it.each(["success", "error"] as const)("discards pending stale %s after IME, Clear, delayed end, and a fresh edit", async (outcome) => {
    let resolve!: (value: PreviewResponse) => void;
    let reject!: (reason: unknown) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((yes, no) => { resolve = yes; reject = no; }));
    render(<CandidatePreview />); change(seedName, "accepted"); click("Preview candidates");
    const submitted = requests()[0], oldSeed = field(seedName);
    fireEvent.compositionStart(oldSeed); change(seedName, "intermediate");
    click("Clear inputs and results"); fireEvent.compositionEnd(oldSeed);
    const fresh = field(seedName);
    fireEvent.compositionStart(fresh); change(seedName, "fresh"); fireEvent.compositionEnd(fresh);
    expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    expect(requests()).toHaveLength(1);
    await act(async () => {
      if (outcome === "success") resolve(response(submitted)); else reject("PREVIEW_CORE_FAILED");
    });
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByRole("status", { name: "Preview status" }).textContent).not.toContain("The Core could not complete");
    expect(start().disabled).toBe(false);
    click("Preview candidates"); await screen.findByText(/Success: preview results received/);
    expect(requests().map((request) => request.seeds)).toEqual([["accepted"], ["fresh"]]);
    expect(requests()[1].requestId).not.toBe(submitted.requestId);
  });
});
