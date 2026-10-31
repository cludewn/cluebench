// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, createEvent, fireEvent, render, screen } from "@testing-library/react";
import App from "./App";
import { runCandidatePreview } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";

vi.mock("./core-bridge", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./core-bridge")>()), runCoreProbe: vi.fn(),
}));
vi.mock("./preview-bridge", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./preview-bridge")>()),
  runCandidatePreview: vi.fn(),
}));
afterEach(cleanup);
beforeEach(() => { vi.mocked(runCandidatePreview).mockReset(); });

type InputName = "Seed" | "Prefix" | "Suffix";
const seedBox = () => screen.getByRole("textbox", { name: "Manual seeds — one per line" }) as HTMLTextAreaElement;
const previewButton = () => screen.getByRole("button", { name: "Preview candidates" }) as HTMLButtonElement;
const previewStatus = () => screen.getByRole("status", { name: "Preview status" });
const enterSeed = (value: string) => fireEvent.change(seedBox(), { target: { value } });
function field(name: InputName): HTMLInputElement | HTMLTextAreaElement {
  if (name === "Seed") return seedBox();
  fireEvent.click(screen.getByRole("button", { name: `Add ${name.toLowerCase()}` }));
  return screen.getByRole("textbox", { name: `${name} 1` }) as HTMLInputElement;
}
function transfer(target: Element, route: "paste" | "drop", raw: string) {
  const data = { types: ["text/plain"], getData: (type: string) => type === "text/plain" ? raw : "" };
  const event = route === "paste"
    ? createEvent.paste(target, { bubbles: true, cancelable: true, clipboardData: data })
    : createEvent.drop(target, { bubbles: true, cancelable: true, dataTransfer: data });
  fireEvent(target, event);
  return event;
}
function successfulResponse(request: PreviewRequest): PreviewResponse {
  return {
    requestId: request.requestId, effectiveLimits: request.limits, constraintBindings: [],
    candidates: [{ text: request.seeds[0], seedId: 1, path: { seedIndex: 0, caseIndex: 0,
      caseVariant: request.options.caseVariants[0], prefixIndex: 0, suffixIndex: 0 },
      prefix: request.options.prefixes[0], suffix: request.options.suffixes[0], relativePoints: 0, matchedSoftHypothesisIds: [] }],
    stats: { attemptedPaths: 1, domainSkips: 0, hardRejections: 0, emittedCandidates: 1 },
    potentialPaths: 1, stop: "completed",
  };
}
function assertRejected(event: Event, target: HTMLInputElement | HTMLTextAreaElement) {
  expect(event.defaultPrevented).toBe(true);
  expect(target.value).toBe("kept");
  expect(previewButton().disabled).toBe(true);
  const message = screen.getByRole("alert").textContent;
  expect(message).toMatch(/Input rejected\./);
  expect(message).not.toContain("Private");
  expect(message).not.toContain("Payload");
  expect(runCandidatePreview).not.toHaveBeenCalled();
}

describe("Preview input events before DOM normalization (React component, mock IPC)", () => {
  it.each([
    ["Seed", "Private\rPayload"],
    ["Prefix", "Private\nPayload"],
    ["Suffix", "Private\rPayload"],
    ["Suffix", "Private\nPayload"],
    ["Suffix", "Private\r\nPayload"],
  ] as const)("prevents %s paste with forbidden original line endings #%#", (name, raw) => {
    render(<App />); enterSeed("kept");
    const target = field(name);
    if (name !== "Seed") fireEvent.change(target, { target: { value: "kept" } });
    assertRejected(transfer(target, "paste", raw), target);
    expect(previewStatus().textContent).toContain("State: Invalid");
  });

  it.each([
    ["Seed", "Private\rPayload"],
    ["Prefix", "Private\nPayload"],
    ["Suffix", "Private\r\nPayload"],
  ] as const)("prevents %s drop rather than inserting a normalized alternative", (name, raw) => {
    render(<App />); enterSeed("kept");
    const target = field(name);
    if (name !== "Seed") fireEvent.change(target, { target: { value: "kept" } });
    assertRejected(transfer(target, "drop", raw), target);
  });

  it.each(["Seed", "Prefix", "Suffix"] as const)("cancels native beforeinput carrying invalid raw data for %s", (name) => {
    render(<App />); enterSeed("kept");
    const target = field(name);
    if (name !== "Seed") fireEvent.change(target, { target: { value: "kept" } });
    const event = new InputEvent("beforeinput", {
      bubbles: true, cancelable: true, inputType: "insertText", data: "Private\rPayload",
    });
    fireEvent(target, event);
    assertRejected(event, target);
  });

  it("checks raw beforeinput data even when its dataTransfer text is safe or empty", () => {
    render(<App />); enterSeed("kept");
    for (const transferred of ["", "safe"]) {
      const event = new InputEvent("beforeinput", {
        bubbles: true, cancelable: true, inputType: "insertText", data: "Private\rPayload",
      });
      Object.defineProperty(event, "dataTransfer", {
        value: { getData: (type: string) => type === "text/plain" ? transferred : "" },
      });
      fireEvent(seedBox(), event);
      assertRejected(event, seedBox());
    }
  });

  it("uses a fixed seed error independent of the rejected transfer content", () => {
    render(<App />); enterSeed("kept");
    transfer(seedBox(), "paste", "Private\rPayload");
    const message = screen.getByRole("alert").textContent;
    expect(message).toContain("Input rejected. Use LF or CRLF between seed lines; a lone CR is not allowed.");
    transfer(seedBox(), "paste", "Other\rSecret");
    expect(screen.getByRole("alert").textContent).toBe(message);
    expect(seedBox().value).toBe("kept");
  });

  it("does not accept a normalized value after observed noncancelable invalid beforeinput", () => {
    render(<App />); enterSeed("kept");
    const event = new InputEvent("beforeinput", {
      bubbles: true, cancelable: false, inputType: "insertText", data: "Private\rPayload",
    });
    fireEvent(seedBox(), event);
    expect(event.defaultPrevented).toBe(false);
    // A noncancelable event cannot prevent its browser default. This explicit
    // change models the subsequent sanitized value, which must not be accepted.
    enterSeed("Private\nPayload");
    expect(seedBox().value).toBe("kept");
    expect(screen.getByRole("alert").textContent).not.toContain("Private");
    expect(previewStatus().textContent).toContain("State: Invalid");
    expect(previewButton().disabled).toBe(true);
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });

  it.each([
    ["A\nB", ["A", "B"]],
    ["A\r\nB", ["A", "B"]],
    ["A\n", ["A"]],
    ["A\r\n", ["A"]],
    ["A\nA", ["A", "A"]],
    [" A \n ", [" A ", " "]],
    ["A".repeat(64), ["A".repeat(64)]],
  ])("permits valid seed paste without altering its seed meaning #%#", async (raw, expected) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (request) => successfulResponse(request));
    render(<App />);
    expect(transfer(seedBox(), "paste", raw as string).defaultPrevented).toBe(false);
    // jsdom dispatches paste but does not perform its native insertion. This
    // explicit change models only a known-valid textarea CRLF-to-LF insertion;
    // it is not evidence of browser clipboard/default-action behavior.
    enterSeed((raw as string).replace(/\r\n/g, "\n"));
    expect(previewButton().disabled).toBe(false);
    fireEvent.click(previewButton());
    await screen.findByText(/Success: preview results received/);
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0].seeds).toEqual(expected);
  });

  it("permits a valid seed drop and preserves duplicates and surrounding spaces", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (request) => successfulResponse(request));
    render(<App />);
    const raw = " A \r\n A \r\n";
    expect(transfer(seedBox(), "drop", raw).defaultPrevented).toBe(false);
    // As above, default drop insertion is not implemented by jsdom.
    enterSeed(raw.replace(/\r\n/g, "\n"));
    fireEvent.click(previewButton());
    await screen.findByText(/Success: preview results received/);
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0].seeds).toEqual([" A ", " A "]);
  });

  it.each(["Prefix", "Suffix"] as const)("preserves spaces/case and 64-byte %s paste", async (name) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (request) => successfulResponse(request));
    render(<App />); enterSeed("A");
    const target = field(name);
    const raw = ` MiXeD${" ".repeat(58)}`;
    expect(raw.length).toBe(64);
    expect(transfer(target, "paste", raw).defaultPrevented).toBe(false);
    // Known-valid single-line insertion, explicitly simulated for jsdom.
    fireEvent.change(target, { target: { value: raw } });
    fireEvent.click(previewButton());
    await screen.findByText(/Success: preview results received/);
    const request = vi.mocked(runCandidatePreview).mock.calls[0][0];
    expect(name === "Prefix" ? request.options.prefixes : request.options.suffixes).toEqual(["", raw]);
  });

  it("retains normal direct edits, explicit empty affixes and selected case options", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (request) => successfulResponse(request));
    render(<App />); enterSeed("Direct1!");
    fireEvent.click(screen.getByLabelText("Original"));
    fireEvent.click(screen.getByLabelText("ASCII Uppercase"));
    fireEvent.click(previewButton());
    await screen.findByText(/Success: preview results received/);
    expect(vi.mocked(runCandidatePreview).mock.calls[0][0]).toMatchObject({
      seeds: ["Direct1!"], options: { caseVariants: ["asciiUppercase"], prefixes: [""], suffixes: [""] },
    });
  });

  it("invalidates displayed results after a rejected transfer without changing retained input", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (request) => successfulResponse(request));
    render(<App />); enterSeed("kept"); fireEvent.click(previewButton());
    await screen.findByText(/Success: preview results received/);
    expect(screen.getByRole("table")).toBeTruthy();
    const event = transfer(seedBox(), "paste", "Private\rPayload");
    expect(event.defaultPrevented).toBe(true);
    expect(seedBox().value).toBe("kept");
    expect(screen.queryByRole("table")).toBeNull();
    expect(previewStatus().textContent).toContain("State: Invalid");
    expect(previewButton().disabled).toBe(true);
    enterSeed("revised");
    expect(previewButton().disabled).toBe(false);
  });

  it.each(["success", "error"] as const)("rejects stale %s after blocked input while preserving actual pending work", async (outcome) => {
    let resolve: (value: PreviewResponse) => void = () => {};
    let reject: (value: unknown) => void = () => {};
    vi.mocked(runCandidatePreview).mockReturnValueOnce(new Promise((yes, no) => { resolve = yes; reject = no; }));
    render(<App />); enterSeed("kept"); fireEvent.click(previewButton());
    const request = vi.mocked(runCandidatePreview).mock.calls[0][0];
    expect(transfer(seedBox(), "drop", "Private\rPayload").defaultPrevented).toBe(true);
    expect(seedBox().value).toBe("kept");
    expect(previewStatus().textContent).toContain("State: Loading");
    expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    await act(async () => {
      if (outcome === "success") resolve(successfulResponse(request));
      else reject(new Error("Private request error"));
    });
    expect(previewStatus().textContent).toContain("State: Invalid");
    expect(screen.queryByRole("table")).toBeNull();
    expect(previewStatus().textContent).not.toContain("Private");
    expect(previewButton().disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Clear inputs and results" }));
    enterSeed("fresh");
    expect(previewButton().disabled).toBe(false);
    expect(runCandidatePreview).toHaveBeenCalledTimes(1);
  });

  it("documents DOM setter normalization separately from user transfer-event coverage", () => {
    render(<App />);
    seedBox().value = "A\rB";
    expect(seedBox().value).toBe("A\nB");
    const prefix = field("Prefix");
    prefix.value = "A\nB";
    expect(prefix.value).toBe("AB");
    // No paste, drop or raw beforeinput was dispatched. Once only these
    // sanitized values exist, a component cannot reconstruct the originals.
  });
});
