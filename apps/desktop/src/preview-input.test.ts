// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { createPreviewRequest, parseSeedLines, utf8Bytes, validatePreviewInput } from "./preview-input";
import type { PreviewInput } from "./preview-input";
const input = (patch: Partial<PreviewInput> = {}): PreviewInput => ({ seedText: "A", caseVariants: ["original"], prefixes: [""], suffixes: [""], attemptLimit: "2048", emissionLimit: "50", ...patch });

describe("Manual preview input", () => {
  it.each([
    ["", []], ["A", ["A"]], ["A\n", ["A"]], ["A\r\n", ["A"]],
    ["A\n\n", ["A", ""]], ["A\nA", ["A", "A"]], [" A ", [" A "]],
    ["A\r\nB\n", ["A", "B"]], ["A\rB", ["A\rB"]], ["  ", ["  "]],
  ])("preserves multiline grammar for %j", (text, expected) => expect(parseSeedLines(text as string)).toEqual(expected));
  it("validates printable ASCII bytes, preserving spaces and duplicates", () => {
    for (const seedText of ["A\nA", " A ", " ", "a".repeat(64), "A\n", "A\r\n"]) expect(validatePreviewInput(input({ seedText }))).toEqual([]);
    for (const seedText of ["", "A\n\n", "a".repeat(65), "é", "\t", "A\rB", "\x7f"]) expect(validatePreviewInput(input({ seedText })).length).toBeGreaterThan(0);
    expect(utf8Bytes("é")).toBe(2);
    expect(validatePreviewInput(input({ seedText: "private-seed\n\n" })).join(" ")).not.toContain("private-seed");
    expect(validatePreviewInput(input({ seedText: "private-seed\n\n" }))).toContain("Seed line 2 must contain 1–64 printable ASCII bytes.");
  });
  it("enforces 64 seeds and 4,096 bytes without clamping", () => {
    expect(validatePreviewInput(input({ seedText: Array(64).fill("a".repeat(64)).join("\n") }))).toEqual([]);
    expect(validatePreviewInput(input({ seedText: Array(65).fill("a").join("\n") }))).toContain("Use at most 64 seed lines.");
    expect(validatePreviewInput(input({ seedText: Array(64).fill("a".repeat(65)).join("\n") }))).toContain("Seed text must total at most 4,096 bytes.");
  });
  it("validates distinct cases and ordered explicit affixes including spaces", () => {
    expect(validatePreviewInput(input({ caseVariants: ["asciiLowercase"], prefixes: ["", " "], suffixes: ["X", "x"] }))).toEqual([]);
    for (const patch of [{ caseVariants: [] }, { caseVariants: ["original", "original"] }, { prefixes: [] }, { prefixes: ["", ""] }, { suffixes: ["a", "a"] }, { prefixes: Array(9).fill("a") }, { suffixes: ["é"] }, { prefixes: ["x".repeat(65)] }]) {
      expect(validatePreviewInput(input(patch as Partial<PreviewInput>)).length).toBeGreaterThan(0);
    }
    const options = Array.from({ length: 8 }, (_, index) => String(index).repeat(64));
    expect(validatePreviewInput(input({ caseVariants: ["original", "asciiLowercase", "asciiUppercase"], prefixes: options, suffixes: options }))).toEqual([]);
  });
  it.each(["0", "-1", "1.5", "NaN", "Infinity", "", " 2", "1e2", "4097"])("rejects invalid attempt allowance %s", (attemptLimit) => {
    expect(validatePreviewInput(input({ attemptLimit }))).toContain("Attempt limit must be an integer from 1 to 4,096.");
  });
  it("validates independent positive emission and attempt allowances", () => {
    expect(validatePreviewInput(input({ attemptLimit: "1", emissionLimit: "100" }))).toEqual([]);
    expect(validatePreviewInput(input({ attemptLimit: "4096", emissionLimit: "1" }))).toEqual([]);
    expect(validatePreviewInput(input({ emissionLimit: "101" }))).toContain("Emission limit must be an integer from 1 to 100.");
  });
  it("builds an explicit DTO in fixed selected-case order without Original injection", () => {
    expect(createPreviewRequest(input({ seedText: " A \nA\nA\n", caseVariants: ["asciiUppercase", "asciiLowercase"], prefixes: ["X", ""], suffixes: [" "] }), "p1")).toEqual({
      requestId: "p1", seeds: [" A ", "A", "A"], options: { caseVariants: ["asciiLowercase", "asciiUppercase"], prefixes: ["X", ""], suffixes: [" "] }, limits: { attemptLimit: 2048, emissionLimit: 50 },
    });
  });
});
