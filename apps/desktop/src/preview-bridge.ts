// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { PREVIEW_LIMITS, isPrintableAscii, utf8Bytes, validatePreviewInput } from "./preview-input";
import type { CaseVariant, PreviewRequest } from "./preview-input";

export const PREVIEW_COMMAND = "candidate_preview";
export type GenerationStop = "completed" | "attemptLimit" | "outputLimit" | "cancelled" | "consumerStopped";
export interface PreviewCandidate {
  text: string;
  seedId: number;
  path: { seedIndex: number; caseIndex: number; caseVariant: CaseVariant; prefixIndex: number; suffixIndex: number };
  prefix: string;
  suffix: string;
  relativePoints: number;
}
export interface PreviewResponse {
  requestId: string;
  effectiveLimits: PreviewRequest["limits"];
  candidates: PreviewCandidate[];
  stats: { attemptedPaths: number; domainSkips: number; hardRejections: number; emittedCandidates: number };
  potentialPaths: number;
  stop: GenerationStop;
}
const messages = {
  DESKTOP_UNAVAILABLE: "Desktop connection unavailable. Open the native Cluebench application to preview candidates.",
  PREVIEW_INVALID_BODY: "The desktop rejected the preview payload format.",
  PREVIEW_REQUEST_TOO_LARGE: "The preview request exceeds 16 KiB. Reduce the input.",
  PREVIEW_INVALID_REQUEST: "The preview input is invalid. Check the seeds, options, and limits.",
  PREVIEW_BUSY: "A preview is still running in Rust. Wait for it to finish, then try again.",
  PREVIEW_CORE_FAILED: "The Core could not complete this preview. You can try again.",
  PREVIEW_WORKER_FAILED: "The preview worker failed. You can try again.",
  PREVIEW_RESPONSE_TOO_LARGE: "The preview response exceeds 64 KiB. Reduce the emission limit.",
  INVALID_RESPONSE: "The desktop returned an unexpected preview response.",
  INVOCATION_FAILED: "The candidate preview failed. You can try again.",
} as const;
export type PreviewErrorCode = keyof typeof messages;
export class PreviewError extends Error {
  constructor(readonly code: PreviewErrorCode) {
    super(messages[code]);
    this.name = "PreviewError";
  }
}
export function safePreviewError(error: unknown): string {
  return new PreviewError(error instanceof PreviewError && Object.hasOwn(messages, error.code) ? error.code : "INVOCATION_FAILED").message;
}
const object = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const fields = (value: unknown, keys: string[]): value is Record<string, unknown> => object(value) && Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
const integer = (value: unknown, min: number, max: number): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= min && value <= max;
const text = (value: unknown, min = 0): value is string => typeof value === "string" && utf8Bytes(value) >= min && utf8Bytes(value) <= 64 && isPrintableAscii(value);

export function encodePreviewRequest(request: PreviewRequest): Uint8Array {
  if (typeof request.requestId !== "string" || !/^[A-Za-z0-9_-]{1,32}$/.test(request.requestId) || validatePreviewInput({
    seedText: request.seeds.join("\n"), caseVariants: request.options.caseVariants,
    prefixes: request.options.prefixes, suffixes: request.options.suffixes,
    attemptLimit: String(request.limits.attemptLimit), emissionLimit: String(request.limits.emissionLimit),
  }).length > 0 || request.seeds.some((seed) => seed.length === 0 || /[\r\n]/.test(seed))) {
    throw new PreviewError("PREVIEW_INVALID_REQUEST");
  }
  // Construct the boundary DTO explicitly; extra object properties are never forwarded.
  const payload = new TextEncoder().encode(JSON.stringify({
    requestId: request.requestId, seeds: request.seeds,
    options: { caseVariants: request.options.caseVariants, prefixes: request.options.prefixes, suffixes: request.options.suffixes },
    limits: { attemptLimit: request.limits.attemptLimit, emissionLimit: request.limits.emissionLimit },
  }));
  if (payload.byteLength > PREVIEW_LIMITS.requestBytes) throw new PreviewError("PREVIEW_REQUEST_TOO_LARGE");
  return payload;
}

export function isPreviewResponse(value: unknown, request: PreviewRequest): value is PreviewResponse {
  if (!fields(value, ["requestId", "effectiveLimits", "candidates", "stats", "potentialPaths", "stop"]) || value.requestId !== request.requestId ||
    !fields(value.effectiveLimits, ["attemptLimit", "emissionLimit"]) || value.effectiveLimits.attemptLimit !== request.limits.attemptLimit ||
    value.effectiveLimits.emissionLimit !== request.limits.emissionLimit || !Array.isArray(value.candidates) || value.candidates.length > 100 ||
    !fields(value.stats, ["attemptedPaths", "domainSkips", "hardRejections", "emittedCandidates"]) ||
    !Object.values(value.stats).every((count) => integer(count, 0, request.limits.attemptLimit)) ||
    value.stats.hardRejections !== 0 || value.stats.emittedCandidates !== value.candidates.length ||
    value.candidates.length > request.limits.emissionLimit ||
    value.stats.attemptedPaths !== (value.stats.domainSkips as number) + (value.stats.hardRejections as number) + (value.stats.emittedCandidates as number) ||
    !integer(value.potentialPaths, 0, 12288) || (value.stats.attemptedPaths as number) > value.potentialPaths ||
    value.potentialPaths !== request.seeds.length * request.options.caseVariants.length * request.options.prefixes.length * request.options.suffixes.length ||
    typeof value.stop !== "string" || !["completed", "attemptLimit", "outputLimit"].includes(value.stop)) return false;
  // Validate impossible one-shot responses; never synthesize or replace the Core stop reason.
  if ((value.stop === "completed" && value.stats.attemptedPaths !== value.potentialPaths) ||
    (value.stop !== "completed" && (value.stats.attemptedPaths as number) >= value.potentialPaths) ||
    (value.stop === "attemptLimit" && value.stats.attemptedPaths !== request.limits.attemptLimit) ||
    (value.stop === "outputLimit" && value.stats.emittedCandidates !== request.limits.emissionLimit)) return false;
  const paths = new Set<string>();
  for (const candidate of value.candidates) {
    if (!fields(candidate, ["text", "seedId", "path", "prefix", "suffix", "relativePoints"]) || !text(candidate.text, 1) ||
      !fields(candidate.path, ["seedIndex", "caseIndex", "caseVariant", "prefixIndex", "suffixIndex"]) ||
      !integer(candidate.path.seedIndex, 0, request.seeds.length - 1) || candidate.seedId !== candidate.path.seedIndex + 1 ||
      !integer(candidate.path.caseIndex, 0, request.options.caseVariants.length - 1) ||
      candidate.path.caseVariant !== request.options.caseVariants[candidate.path.caseIndex] ||
      !integer(candidate.path.prefixIndex, 0, request.options.prefixes.length - 1) ||
      !integer(candidate.path.suffixIndex, 0, request.options.suffixes.length - 1) ||
      !text(candidate.prefix) || !text(candidate.suffix) || candidate.prefix !== request.options.prefixes[candidate.path.prefixIndex] ||
      candidate.suffix !== request.options.suffixes[candidate.path.suffixIndex] || candidate.relativePoints !== 0) return false;
    const key = [candidate.path.seedIndex, candidate.path.caseIndex, candidate.path.prefixIndex, candidate.path.suffixIndex].join(":");
    if (paths.has(key)) return false;
    paths.add(key);
  }
  try { return utf8Bytes(JSON.stringify(value)) <= PREVIEW_LIMITS.responseBytes; } catch { return false; }
}

export async function runCandidatePreview(request: PreviewRequest): Promise<PreviewResponse> {
  const payload = encodePreviewRequest(request);
  if (!isTauri()) throw new PreviewError("DESKTOP_UNAVAILABLE");
  let response: unknown;
  try {
    response = await invoke<unknown>(PREVIEW_COMMAND, payload);
  } catch (error: unknown) {
    // Never expose a raw Rust/Serde/transport exception or its potentially secret contents.
    if (typeof error === "string" && Object.hasOwn(messages, error)) throw new PreviewError(error as PreviewErrorCode);
    throw new PreviewError("INVOCATION_FAILED");
  }
  if (!isPreviewResponse(response, request)) throw new PreviewError("INVALID_RESPONSE");
  return response;
}
