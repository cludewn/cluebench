// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

export const PREVIEW_LIMITS = {
  requestBytes: 16 * 1024,
  requestIdBytes: 32,
  seeds: 64,
  seedBytes: 64,
  totalSeedBytes: 4096,
  caseOptions: 3,
  affixOptions: 8,
  affixBytes: 64,
  configBytes: 1043,
  defaultAttempts: 2048,
  maxAttempts: 4096,
  defaultEmissions: 50,
  maxEmissions: 100,
  responseBytes: 64 * 1024,
} as const;

export const CASE_OPTIONS = [
  { value: "original", label: "Original" },
  { value: "asciiLowercase", label: "ASCII Lowercase" },
  { value: "asciiUppercase", label: "ASCII Uppercase" },
] as const;
export type CaseVariant = (typeof CASE_OPTIONS)[number]["value"];
export interface PreviewRequest {
  requestId: string;
  seeds: string[];
  options: { caseVariants: CaseVariant[]; prefixes: string[]; suffixes: string[] };
  limits: { attemptLimit: number; emissionLimit: number };
}
export interface PreviewInput {
  seedText: string;
  caseVariants: CaseVariant[];
  prefixes: string[];
  suffixes: string[];
  attemptLimit: string;
  emissionLimit: string;
}

const encoder = new TextEncoder();
export const utf8Bytes = (value: string): number => encoder.encode(value).byteLength;
export const isPrintableAscii = (value: string): boolean => /^[\x20-\x7e]*$/.test(value);

/** Remove exactly one final line terminator; never trim or normalize seed text. */
export function parseSeedLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split(/\r\n|\n/);
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

export function validatePreviewInput(input: PreviewInput): string[] {
  const errors: string[] = [];
  const seeds = parseSeedLines(input.seedText);
  if (seeds.length === 0) errors.push("Enter at least one manual seed.");
  if (seeds.length > PREVIEW_LIMITS.seeds) errors.push("Use at most 64 seed lines.");
  seeds.slice(0, PREVIEW_LIMITS.seeds).forEach((seed, index) => {
    if (utf8Bytes(seed) < 1 || utf8Bytes(seed) > PREVIEW_LIMITS.seedBytes || !isPrintableAscii(seed)) {
      errors.push(`Seed line ${index + 1} must contain 1–64 printable ASCII bytes.`);
    }
  });
  if (seeds.reduce((sum, seed) => sum + utf8Bytes(seed), 0) > PREVIEW_LIMITS.totalSeedBytes) {
    errors.push("Seed text must total at most 4,096 bytes.");
  }
  if (input.caseVariants.length < 1 || input.caseVariants.length > 3 ||
    new Set(input.caseVariants).size !== input.caseVariants.length ||
    input.caseVariants.some((value) => !CASE_OPTIONS.some((option) => option.value === value))) {
    errors.push("Select 1–3 distinct case variants.");
  }
  for (const [name, values] of [["Prefix", input.prefixes], ["Suffix", input.suffixes]] as const) {
    if (values.length < 1 || values.length > PREVIEW_LIMITS.affixOptions) {
      errors.push(`${name} options must contain 1–8 entries.`);
    }
    if (new Set(values).size !== values.length) errors.push(`${name} options must be distinct.`);
    values.slice(0, PREVIEW_LIMITS.affixOptions).forEach((value, index) => {
      if (utf8Bytes(value) > PREVIEW_LIMITS.affixBytes || !isPrintableAscii(value)) {
        errors.push(`${name} option ${index + 1} must contain 0–64 printable ASCII bytes.`);
      }
    });
  }
  const configBytes = input.caseVariants.length + 16 +
    [...input.prefixes, ...input.suffixes].reduce((sum, value) => sum + utf8Bytes(value), 0);
  if (configBytes > PREVIEW_LIMITS.configBytes) errors.push("Generation options exceed 1,043 logical bytes.");
  for (const [name, value, max] of [
    ["Attempt limit", input.attemptLimit, PREVIEW_LIMITS.maxAttempts],
    ["Emission limit", input.emissionLimit, PREVIEW_LIMITS.maxEmissions],
  ] as const) {
    if (!/^[0-9]+$/.test(value) || !Number.isSafeInteger(Number(value)) || Number(value) < 1 || Number(value) > max) {
      errors.push(`${name} must be an integer from 1 to ${max.toLocaleString("en-US")}.`);
    }
  }
  return errors;
}

export function createPreviewRequest(input: PreviewInput, requestId: string): PreviewRequest {
  return {
    requestId,
    seeds: parseSeedLines(input.seedText),
    options: {
      caseVariants: CASE_OPTIONS.filter((option) => input.caseVariants.includes(option.value)).map((option) => option.value),
      prefixes: [...input.prefixes],
      suffixes: [...input.suffixes],
    },
    limits: { attemptLimit: Number(input.attemptLimit), emissionLimit: Number(input.emissionLimit) },
  };
}
