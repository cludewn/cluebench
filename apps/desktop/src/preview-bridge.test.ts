// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { PREVIEW_COMMAND, PreviewError, encodePreviewRequest, isPreviewResponse, runCandidatePreview, safePreviewError } from "./preview-bridge";
import { createDefaultHypothesis } from "./clue-hypotheses";
import { createDefaultDirectConstraint } from "./direct-constraints";
import type { DirectConstraint } from "./direct-constraints";
import type { PreviewResponse } from "./preview-bridge";
import { createPreviewRequest } from "./preview-input";
import type { PreviewInput, PreviewRequest } from "./preview-input";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => { vi.mocked(isTauri).mockReturnValue(true); });
export const request = (): PreviewRequest => ({ clues: [], hypotheses: [], directConstraints: [], requestId: "p1", seeds: ["A"], options: { caseVariants: ["original"], prefixes: [""], suffixes: [""] }, limits: { attemptLimit: 2048, emissionLimit: 50 } });
export const response = (): PreviewResponse => ({ hypothesisBindings: [], constraintBindings: [], requestId: "p1", effectiveLimits: { attemptLimit: 2048, emissionLimit: 50 }, candidates: [{ text: "A", seedId: 1, path: { seedIndex: 0, caseIndex: 0, caseVariant: "original", prefixIndex: 0, suffixIndex: 0 }, prefix: "", suffix: "", relativePoints: 0, matchedSoftHypothesisIds: [] }], stats: { attemptedPaths: 1, domainSkips: 0, hardRejections: 0, emittedCandidates: 1 }, potentialPaths: 1, stop: "completed" });

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

const condition = (editorId: number, overrides: Partial<DirectConstraint> = {}): DirectConstraint => ({ editorId, enabled: true, applicability: "soft", confidence: "high", constraint: { type: "contains", literal: "A" }, ...overrides });
function constrained() {
  const req = request();
  req.directConstraints = [condition(7), condition(42, { applicability: "hard" }), condition(99, { enabled: false })];
  const res = response();
  res.constraintBindings = [{ editorId: 7, hypothesisId: 2 }, { editorId: 42, hypothesisId: 4 }, { editorId: 99, hypothesisId: 6 }];
  res.candidates[0].relativePoints = 3;
  res.candidates[0].matchedSoftHypothesisIds = [2];
  return { req, res };
}

describe("Direct condition request and response provenance", () => {
  it("encodes canonical conditions, preserves spaces, and strips extra nested properties", () => {
    const { req } = constrained();
    req.directConstraints[0] = { ...condition(7), secretExtra: "hidden", constraint: { type: "contains", literal: " A ", secretExtra: "hidden" } } as DirectConstraint;
    const dto = JSON.parse(new TextDecoder().decode(encodePreviewRequest(req)));
    expect(dto.directConstraints[0]).toEqual(condition(7, { constraint: { type: "contains", literal: " A " } }));
    expect(JSON.stringify(dto)).not.toContain("hidden");
  });
  it("rejects invalid disabled rows, duplicates, scalar types and row bounds before IPC", () => {
    const invalid = [
      [condition(1, { enabled: false, constraint: { type: "contains", literal: "" } })],
      [condition(1), condition(1)], [condition(0)], [condition(0x1_0000_0000)],
      Array.from({ length: 17 }, (_, i) => condition(i + 1)),
      [condition(1, { constraint: { type: "length", min: "1", max: 64 } as unknown as DirectConstraint["constraint"] })],
      [condition(1, { constraint: { type: "position", index: 1, allowed: "A".repeat(96) } })],
    ];
    for (const directConstraints of invalid) expect(() => encodePreviewRequest({ ...request(), directConstraints })).toThrowError(new PreviewError("PREVIEW_INVALID_REQUEST"));
  });
  it("independently measures an escaped canonical request at the existing manual-workflow maxima", () => {
    const req = request(); req.requestId = "r".repeat(32);
    req.seeds = Array(64).fill('"'.repeat(64));
    const affixes = Array.from({ length: 8 }, (_, i) => '"'.repeat(61) + [i & 1 ? "\\" : '"', i & 2 ? "\\" : '"', i & 4 ? "\\" : '"'].join(""));
    req.options = { caseVariants: ["original", "asciiLowercase", "asciiUppercase"], prefixes: affixes, suffixes: affixes };
    req.limits = { attemptLimit: 4096, emissionLimit: 100 };
    req.directConstraints = Array.from({ length: 16 }, (_, i) => condition(0xffff_ffff - i, { enabled: false, applicability: "hard", confidence: "medium", constraint: { type: "position", index: 64, allowed: '"'.repeat(95) } }));
    const payload = encodePreviewRequest(req);
    expect(payload.byteLength).toBe(16050);
    expect(payload.byteLength).toBeLessThanOrEqual(16384);
    expect(JSON.parse(new TextDecoder().decode(payload)).directConstraints).toEqual(req.directConstraints);
  });
  it("accepts Core points and enabled Soft IDs with exact bindings including disabled rows", () => {
    const { req, res } = constrained();
    expect(isPreviewResponse(res, req)).toBe(true);
    // Binding correspondence is by identity; transport order is not authoritative.
    res.constraintBindings.reverse();
    expect(isPreviewResponse(res, req)).toBe(true);
  });
  it.each([
    (res: PreviewResponse) => { res.constraintBindings.pop(); },
    (res: PreviewResponse) => { res.constraintBindings[1] = res.constraintBindings[0]; },
    (res: PreviewResponse) => { res.constraintBindings[0].editorId = 8; },
    (res: PreviewResponse) => { res.constraintBindings[0].hypothesisId = 3; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [2, 2]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [4]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [6]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [8]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [0]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [2.5]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = Array(17).fill(2); },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = 49; },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = -1; },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = 1.5; },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = 0; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = []; },
  ])("rejects structurally invalid or unowned Soft provenance #%#", (mutate) => {
    const { req, res } = constrained(); mutate(res);
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it.each([
    ["low", 1, true], ["low", 48, false],
    ["medium", 2, true], ["medium", 1, false], ["medium", 3, false],
    ["high", 3, true], ["high", 2, false],
  ] as const)("validates %s confidence with %i returned points (accepted: %s)", (confidence, relativePoints, accepted) => {
    const { req, res } = constrained(); req.directConstraints[0].confidence = confidence;
    res.candidates[0].relativePoints = relativePoints;
    expect(isPreviewResponse(res, req)).toBe(accepted);
  });
  it("sums mixed confidence weights through reordered bindings and distinct duplicate-valued hypotheses", () => {
    const req = request();
    req.directConstraints = [condition(7, { confidence: "low" }), condition(42, { confidence: "medium" }), condition(99)];
    const res = response();
    res.constraintBindings = [{ editorId: 99, hypothesisId: 6 }, { editorId: 7, hypothesisId: 2 }, { editorId: 42, hypothesisId: 4 }];
    res.candidates[0].matchedSoftHypothesisIds = [6, 2, 4];
    res.candidates[0].relativePoints = 6;
    expect(req.directConstraints.map((row) => row.constraint)).toEqual(Array(3).fill({ type: "contains", literal: "A" }));
    expect(isPreviewResponse(res, req)).toBe(true);
    for (const relativePoints of [1, 5, 7, 48]) {
      res.candidates[0].relativePoints = relativePoints;
      expect(isPreviewResponse(res, req)).toBe(false);
    }
    // A valid total for a subset must not include unmatched hypotheses.
    res.candidates[0].matchedSoftHypothesisIds = [4, 2]; res.candidates[0].relativePoints = 3;
    expect(isPreviewResponse(res, req)).toBe(true);
    res.candidates[0].relativePoints = 6; expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("accepts 16 distinct High-confidence Soft matches at 48 points and rejects 47", () => {
    const req = request(); req.directConstraints = Array.from({ length: 16 }, (_, i) => condition(i + 1));
    const res = response(); res.constraintBindings = req.directConstraints.map((row, i) => ({ editorId: row.editorId, hypothesisId: 2 + 2 * i }));
    res.candidates[0].matchedSoftHypothesisIds = res.constraintBindings.map((binding) => binding.hypothesisId);
    res.candidates[0].relativePoints = 48;
    expect(isPreviewResponse(res, req)).toBe(true);
    res.candidates[0].relativePoints = 47;
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("requires zero points for no matches, including requests with no active Soft conditions", () => {
    const { req, res } = constrained(); res.candidates[0].matchedSoftHypothesisIds = [];
    for (const activeSoft of [true, false]) {
      req.directConstraints[0].enabled = activeSoft;
      res.candidates[0].relativePoints = 0; expect(isPreviewResponse(res, req)).toBe(true);
      res.candidates[0].relativePoints = 1; expect(isPreviewResponse(res, req)).toBe(false);
    }
  });
  it.each([
    ["hard", true], ["soft", false],
  ] as const)("rejects matched IDs with %s applicability and enabled=%s even when their weights sum correctly", (applicability, enabled) => {
    const { req, res } = constrained();
    req.directConstraints[0].applicability = applicability; req.directConstraints[0].enabled = enabled;
    expect(isPreviewResponse(res, req)).toBe(false);
    res.candidates[0].matchedSoftHypothesisIds = []; res.candidates[0].relativePoints = 0;
    expect(isPreviewResponse(res, req)).toBe(true);
  });
  it("checks point arithmetic without re-evaluating whether candidate text matches the conditions", () => {
    const { req, res } = constrained(); req.directConstraints[0].constraint = { type: "contains", literal: "Z" };
    expect(isPreviewResponse(res, req)).toBe(true);
  });
  it.each([1, 3])("validates %i returned points against the submitted snapshot after draft confidence changes", async (relativePoints) => {
    const draft = createDefaultDirectConstraint(7); draft.applicability = "soft"; draft.confidence = "low";
    const input: PreviewInput = { clues: [], hypotheses: [], seedText: "A", caseVariants: ["original"], prefixes: [""], suffixes: [""], attemptLimit: "2048", emissionLimit: "50", directConstraints: [draft] };
    const submitted = createPreviewRequest(input, "p1");
    const res = response(); res.constraintBindings = [{ editorId: 7, hypothesisId: 2 }];
    res.candidates[0].matchedSoftHypothesisIds = [2]; res.candidates[0].relativePoints = relativePoints;
    let finish!: (value: PreviewResponse) => void;
    vi.mocked(invoke).mockReturnValueOnce(new Promise<PreviewResponse>((resolve) => { finish = resolve; }));
    const pending = runCandidatePreview(submitted);
    draft.confidence = "high";
    expect(submitted.directConstraints[0].confidence).toBe("low");
    expect(createPreviewRequest(input, "p1").directConstraints[0].confidence).toBe("high");
    finish(res);
    if (relativePoints === 1) await expect(pending).resolves.toEqual(res);
    else await expect(pending).rejects.toMatchObject({ code: "INVALID_RESPONSE", message: "The desktop returned an unexpected preview response." });
  });
  it("rejects inconsistent points with the fixed error without exposing literals or raw response data", async () => {
    const { req, res } = constrained(); req.directConstraints[0].confidence = "low";
    req.directConstraints[0].constraint = { type: "contains", literal: "PrivateSyntheticLiteral" };
    res.candidates[0].relativePoints = 48;
    vi.mocked(invoke).mockResolvedValueOnce(res);
    await expect(runCandidatePreview(req)).rejects.toMatchObject({ code: "INVALID_RESPONSE", message: "The desktop returned an unexpected preview response." });
  });
  it.each(["completed", "attemptLimit", "outputLimit"] as const)("accepts Hard rejections with consistent %s counters", (stop) => {
    const { req, res } = constrained(); req.seeds = ["A", "B", "C"]; req.limits = { attemptLimit: stop === "attemptLimit" ? 2 : 2048, emissionLimit: stop === "outputLimit" ? 1 : 50 };
    res.effectiveLimits = req.limits; res.potentialPaths = 3; res.stop = stop;
    res.constraintBindings = [{ editorId: 7, hypothesisId: 4 }, { editorId: 42, hypothesisId: 6 }, { editorId: 99, hypothesisId: 8 }];
    res.candidates[0].matchedSoftHypothesisIds = [4];
    res.stats = { attemptedPaths: stop === "completed" ? 3 : 2, domainSkips: 0, hardRejections: stop === "completed" ? 2 : 1, emittedCandidates: 1 };
    expect(isPreviewResponse(res, req)).toBe(true);
    req.directConstraints[1].enabled = false;
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("accepts Empty with all paths Hard-rejected without treating it as a solver result", () => {
    const { req, res } = constrained(); res.candidates = []; res.stats.hardRejections = 1; res.stats.emittedCandidates = 0;
    expect(isPreviewResponse(res, req)).toBe(true);
  });
  it("requires AttemptLimit when both budgets are reached before the final path", () => {
    const req = request(); req.seeds = ["A", "B"]; req.limits = { attemptLimit: 1, emissionLimit: 1 };
    const res = response(); res.potentialPaths = 2; res.effectiveLimits = req.limits; res.stop = "outputLimit";
    expect(isPreviewResponse(res, req)).toBe(false);
    res.stop = "attemptLimit"; expect(isPreviewResponse(res, req)).toBe(true);
  });
  it("rejects duplicate or out-of-order emitted paths while preserving duplicate text", () => {
    const req = request(); req.seeds = ["A", "A"];
    const res = response(); const first = res.candidates[0];
    res.candidates.push({ ...first, seedId: 2, path: { ...first.path, seedIndex: 1 } });
    res.stats.attemptedPaths = 2; res.stats.emittedCandidates = 2; res.potentialPaths = 2;
    expect(isPreviewResponse(res, req)).toBe(true);
    res.candidates.reverse(); expect(isPreviewResponse(res, req)).toBe(false);
    res.candidates[1] = res.candidates[0]; expect(isPreviewResponse(res, req)).toBe(false);
  });
});

function clueBacked() {
  const { req, res } = constrained();
  req.clues = [{ editorId: 100, text: "  e\u0301😀\n\t ", source: "synthetic\nsource" }];
  req.hypotheses = [
    { ...createDefaultHypothesis(101, 100), interpretation: "submitted", applicability: "soft", confidence: "medium", effects: [
      { editorId: 102, constraint: { type: "contains", literal: "A" } },
      { editorId: 103, constraint: { type: "suffix", literal: "A" } },
    ] },
    { ...createDefaultHypothesis(104, 100), enabled: false, effects: [] },
    { ...createDefaultHypothesis(105, 100), applicability: "soft", effects: [] },
  ];
  res.hypothesisBindings = [{ editorId: 101, hypothesisId: 9 }, { editorId: 104, hypothesisId: 12 }, { editorId: 105, hypothesisId: 13 }];
  res.candidates[0].matchedSoftHypothesisIds = [2, 9]; res.candidates[0].relativePoints = 5;
  return { req, res };
}

describe("Clue-backed request snapshots and response provenance", () => {
  it("encodes canonical nested evidence and multiple effects, retaining Unicode without response duplication", () => {
    const { req, res } = clueBacked();
    const dto = JSON.parse(new TextDecoder().decode(encodePreviewRequest(req)));
    expect(dto).toEqual(req);
    expect(isPreviewResponse(res, req)).toBe(true);
    res.hypothesisBindings.reverse();
    expect(isPreviewResponse(res, req)).toBe(true);
    expect(JSON.stringify(res)).not.toContain("submitted");
  });
  it.each([
    (res: PreviewResponse) => { res.hypothesisBindings.pop(); },
    (res: PreviewResponse) => { res.hypothesisBindings[1] = res.hypothesisBindings[0]; },
    (res: PreviewResponse) => { res.hypothesisBindings[0].hypothesisId = 8; },
    (res: PreviewResponse) => { res.hypothesisBindings[0].hypothesisId = 2; },
    (res: PreviewResponse) => { res.hypothesisBindings[0].editorId = 102; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [9, 9]; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [12]; res.candidates[0].relativePoints = 2; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [13]; res.candidates[0].relativePoints = 2; },
    (res: PreviewResponse) => { res.candidates[0].matchedSoftHypothesisIds = [10]; res.candidates[0].relativePoints = 2; },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = 7; },
    (res: PreviewResponse) => { res.candidates[0].relativePoints = 73; },
  ])("rejects incomplete, duplicate, wrong-owner and impossible score provenance #%#", (mutate) => {
    const { req, res } = clueBacked(); mutate(res);
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("considers clue-backed Hard effects for rejection counters but excludes zero-effect Hard", () => {
    const { req, res } = clueBacked();
    req.directConstraints.forEach((row) => { row.enabled = false; });
    req.hypotheses[0].applicability = "hard";
    res.candidates = []; res.stats.emittedCandidates = 0; res.stats.hardRejections = 1;
    expect(isPreviewResponse(res, req)).toBe(true);
    req.hypotheses[0].effects = [];
    res.hypothesisBindings = [{ editorId: 101, hypothesisId: 9 }, { editorId: 104, hypothesisId: 10 }, { editorId: 105, hypothesisId: 11 }];
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("counts a multi-effect Medium match once without evaluating candidate constraints", () => {
    const { req, res } = clueBacked();
    req.hypotheses[0].effects[0].constraint = { type: "contains", literal: "Z" };
    expect(isPreviewResponse(res, req)).toBe(true);
    res.candidates[0].relativePoints = 7;
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("accepts 24 High hypotheses and 72 points with exact allocation through Core ID 128", () => {
    const req = request(); req.seeds = Array(64).fill("A");
    req.directConstraints = Array.from({ length: 16 }, (_, i) => condition(i + 1));
    req.clues = Array.from({ length: 8 }, (_, i) => ({ editorId: 17 + i, text: "", source: "" }));
    req.hypotheses = Array.from({ length: 8 }, (_, i) => ({ ...createDefaultHypothesis(25 + i, 17 + i), applicability: "soft", confidence: "high",
      effects: [{ editorId: 33 + i * 2, constraint: { type: "length", min: 1, max: 64 } }, { editorId: 34 + i * 2, constraint: { type: "length", min: 1, max: 64 } }],
    }));
    const res = response();
    res.potentialPaths = 64; res.stats.attemptedPaths = 64; res.stats.emittedCandidates = 64;
    res.effectiveLimits.emissionLimit = req.limits.emissionLimit = 100;
    res.constraintBindings = req.directConstraints.map((row, i) => ({ editorId: row.editorId, hypothesisId: 65 + i * 2 }));
    res.hypothesisBindings = req.hypotheses.map((row, i) => ({ editorId: row.editorId, hypothesisId: 105 + i * 3 }));
    res.candidates = Array.from({ length: 64 }, (_, i) => ({ ...response().candidates[0], seedId: i + 1, path: { ...response().candidates[0].path, seedIndex: i },
      matchedSoftHypothesisIds: [...res.constraintBindings, ...res.hypothesisBindings].map((binding) => binding.hypothesisId), relativePoints: 72,
    }));
    expect(isPreviewResponse(res, req)).toBe(true);
    res.candidates[0].matchedSoftHypothesisIds.push(128);
    expect(isPreviewResponse(res, req)).toBe(false);
  });
  it("rejects invalid references, global editor collisions and malformed Unicode before invocation", async () => {
    for (const mutate of [
      (req: PreviewRequest) => { req.hypotheses[0].clueEditorId = 102; },
      (req: PreviewRequest) => { req.hypotheses[0].effects[0].editorId = 7; },
      (req: PreviewRequest) => { req.clues[0].text = "x\r\ny"; },
      (req: PreviewRequest) => { req.hypotheses[0].interpretation = "\ud800"; },
      (req: PreviewRequest) => { req.hypotheses[1].effects.push({ editorId: 106, constraint: { type: "contains", literal: "" } }); },
    ]) {
      const { req } = clueBacked(); mutate(req);
      await expect(runCandidatePreview(req)).rejects.toMatchObject({ code: "PREVIEW_INVALID_REQUEST" });
    }
    expect(invoke).not.toHaveBeenCalled();
  });
});

function exactBudgetRequest(bytes: number): PreviewRequest {
  const req = request(); req.limits = { attemptLimit: 4096, emissionLimit: 100 };
  req.seeds = Array(64).fill("A".repeat(64));
  req.directConstraints = Array.from({ length: 16 }, (_, i) => condition(i + 1, { constraint: { type: "position", index: 1, allowed: '"'.repeat(95) } }));
  req.clues = [{ editorId: 17, text: "", source: "" }];
  req.hypotheses = Array.from({ length: 4 }, (_, i) => ({ ...createDefaultHypothesis(18 + i, 17), applicability: "soft", confidence: "high",
    effects: Array.from({ length: 4 }, (_, j) => ({ editorId: 30 + i * 4 + j, constraint: { type: "position", index: 1, allowed: '"'.repeat(95) } })),
  }));
  const extraEscapes = bytes - new TextEncoder().encode(JSON.stringify(req)).byteLength;
  expect(extraEscapes).toBeGreaterThan(0); expect(extraEscapes).toBeLessThan(4096);
  req.seeds = req.seeds.map((seed, i) => '"'.repeat(Math.max(0, Math.min(64, extraEscapes - i * 64))) + seed.slice(Math.max(0, Math.min(64, extraEscapes - i * 64))));
  expect(new TextEncoder().encode(JSON.stringify(req)).byteLength).toBe(bytes);
  return req;
}

describe("Shared canonical request budget", () => {
  it("measures actual JSON escaping at exactly 16,384 and rejects 16,385 bytes without trimming", () => {
    const exact = exactBudgetRequest(16384);
    expect(encodePreviewRequest(exact).byteLength).toBe(16384);
    const oversized = exactBudgetRequest(16385);
    const before = JSON.stringify(oversized);
    expect(() => encodePreviewRequest(oversized)).toThrowError(new PreviewError("PREVIEW_REQUEST_TOO_LARGE"));
    expect(JSON.stringify(oversized)).toBe(before);
  });
});
