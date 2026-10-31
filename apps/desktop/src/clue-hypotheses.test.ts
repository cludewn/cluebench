// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { canonicalClueHypotheses, createDefaultEffect, createDefaultHypothesis, serializeClueHypotheses, validateClueHypotheses } from "./clue-hypotheses";
import { createDefaultDirectConstraint } from "./direct-constraints";
import { hasLoneSurrogate, unicodeEvidenceBytes } from "./unicode-text";

const clue = { editorId: 1, text: "  e\u0301😀\n\t\u624b\u639b\u304b\u308a  ", source: "\t\u5408\u6210\nsource  " };
const hypothesis = () => ({ ...createDefaultHypothesis(2, 1), effects: [createDefaultEffect(3), createDefaultEffect(4)] });

describe("Clue and hypothesis authoring contracts", () => {
  it("preserves scalar Unicode, whitespace, distinct equal interpretations and multiple effects without mutation", () => {
    const first = { ...hypothesis(), interpretation: " e\u0301😀\n\t " };
    const second = { ...first, editorId: 5, effects: [{ ...first.effects[0], editorId: 6 }] };
    const dto = serializeClueHypotheses([clue], [first, second]);
    expect(dto.clues).toEqual([clue]);
    expect(dto.hypotheses[0].interpretation).toBe(first.interpretation);
    expect(dto.hypotheses.map((row) => row.editorId)).toEqual([2, 5]);
    expect(canonicalClueHypotheses(dto.clues, dto.hypotheses)).toEqual(dto);
    first.interpretation = "changed"; first.effects[0].constraint = { type: "suffix", literal: "changed" };
    expect(dto.hypotheses[0].interpretation).toBe(" e\u0301😀\n\t ");
    expect(dto.hypotheses[0].effects[0].constraint.type).toBe("length");
  });
  it("accepts empty text, source, interpretation and zero effects including disabled drafts", () => {
    expect(validateClueHypotheses([{ editorId: 1, text: "", source: "" }], [{ ...createDefaultHypothesis(2, 1), enabled: false }])).toEqual([]);
  });
  it.each(["\r", "\r\n", "\ud800", "\udc00", "x\ud800y"])("rejects invalid scalar/line-ending input before UTF-8 encoding #%#", (text) => {
    expect(unicodeEvidenceBytes(text)).toBeNull();
    expect(validateClueHypotheses([{ ...clue, text }], []).length).toBeGreaterThan(0);
    expect(() => serializeClueHypotheses([clue], [{ ...hypothesis(), interpretation: text }])).toThrow("Invalid clue-backed hypotheses.");
  });
  it("counts UTF-8 bytes and rejects individual or combined excess without truncation", () => {
    expect(hasLoneSurrogate("😀\uFFFD")).toBe(false);
    expect(unicodeEvidenceBytes("😀e\u0301")).toBe(7);
    expect(validateClueHypotheses([{ ...clue, text: "😀".repeat(128), source: "x".repeat(256) }], [{ ...hypothesis(), interpretation: "y".repeat(256) }])).toEqual([]);
    for (const value of [{ ...clue, text: "😀".repeat(128) + "A" }, { ...clue, source: "é".repeat(128) + "A" }]) {
      expect(validateClueHypotheses([value], []).length).toBeGreaterThan(0);
    }
    const clues = Array.from({ length: 4 }, (_, i) => ({ editorId: i + 1, text: "x".repeat(512), source: "" }));
    expect(validateClueHypotheses(clues, [])).toEqual([]);
    clues[0].source = "A";
    expect(validateClueHypotheses(clues, [])).toContain("Clue text, sources, and interpretations must total at most 2,048 UTF-8 bytes.");
    expect(clues[0].source).toBe("A");
  });
  it("rejects references to missing or wrong-kind editor records and every shared ID collision", () => {
    for (const clueEditorId of [0, 2, 3, 999]) expect(validateClueHypotheses([clue], [{ ...hypothesis(), clueEditorId }]).length).toBeGreaterThan(0);
    for (const editorId of [0, -1, 1.5, 0x1_0000_0000, NaN, 1, 2]) {
      const row = hypothesis(); row.effects[0].editorId = editorId;
      expect(validateClueHypotheses([clue], [row]).length).toBeGreaterThan(0);
    }
    expect(validateClueHypotheses([clue], [hypothesis()], [createDefaultDirectConstraint(4)]).length).toBeGreaterThan(0);
    expect(validateClueHypotheses([{ ...clue, editorId: 0xffff_ffff }], [])).toEqual([]);
  });
  it("validates disabled effects without repairing numeric drafts or silently dropping them", () => {
    const row = { ...hypothesis(), enabled: false };
    row.effects[0].constraint = { type: "position", index: " 2", allowed: "A" };
    expect(validateClueHypotheses([clue], [row]).length).toBeGreaterThan(0);
    expect(() => serializeClueHypotheses([clue], [row])).toThrow();
    expect(row.effects[0].constraint.index).toBe(" 2");
  });
  it("enforces clue, hypothesis, per-hypothesis and combined effect limits", () => {
    expect(validateClueHypotheses(Array.from({ length: 9 }, (_, i) => ({ ...clue, editorId: i + 1 })), []).length).toBeGreaterThan(0);
    expect(validateClueHypotheses([clue], Array.from({ length: 9 }, (_, i) => createDefaultHypothesis(i + 2, 1))).length).toBeGreaterThan(0);
    const tooMany = { ...hypothesis(), effects: Array.from({ length: 5 }, (_, i) => createDefaultEffect(i + 20)) };
    expect(validateClueHypotheses([clue], [tooMany])).toContain("Use at most 4 effects per hypothesis, including disabled drafts.");
    const rows = Array.from({ length: 5 }, (_, i) => ({ ...createDefaultHypothesis(i + 2, 1), effects: Array.from({ length: 4 }, (_, j) => createDefaultEffect(20 + i * 4 + j)) }));
    expect(validateClueHypotheses([clue], rows)).toContain("Use at most 16 clue-backed effects in total.");
  });
  it("canonical projection strips extras but rejects invalid typed numbers", () => {
    const dto = serializeClueHypotheses([clue], [hypothesis()]);
    const extra = { ...dto.hypotheses[0], extra: "synthetic", effects: [{ ...dto.hypotheses[0].effects[0], extra: "synthetic" }] };
    const extraClue = { ...clue, extra: "synthetic" };
    expect(JSON.stringify(canonicalClueHypotheses([extraClue], [extra]))).not.toContain("synthetic");
    extra.effects[0].constraint = { type: "length", min: "1", max: 64 } as never;
    expect(() => canonicalClueHypotheses([clue], [extra])).toThrow();
  });
});
