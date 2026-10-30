// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { PREVIEW_COMMAND, PreviewError, encodePreviewRequest, runCandidatePreview, safePreviewError } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import type { PreviewRequest } from "./preview-input";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => { vi.mocked(isTauri).mockReturnValue(true); });
export const request = (): PreviewRequest => ({ requestId: "p1", seeds: ["A"], options: { caseVariants: ["original"], prefixes: [""], suffixes: [""] }, limits: { attemptLimit: 2048, emissionLimit: 50 } });
export const response = (): PreviewResponse => ({ requestId: "p1", effectiveLimits: { attemptLimit: 2048, emissionLimit: 50 }, candidates: [{ text: "A", seedId: 1, path: { seedIndex: 0, caseIndex: 0, caseVariant: "original", prefixIndex: 0, suffixIndex: 0 }, prefix: "", suffix: "", relativePoints: 0 }], stats: { attemptedPaths: 1, domainSkips: 0, hardRejections: 0, emittedCandidates: 1 }, potentialPaths: 1, stop: "completed" });

describe("Preview raw IPC bridge", () => {
  it("sends explicitly encoded UTF-8 Uint8Array, returning a validated real-shaped DTO", async () => {
    const req = request(); req.seeds = [' "\\ '];
    const result = response(); result.candidates[0].text = ' "\\ ';
    vi.mocked(invoke).mockResolvedValue(result);
    await expect(runCandidatePreview(req)).resolves.toEqual(result);
    const [command, payload] = vi.mocked(invoke).mock.calls[0];
    expect(command).toBe(PREVIEW_COMMAND); expect(ArrayBuffer.isView(payload)).toBe(true); expect(payload?.constructor.name).toBe("Uint8Array");
    expect(JSON.parse(new TextDecoder().decode(payload as Uint8Array))).toEqual(req);
    expect((payload as Uint8Array).byteLength).toBeLessThanOrEqual(16384);
  });
  it("does not forward unrelated object properties", () => {
    expect(new TextDecoder().decode(encodePreviewRequest({ ...request(), secretExtra: "hidden" } as PreviewRequest))).not.toContain("hidden");
  });
  it("rejects invalid input before invocation, without exposing contents", async () => {
    for (const req of [{ ...request(), requestId: "é" }, { ...request(), seeds: ["private\nsecret"] }, { ...request(), seeds: Array(65).fill("A") }, { ...request(), seeds: [""] }, { ...request(), limits: { attemptLimit: 4097, emissionLimit: 1 } }]) {
      await expect(runCandidatePreview(req)).rejects.toMatchObject({ code: "PREVIEW_INVALID_REQUEST" });
    }
    expect(invoke).not.toHaveBeenCalled();
  });
  it("safely rejects non-Tauri execution without a fake result", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(runCandidatePreview(request())).rejects.toMatchObject({ code: "DESKTOP_UNAVAILABLE" });
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each(["PREVIEW_BUSY", "PREVIEW_INVALID_BODY", "PREVIEW_REQUEST_TOO_LARGE", "PREVIEW_INVALID_REQUEST", "PREVIEW_CORE_FAILED", "PREVIEW_WORKER_FAILED", "PREVIEW_RESPONSE_TOO_LARGE"])("maps fixed Rust error %s", async (code) => {
    vi.mocked(invoke).mockRejectedValue(code);
    await expect(runCandidatePreview(request())).rejects.toMatchObject({ code });
  });
  it("sanitizes arbitrary secret-bearing errors and overwritten Error messages", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("private-seed"));
    await expect(runCandidatePreview(request())).rejects.toMatchObject({ code: "INVOCATION_FAILED", message: "The candidate preview failed. You can try again." });
    const error = new PreviewError("PREVIEW_BUSY"); error.message = "private-seed";
    expect(safePreviewError(error)).not.toContain("private-seed");
    expect(safePreviewError({ message: "private-seed" })).not.toContain("private-seed");
  });
  it.each([
    () => null,
    () => ({ ...response(), requestId: "old" }),
    () => ({ ...response(), extra: "private" }),
    () => ({ ...response(), candidates: [{ ...response().candidates[0], text: "<script>safe</script>", extra: "private" }] }),
    () => ({ ...response(), stats: { ...response().stats, emittedCandidates: 2 } }),
    () => ({ ...response(), stats: { ...response().stats, attemptedPaths: 2 } }),
    () => ({ ...response(), effectiveLimits: { attemptLimit: 1, emissionLimit: 50 } }),
    () => ({ ...response(), potentialPaths: 2 }),
    () => ({ ...response(), stop: "cancelled" }),
    () => ({ ...response(), stop: "consumerStopped" }),
    () => ({ ...response(), stop: "outputLimit" }),
    () => ({ ...response(), candidates: [{ ...response().candidates[0], seedId: 0 }] }),
    () => ({ ...response(), candidates: [{ ...response().candidates[0], prefix: "wrong" }] }),
    () => ({ ...response(), candidates: [{ ...response().candidates[0], relativePoints: 1 }] }),
    () => ({ ...response(), candidates: [{ ...response().candidates[0], text: "é" }] }),
  ])("rejects invalid, mismatched or impossible response #%#", async (makeResponse) => {
    vi.mocked(invoke).mockResolvedValue(makeResponse());
    await expect(runCandidatePreview(request())).rejects.toMatchObject({ code: "INVALID_RESPONSE" });
  });
  it("accepts exact final-path Completed at the emission boundary", async () => {
    const req = request(); req.limits.emissionLimit = 1;
    const result = response(); result.effectiveLimits.emissionLimit = 1;
    vi.mocked(invoke).mockResolvedValue(result);
    await expect(runCandidatePreview(req)).resolves.toMatchObject({ stop: "completed" });
  });
  it("accepts zero candidates with an independent attempt-limit stop", async () => {
    const req = request(); req.seeds = ["A".repeat(64)]; req.options.suffixes = ["X", "Y"]; req.limits.attemptLimit = 1;
    const result = response(); result.candidates = []; result.effectiveLimits.attemptLimit = 1;
    result.stats = { attemptedPaths: 1, domainSkips: 1, hardRejections: 0, emittedCandidates: 0 }; result.potentialPaths = 2; result.stop = "attemptLimit";
    vi.mocked(invoke).mockResolvedValue(result);
    await expect(runCandidatePreview(req)).resolves.toEqual(result);
  });
});
