// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { canonicalDirectConstraints, changeConstraintType, createDefaultDirectConstraint, describeDirectConstraint, serializeDirectConstraints, validateDirectConstraints } from "./direct-constraints";
import type { DirectConstraint, DirectConstraintDraft } from "./direct-constraints";

const row = (constraint: DirectConstraintDraft["constraint"]): DirectConstraintDraft => ({ ...createDefaultDirectConstraint(1), constraint });

describe("Direct condition authoring validation (no matching evaluator)", () => {
  it.each([
    { type: "length", min: "1", max: "64" },
    { type: "position", index: "64", allowed: " " + "A".repeat(94) },
    { type: "contains", literal: " MiXeD " },
    { type: "notContains", literal: "a".repeat(64) },
    { type: "prefix", literal: " " },
    { type: "suffix", literal: "a" },
    { type: "characterCount", class: "space", min: "0", max: "64" },
  ] as const)("accepts the %s canonical boundary", (constraint) => {
    const draft = row(constraint);
    expect(validateDirectConstraints([draft])).toEqual([]);
    const serialized = serializeDirectConstraints([draft])[0];
    expect(serialized.editorId).toBe(1);
    expect(serialized.constraint.type).toBe(constraint.type);
    expect(canonicalDirectConstraints([serialized])).toEqual([serialized]);
  });

  it.each(["", " ", " 1", "1 ", "1x", "1.0", "1e1", "+1", "-1", "1\n", "0x10", "Infinity", "9".repeat(500)])("rejects the whole invalid numeric draft #%# without repair", (value) => {
    const draft = row({ type: "length", min: value, max: "64" });
    expect(validateDirectConstraints([draft]).length).toBeGreaterThan(0);
    expect(() => serializeDirectConstraints([draft])).toThrow("Invalid direct conditions.");
    expect(draft.constraint).toEqual({ type: "length", min: value, max: "64" });
  });

  it.each([
    { type: "length", min: "0", max: "64" },
    { type: "length", min: "2", max: "1" },
    { type: "length", min: "1", max: "65" },
    { type: "position", index: "0", allowed: "A" },
    { type: "position", index: "65", allowed: "A" },
    { type: "position", index: "1", allowed: "A".repeat(96) },
    { type: "position", index: "1", allowed: "" },
    { type: "contains", literal: "a".repeat(65) },
    { type: "notContains", literal: "" },
    { type: "prefix", literal: "private\rpayload" },
    { type: "suffix", literal: "é" },
    { type: "characterCount", class: "digit", min: "-1", max: "64" },
    { type: "characterCount", class: "digit", min: "0", max: "65" },
    { type: "characterCount", class: "digit", min: "2", max: "1" },
  ] as const)("rejects invalid values even on disabled rows #%#", (constraint) => {
    const errors = validateDirectConstraints([{ ...row(constraint), enabled: false }]);
    expect(errors.length).toBeGreaterThan(0);
    expect(errors.join(" ")).not.toContain("private");
    expect(errors.join(" ")).not.toContain("payload");
  });

  it("preserves spaces, case, repeated allowed characters, duplicate values, and contradictory valid rows", () => {
    const rows = [row({ type: "position", index: "1", allowed: " AaAA " }), row({ type: "prefix", literal: " A " }), row({ type: "prefix", literal: " B " })]
      .map((draft, index) => ({ ...draft, editorId: index + 1 }));
    rows.push({ ...rows[1], editorId: 4 });
    expect(validateDirectConstraints(rows)).toEqual([]);
    expect(serializeDirectConstraints(rows).map((draft) => draft.constraint)).toEqual([
      { type: "position", index: 1, allowed: " AaAA " }, { type: "prefix", literal: " A " }, { type: "prefix", literal: " B " }, { type: "prefix", literal: " A " },
    ]);
    expect(serializeDirectConstraints(rows).map((draft) => draft.editorId)).toEqual([1, 2, 3, 4]);
  });

  it("accepts 16 rows and 1,520 string bytes, rejects 17 even when disabled", () => {
    const rows = Array.from({ length: 16 }, (_, index) => ({ ...row({ type: "position", index: "1", allowed: "a".repeat(95) }), editorId: index + 1, enabled: false }));
    expect(validateDirectConstraints(rows)).toEqual([]);
    expect(validateDirectConstraints([...rows, { ...rows[0], editorId: 17 }])).toContain("Use at most 16 direct conditions, including disabled rows.");
    expect(validateDirectConstraints([])).toEqual([]);
  });

  it.each([0, -1, 1.1, 0x1_0000_0000, NaN])("rejects editor ID outside nonzero u32 #%#", (editorId) => {
    expect(validateDirectConstraints([{ ...createDefaultDirectConstraint(editorId) }]).length).toBeGreaterThan(0);
  });
  it("accepts the maximum editor ID and rejects duplicate identities", () => {
    expect(validateDirectConstraints([createDefaultDirectConstraint(0xffff_ffff)])).toEqual([]);
    expect(validateDirectConstraints([createDefaultDirectConstraint(1), createDefaultDirectConstraint(1)]).length).toBeGreaterThan(0);
  });

  it("retains metadata while changing type and returns fresh incomplete fields", () => {
    const draft: DirectConstraintDraft = { ...row({ type: "prefix", literal: "retained" }), editorId: 27, enabled: false, applicability: "soft", confidence: "high" };
    const next = changeConstraintType(draft, "position");
    expect(next).toEqual({ editorId: 27, enabled: false, applicability: "soft", confidence: "high", constraint: { type: "position", index: "1", allowed: "" } });
    expect(draft.constraint).toEqual({ type: "prefix", literal: "retained" });
  });

  it("projects only explicit canonical fields and rejects string DTO numbers", () => {
    const dto = { ...serializeDirectConstraints([createDefaultDirectConstraint(1)])[0], unexpected: true, constraint: { type: "length", min: 1, max: 64, unexpected: true } } as DirectConstraint;
    expect(canonicalDirectConstraints([dto])).toEqual([{ editorId: 1, enabled: true, applicability: "hard", confidence: "medium", constraint: { type: "length", min: 1, max: 64 } }]);
    expect(() => canonicalDirectConstraints([{ ...dto, constraint: { type: "length", min: "1", max: 64 } } as unknown as DirectConstraint])).toThrow("Invalid direct conditions.");
    expect(() => canonicalDirectConstraints([{ ...dto, constraint: { type: "contains", literal: ["a"] } } as unknown as DirectConstraint])).toThrow("Invalid direct conditions.");
  });

  it("describes submitted data without asserting matches", () => {
    const rows = serializeDirectConstraints([row({ type: "prefix", literal: " MiXeD " })]);
    expect(describeDirectConstraint(rows[0])).toBe("Prefix: “ MiXeD ”");
  });
});
