// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import App from "./App";
import { ProbeError, runCoreProbe } from "./core-bridge";
import { PreviewError, runCandidatePreview } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";

vi.mock("./core-bridge", async (importOriginal) => ({ ...(await importOriginal<typeof import("./core-bridge")>()), runCoreProbe: vi.fn() }));
vi.mock("./preview-bridge", async (importOriginal) => ({ ...(await importOriginal<typeof import("./preview-bridge")>()), runCandidatePreview: vi.fn() }));
afterEach(cleanup);
beforeEach(() => { vi.mocked(runCandidatePreview).mockReset(); vi.mocked(runCoreProbe).mockReset(); });
const start = () => screen.getByRole("button", { name: "Preview candidates" }) as HTMLButtonElement;
const seedBox = () => screen.getByRole("textbox", { name: "Manual seeds — one per line" });
const enter = (text = "Synthetic1!") => fireEvent.change(seedBox(), { target: { value: text } });
const status = () => screen.getByRole("status", { name: "Preview status" });
function result(req: PreviewRequest): PreviewResponse {
  return { requestId: req.requestId, effectiveLimits: req.limits,
    candidates: [{ text: req.seeds[0], seedId: 1, path: { seedIndex: 0, caseIndex: 0, caseVariant: req.options.caseVariants[0], prefixIndex: 0, suffixIndex: 0 }, prefix: req.options.prefixes[0], suffix: req.options.suffixes[0], relativePoints: 0 }],
    stats: { attemptedPaths: 1, domainSkips: 0, hardRejections: 0, emittedCandidates: 1 }, potentialPaths: 1, stop: "completed" };
}
function hold() {
  let resolve: (value: PreviewResponse) => void = () => {};
  let reject: (value: unknown) => void = () => {};
  vi.mocked(runCandidatePreview).mockReturnValueOnce(new Promise((yes, no) => { resolve = yes; reject = no; }));
  return { resolve, reject };
}
const activeRequest = () => vi.mocked(runCandidatePreview).mock.calls.at(-1)![0];

describe("Candidate Preview desktop workflow (mock IPC)", () => {
  it("starts Idle with safe defaults and disabled empty execution", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Candidate Preview" })).toBeTruthy();
    expect(status().textContent).toContain("State: Idle"); expect(start().disabled).toBe(true);
    expect((screen.getByLabelText("Original") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("ASCII Lowercase") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("No prefix") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Attempt limit", { exact: false }) as HTMLInputElement).value).toBe("2048");
    expect((screen.getByLabelText("Emission limit", { exact: false }) as HTMLInputElement).value).toBe("50");
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
    expect(seedBox().getAttribute("spellcheck")).toBe("false");
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });
  it("shows fixed line validation with no seed disclosure or automatic run", () => {
    render(<App />); enter("private-seed\n\n");
    expect(status().textContent).toContain("State: Invalid"); expect(start().disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("Seed line 2");
    expect(screen.getByRole("alert").textContent).not.toContain("private-seed");
    expect(runCandidatePreview).not.toHaveBeenCalled();
  });
  it("preserves spaces, duplicate lines, explicit selected case order and affix order", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => result(req));
    render(<App />); enter(" A \nA\nA\n");
    fireEvent.click(screen.getByLabelText("Original")); fireEvent.click(screen.getByLabelText("ASCII Uppercase")); fireEvent.click(screen.getByLabelText("ASCII Lowercase"));
    fireEvent.click(screen.getByRole("button", { name: "Add prefix" }));
    expect(start().disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Prefix 1" }), { target: { value: "PRE-" } });
    fireEvent.click(screen.getByRole("button", { name: "Add suffix" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Suffix 1" }), { target: { value: " " } });
    fireEvent.click(screen.getByLabelText("No suffix"));
    fireEvent.click(start());
    await screen.findByText(/Success: preview results received/);
    expect(activeRequest()).toMatchObject({ seeds: [" A ", "A", "A"], options: { caseVariants: ["asciiLowercase", "asciiUppercase"], prefixes: ["", "PRE-"], suffixes: [" "] } });
  });
  it("keeps affix row positions and blocks blank/duplicate options", () => {
    render(<App />); enter();
    fireEvent.click(screen.getByRole("button", { name: "Add prefix" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Prefix 1" }), { target: { value: "a" } });
    fireEvent.click(screen.getByRole("button", { name: "Add prefix" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Prefix 2" }), { target: { value: "b" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Prefix 1" }), { target: { value: "" } });
    expect((screen.getByRole("textbox", { name: "Prefix 2" }) as HTMLInputElement).value).toBe("b");
    expect(start().disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Prefix 1" }), { target: { value: "b" } });
    expect(screen.getByRole("alert").textContent).toContain("Prefix options must be distinct");
    fireEvent.click(screen.getByRole("button", { name: "Remove prefix 1" })); expect(start().disabled).toBe(false);
    fireEvent.click(screen.getByLabelText("No suffix")); expect(start().disabled).toBe(true);
  });
  it("rejects numeric allowances without clamping and permits independent limits", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => result(req));
    render(<App />); enter();
    fireEvent.change(screen.getByLabelText("Attempt limit", { exact: false }), { target: { value: "4097" } }); expect(start().disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("1 to 4,096");
    fireEvent.change(screen.getByLabelText("Attempt limit", { exact: false }), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("Emission limit", { exact: false }), { target: { value: "100" } });
    fireEvent.click(start()); await screen.findByText(/Success: preview/);
    expect(activeRequest().limits).toEqual({ attemptLimit: 1, emissionLimit: 100 });
  });
  it("holds Loading until actual promise settlement and renders provenance, statistics and Core stop", async () => {
    const pending = hold(); render(<App />); enter(); fireEvent.click(start());
    expect(status().getAttribute("aria-busy")).toBe("true"); expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    await act(async () => pending.resolve(result(activeRequest())));
    expect(status().textContent).toContain("State: Success"); expect(status().getAttribute("aria-busy")).toBe("false");
    expect(within(screen.getByRole("table")).getByText("Synthetic1!")).toBeTruthy(); expect(screen.getByText(/Seed 1 \(line 1\)/)).toBeTruthy();
    expect(screen.getByText("Path 0/0/0/0")).toBeTruthy(); expect(screen.getByText("Completed")).toBeTruthy();
    expect(screen.getByText("Attempted paths")).toBeTruthy(); expect(screen.getByText("Relative points")).toBeTruthy();
    expect(start().disabled).toBe(false);
  });
  it.each(["attemptLimit", "outputLimit"] as const)("shows Limited separately from Core %s", async (stop) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => ({ ...result(req), stop }));
    render(<App />); enter(); fireEvent.click(start());
    await screen.findByText(/Limited: a configured allowance/); expect(status().textContent).toContain("State: Limited");
    expect(screen.getByText(stop === "attemptLimit" ? "AttemptLimit" : "OutputLimit")).toBeTruthy();
  });
  it.each(["completed", "attemptLimit"] as const)("shows Empty while retaining %s and simultaneous limitation", async (stop) => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => ({ ...result(req), candidates: [], stats: { attemptedPaths: 1, domainSkips: 1, hardRejections: 0, emittedCandidates: 0 }, stop }));
    render(<App />); enter(); fireEvent.click(start());
    await screen.findByText(/Empty: no candidates/); expect(status().textContent).toContain("State: Empty");
    expect(screen.getByText(stop === "completed" ? "Completed" : "AttemptLimit")).toBeTruthy();
    expect(status().textContent.includes("Limited: a configured allowance")).toBe(stop === "attemptLimit");
    expect(status().textContent).toContain("Zero candidates does not prove unsatisfiability");
  });
  it("retains duplicate candidate strings as distinct path records and renders HTML-like seeds as text", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => { const res = result(req); res.candidates.push({ ...res.candidates[0], seedId: 2, path: { ...res.candidates[0].path, seedIndex: 1 } }); res.stats.attemptedPaths = 2; res.stats.emittedCandidates = 2; res.potentialPaths = 2; return res; });
    render(<App />); enter("<b>text</b>\n<b>text</b>"); fireEvent.click(start());
    await screen.findByText(/Success: preview/);
    expect(screen.getAllByText("<b>text</b>")).toHaveLength(2); expect(document.querySelector("tbody b")).toBeNull();
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(3);
  });
  it("sanitizes invocation failures, recovers from PREVIEW_BUSY and gives each retry a new ID", async () => {
    vi.mocked(runCandidatePreview).mockRejectedValueOnce(new Error("private-seed")).mockRejectedValueOnce(new PreviewError("PREVIEW_BUSY")).mockImplementation(async (req) => result(req));
    render(<App />); enter(); fireEvent.click(start());
    await screen.findByText("The candidate preview failed. You can try again."); expect(status().textContent).not.toContain("private-seed");
    const firstId = activeRequest().requestId; fireEvent.click(start()); await screen.findByText(/A preview is still running in Rust/);
    expect(activeRequest().requestId).not.toBe(firstId); fireEvent.click(start()); await screen.findByText(/Success: preview/);
    expect(runCandidatePreview).toHaveBeenCalledTimes(3);
  });
  it("rejects a mismatched response request ID", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => ({ ...result(req), requestId: "old-id" }));
    render(<App />); enter(); fireEvent.click(start()); await screen.findByText("The desktop returned an unexpected preview response.");
  });
  it.each(["success", "error"] as const)("discards stale %s after input revision even when text later matches", async (outcome) => {
    const pending = hold(); render(<App />); enter("A"); fireEvent.click(start()); const req = activeRequest();
    enter("B"); enter("A"); expect(status().textContent).toContain("State: Loading");
    expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    await act(async () => { if (outcome === "success") pending.resolve(result(req)); else pending.reject(new PreviewError("PREVIEW_CORE_FAILED")); });
    expect(status().textContent).toContain("State: Idle"); expect(screen.queryByRole("table")).toBeNull();
    expect(status().textContent).not.toContain("The Core could not complete"); expect(start().disabled).toBe(false);
  });
  it("clears references during a request without declaring Rust completion or permitting overlap", async () => {
    const pending = hold(); render(<App />); enter(); fireEvent.click(start()); const req = activeRequest();
    fireEvent.click(screen.getByRole("button", { name: "Clear inputs and results" })); enter("new-seed");
    expect(status().textContent).toContain("State: Loading"); expect(screen.getByRole("button", { name: "Preview running…" }).hasAttribute("disabled")).toBe(true);
    expect(runCandidatePreview).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve(result(req)));
    expect(screen.queryByText("Synthetic1!")).toBeNull(); expect(start().disabled).toBe(false);
  });
  it("invalidates an existing result on affix edit and Clear resets the editor", async () => {
    vi.mocked(runCandidatePreview).mockImplementation(async (req) => result(req));
    render(<App />); enter(); fireEvent.click(start()); await screen.findByText(/Success: preview/);
    fireEvent.click(screen.getByRole("button", { name: "Add prefix" })); expect(screen.queryByRole("table")).toBeNull(); expect(start().disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Clear inputs and results" })); expect(screen.queryByRole("textbox", { name: "Prefix 1" })).toBeNull(); expect(status().textContent).toContain("State: Idle");
  });
  it.each(["success", "error"] as const)("ignores unmounted %s without affecting a fresh request", async (outcome) => {
    const oldPending = hold(); const oldView = render(<App />); enter("old-seed"); fireEvent.click(start()); const oldReq = activeRequest(); oldView.unmount();
    const nextPending = hold(); render(<StrictMode><App /></StrictMode>); enter("new-seed"); fireEvent.click(start()); const nextReq = activeRequest();
    await act(async () => { if (outcome === "success") oldPending.resolve(result(oldReq)); else oldPending.reject(new Error("private")); });
    expect(status().textContent).toContain("State: Loading"); expect(status().getAttribute("aria-busy")).toBe("true");
    await act(async () => nextPending.resolve(result(nextReq))); expect(status().textContent).toContain("State: Success");
    expect(within(screen.getByRole("table")).getByText("new-seed")).toBeTruthy(); expect(screen.queryByText("old-seed")).toBeNull();
  });
  it("shows Desktop unavailable and leaves retry possible", async () => {
    vi.mocked(runCandidatePreview).mockRejectedValue(new PreviewError("DESKTOP_UNAVAILABLE"));
    render(<App />); enter(); fireEvent.click(start()); await screen.findByText(/Desktop connection unavailable/); expect(start().disabled).toBe(false);
  });
});

describe("Auxiliary Core probe", () => {
  it("holds Loading, then displays actual response and permits retry", async () => {
    let finish: (value: { acceptedCaseEligible: boolean; rejectedCaseEligible: boolean }) => void = () => {};
    vi.mocked(runCoreProbe).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    render(<App />); fireEvent.click(screen.getByRole("button", { name: "Check Core connection" }));
    expect(screen.getByRole("button", { name: "Checking…" }).hasAttribute("disabled")).toBe(true);
    await act(async () => finish({ acceptedCaseEligible: true, rejectedCaseEligible: false }));
    expect(screen.getByText(/Core check completed. Accepted: true; rejected: false/)).toBeTruthy();
    vi.mocked(runCoreProbe).mockRejectedValueOnce(new ProbeError("DESKTOP_UNAVAILABLE"));
    fireEvent.click(screen.getByRole("button", { name: "Check Core connection" })); await screen.findByText(/Desktop connection unavailable/);
  });
});
