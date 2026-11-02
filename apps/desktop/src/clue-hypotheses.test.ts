// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { canonicalClueHypotheses, createDefaultEffect, createDefaultGroup, createDefaultHypothesis, isHypothesisActive, removeAlternativeGroup, removeHypothesis, selectAlternative, serializeClueHypotheses, setHypothesisGroup, setHypothesisOrigin, validateClueHypotheses } from "./clue-hypotheses";
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

// Group operations update membership and selection in one returned scenario.
describe("Exclusive alternative group editing", () => {
  const scenario = () => ({
    groups: [createDefaultGroup(20, 1), createDefaultGroup(21, 1)],
    hypotheses: [hypothesis(), { ...createDefaultHypothesis(5, 1), interpretation: "second", applicability: "soft" as const, confidence: "high" as const, effects: [createDefaultEffect(6)] }],
  });
  it("creates empty None groups and serializes only hypothesis-owned membership", () => {
    const state = scenario();
    expect(state.groups[0]).toEqual({ editorId: 20, clueEditorId: 1, selectedHypothesisEditorId: null });
    expect(validateClueHypotheses([clue], state.hypotheses, [], state.groups)).toEqual([]);
    const grouped = setHypothesisGroup(state, 2, 20);
    expect(grouped.groups).toEqual(state.groups);
    expect(grouped.hypotheses[0].enabled).toBe(false);
    expect(grouped.hypotheses[0].effects).toBe(state.hypotheses[0].effects);
    const dto = serializeClueHypotheses([clue], grouped.hypotheses, grouped.groups);
    expect(dto.hypotheses[0].groupEditorId).toBe(20);
    expect(Object.keys(dto.groups[0])).toEqual(["editorId", "clueEditorId", "selectedHypothesisEditorId"]);
    expect(canonicalClueHypotheses(dto.clues, dto.hypotheses, dto.groups)).toEqual(dto);
    expect(state.hypotheses[0].enabled).toBe(true);
  });
  it("selects A/B/None exclusively, keeps same-group assignment a no-op and preserves identities", () => {
    let state = scenario();
    state = setHypothesisGroup(setHypothesisGroup(state, 2, 20), 5, 20);
    state = selectAlternative(state, 20, 2);
    expect(state.hypotheses.map((row) => isHypothesisActive(row, state.groups))).toEqual([true, false]);
    expect(setHypothesisGroup(state, 2, 20)).toBe(state);
    state = selectAlternative(state, 20, 5);
    expect(state.hypotheses.map((row) => isHypothesisActive(row, state.groups))).toEqual([false, true]);
    state = selectAlternative(state, 20, null);
    expect(state.hypotheses.map((row) => isHypothesisActive(row, state.groups))).toEqual([false, false]);
    expect(state.hypotheses.map((row) => row.editorId)).toEqual([2, 5]);
  });
  it("moves selected members without changing the destination selection and ungroups disabled", () => {
    const original = scenario();
    let state = setHypothesisGroup(setHypothesisGroup(original, 2, 20), 5, 21);
    state = selectAlternative(selectAlternative(state, 20, 2), 21, 5);
    state = setHypothesisGroup(state, 2, 21);
    expect(state.groups.map((group) => group.selectedHypothesisEditorId)).toEqual([null, 5]);
    expect(state.hypotheses[0]).toEqual({ ...original.hypotheses[0], enabled: false, groupEditorId: 21 });
    state = setHypothesisGroup(state, 5, undefined);
    expect(state.groups[1].selectedHypothesisEditorId).toBeNull();
    expect(state.hypotheses[1]).toEqual({ ...original.hypotheses[1], enabled: false });
    expect(Object.hasOwn(state.hypotheses[1], "groupEditorId")).toBe(false);
    expect(setHypothesisOrigin(state, 5, 100).hypotheses[1].clueEditorId).toBe(100);
  });
  it("removes a selected final member while retaining the empty group, and removes groups without losing members", () => {
    let state = scenario();
    state = selectAlternative(setHypothesisGroup(state, 2, 20), 20, 2);
    const removed = removeHypothesis(state, 2);
    expect(removed.groups).toEqual(scenario().groups);
    expect(removed.hypotheses.map((row) => row.editorId)).toEqual([5]);
    state = setHypothesisGroup(state, 5, 20);
    const result = removeAlternativeGroup(state, 20);
    expect(result.groups.map((row) => row.editorId)).toEqual([21]);
    expect(result.hypotheses).toEqual(scenario().hypotheses.map((row) => ({ ...row, enabled: false })));
    expect(result.hypotheses[0].effects).toBe(state.hypotheses[0].effects);
    expect(result.hypotheses[1].interpretation).toBe("second");
  });
  it("rejects cross-clue membership, missing edits and origin changes while grouped", () => {
    const state = scenario(); state.groups[1].clueEditorId = 99;
    expect(() => setHypothesisGroup(state, 2, 21)).toThrow("Invalid alternative group edit.");
    expect(() => setHypothesisGroup(state, 2, 999)).toThrow();
    expect(() => selectAlternative(state, 20, 2)).toThrow();
    expect(() => removeAlternativeGroup(state, 999)).toThrow();
    expect(() => removeHypothesis(state, 999)).toThrow();
    const grouped = setHypothesisGroup(state, 2, 20);
    expect(() => setHypothesisOrigin(grouped, 2, 99)).toThrow("Ungroup the hypothesis before changing its clue.");
    grouped.groups[0].selectedHypothesisEditorId = 5;
    expect(() => setHypothesisGroup(grouped, 2, undefined)).toThrow();
  });
  it("validates every group reference, selection, global ID and unselected effect", () => {
    for (const groupEditorId of [null, 0, 1, 3, 999, "20", 0x1_0000_0000, undefined]) {
      const state = scenario();
      state.hypotheses[0] = { ...state.hypotheses[0], enabled: false, groupEditorId } as never;
      expect(validateClueHypotheses([clue], state.hypotheses, [], state.groups).length).toBeGreaterThan(0);
    }
    const state = setHypothesisGroup(scenario(), 2, 20);
    for (const patch of [{ editorId: 1 }, { editorId: 2 }, { editorId: 3 }, { editorId: 0 }, { clueEditorId: 2 }, { clueEditorId: 99 }, { selectedHypothesisEditorId: 5 }, { selectedHypothesisEditorId: 999 }, { selectedHypothesisEditorId: undefined }]) {
      expect(validateClueHypotheses([clue], state.hypotheses, [], [{ ...state.groups[0], ...patch }] as never).length).toBeGreaterThan(0);
    }
    const { selectedHypothesisEditorId: _selected, ...missing } = state.groups[0];
    expect(validateClueHypotheses([clue], state.hypotheses, [], [missing] as never).length).toBeGreaterThan(0);
    expect(validateClueHypotheses([clue], state.hypotheses, [], state.groups.map((group) => ({ ...group, clueEditorId: 99 }))).length).toBeGreaterThan(0);
    expect(validateClueHypotheses([clue], [{ ...state.hypotheses[0], enabled: true }], [], state.groups).length).toBeGreaterThan(0);
    state.hypotheses[0].effects[0].constraint = { type: "contains", literal: "" };
    expect(validateClueHypotheses([clue], state.hypotheses, [], state.groups).length).toBeGreaterThan(0);
  });
  it("allows eight empty groups but rejects nine and counts group IDs across Direct Constraints", () => {
    const groups = Array.from({ length: 8 }, (_, i) => createDefaultGroup(i + 20, 1));
    expect(validateClueHypotheses([clue], [], [], groups)).toEqual([]);
    expect(validateClueHypotheses([clue], [], [], [...groups, createDefaultGroup(28, 1)])).toContain("Use at most 8 alternative groups.");
    expect(validateClueHypotheses([clue], [], [createDefaultDirectConstraint(20)], groups).length).toBeGreaterThan(0);
    expect(validateClueHypotheses([clue], [], [], null as never)).toEqual(["Alternative groups are invalid."]);
  });
});
