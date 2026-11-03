// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { authoringDocument, defaultAuthoringInput, isProjectDocument, stageProject } from "./project-document";
import type { ProjectDocumentV1 } from "./project-document";
import { validatePreviewInput } from "./preview-input";

const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
export function syntheticDocument(): ProjectDocumentV1 {
  return {
    ...authoringDocument(defaultAuthoringInput(), new Map()),
    clues: [{ id: id(1), text: "", source: " Synthetic\t資料\n" }],
    hypotheses: [
      { id: id(2), origin: { type: "clue", clueId: id(1) }, interpretation: "", applicability: "soft", confidence: "high", activation: { type: "independent", enabled: false }, effects: [] },
      { id: id(3), origin: { type: "clue", clueId: id(1) }, interpretation: " 仮説\n", applicability: "hard", confidence: "low", activation: { type: "grouped", groupId: id(4) }, effects: [
        { id: id(5), constraint: { type: "length", min: "0002", max: "-" } },
        { id: id(6), constraint: { type: "position", index: "", allowed: "é" } },
        { id: id(7), constraint: { type: "contains", literal: "" } },
      ] },
    ],
    groups: [{ id: id(4), anchorClueId: id(1), selectedHypothesisId: id(3) }],
    directConstraints: [{ id: id(8), enabled: false, applicability: "soft", confidence: "medium", constraint: { type: "characterCount", class: "symbol", min: "9", max: "2" } }],
    manualSeeds: { text: "\n\nseed\t試験\n" },
    options: { caseVariants: [], prefixes: { includeEmpty: true, rows: [{ id: id(9), value: "" }, { id: id(10), value: "é" }, { id: id(11), value: "é" }] }, suffixes: { includeEmpty: false, rows: [] } },
    limits: { attemptLimit: "invalid", emissionLimit: "0002" },
  };
}

describe("Version 1 authoring documents", () => {
  it("round trips incomplete numeric fields, all raw strings, empty variants and affix rows independently of Preview", () => {
    const document = syntheticDocument();
    expect(isProjectDocument(document)).toBe(true);
    const staged = stageProject(document, 100);
    expect(authoringDocument(staged.input, staged.identities)).toEqual(document);
    expect(validatePreviewInput(staged.input).length).toBeGreaterThan(0);
    expect(staged.input.seedText).toBe("\n\nseed\t試験\n");
    expect(staged.input.prefixDraft.rows.map((row) => row.value)).toEqual(["", "é", "é"]);
    expect(staged.input.hypotheses[1]).toMatchObject({ enabled: false, groupEditorId: staged.input.groups![0].editorId });
  });
  it("rebuilds editor IDs without changing UUIDs or group selections on repeated loads", () => {
    const document = syntheticDocument();
    const first = stageProject(document, 0), second = stageProject(document, first.lastEditorId);
    expect([...second.identities.keys()].every((key) => key > first.lastEditorId)).toBe(true);
    expect(authoringDocument(second.input, second.identities)).toEqual(document);
    expect(second.input.groups![0].selectedHypothesisEditorId).toBe(second.input.hypotheses[1].editorId);
  });
  it("reserves IDs for all entities including affixes and rejects exhaustion before any mapping is applied", () => {
    const document = syntheticDocument();
    expect(stageProject(document, 0xffff_ffff - 11).lastEditorId).toBe(0xffff_ffff);
    expect(() => stageProject(document, 0xffff_ffff - 10)).toThrow("capacity");
    expect(document).toEqual(syntheticDocument());
  });
  it("preserves eight rows plus includeEmpty although that state exceeds the Preview option allowance", () => {
    const document = syntheticDocument();
    document.options.prefixes = { includeEmpty: true, rows: Array.from({ length: 8 }, (_, n) => ({ id: id(20 + n), value: String(n) })) };
    expect(isProjectDocument(document)).toBe(true);
    const staged = stageProject(document, 0);
    expect(staged.input.prefixes).toHaveLength(9);
    expect(authoringDocument(staged.input, staged.identities)).toEqual(document);
    expect(validatePreviewInput(staged.input)).toContain("Prefix options must contain 1–8 entries.");
  });
  it("allows evidence beyond Preview's shared budget and preserves authored case order", () => {
    const document = syntheticDocument();
    document.clues = Array.from({ length: 8 }, (_, n) => ({ id: id(n === 0 ? 1 : 30 + n), text: "a".repeat(512), source: "" }));
    document.options.caseVariants = ["asciiUppercase", "original", "asciiLowercase"];
    const staged = stageProject(document, 0);
    expect(authoringDocument(staged.input, staged.identities)).toEqual(document);
    expect(validatePreviewInput(staged.input)).toContain("Clue text, sources, and interpretations must total at most 2,048 UTF-8 bytes.");
  });
  it.each([
    (doc: ProjectDocumentV1) => { doc.schemaVersion = 2 as 1; },
    (doc: ProjectDocumentV1) => { Object.assign(doc, { extra: false }); },
    (doc: ProjectDocumentV1) => { doc.clues[0].id = doc.hypotheses[0].id; },
    (doc: ProjectDocumentV1) => { doc.clues[0].id = "00000000-0000-4000-A000-000000000001"; },
    (doc: ProjectDocumentV1) => { doc.hypotheses[0].origin.clueId = doc.directConstraints[0].id; },
    (doc: ProjectDocumentV1) => { doc.hypotheses[0].origin.clueId = id(999); },
    (doc: ProjectDocumentV1) => { doc.groups[0].selectedHypothesisId = doc.hypotheses[0].id; },
    (doc: ProjectDocumentV1) => { doc.clues.push({ id: id(90), text: "", source: "" }); doc.groups[0].anchorClueId = id(90); },
    (doc: ProjectDocumentV1) => { doc.clues[0].text = "\r\n"; },
    (doc: ProjectDocumentV1) => { doc.manualSeeds.text = "a\r\nb"; },
    (doc: ProjectDocumentV1) => { doc.clues[0].text = "\ud800"; },
    (doc: ProjectDocumentV1) => { doc.manualSeeds.text = "a".repeat(16385); },
    (doc: ProjectDocumentV1) => { doc.options.prefixes.rows[0].value = "\n"; },
    (doc: ProjectDocumentV1) => { doc.options.prefixes.rows[0].value = "é".repeat(129); },
    (doc: ProjectDocumentV1) => { doc.limits.attemptLimit = "a".repeat(257); },
    (doc: ProjectDocumentV1) => { Object.assign(doc.groups[0], { selectedHypothesisId: undefined }); },
  ])("rejects invalid structure, identity, references or nonreproducible strings (%#)", (mutate) => {
    const document = syntheticDocument(); mutate(document);
    expect(isProjectDocument(document)).toBe(false);
    expect(() => stageProject(document, 0)).toThrow();
  });
  it("accepts exact byte boundaries and zero-effect independent enabled drafts", () => {
    const document = syntheticDocument();
    document.manualSeeds.text = "\t".repeat(16384);
    document.limits.attemptLimit = "é".repeat(128);
    document.hypotheses[0].activation = { type: "independent", enabled: true };
    document.options.prefixes.rows[0].value = "é".repeat(128);
    expect(isProjectDocument(document)).toBe(true);
    expect(authoringDocument(stageProject(document, 0).input, stageProject(document, 0).identities)).toEqual(document);
  });
});
