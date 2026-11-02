// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { canonicalConstraint, createDefaultDirectConstraint, serializeConstraint, validateConstraint } from "./direct-constraints";
import type { DirectConstraint, DirectConstraintDraft } from "./direct-constraints";
import { unicodeEvidenceBytes } from "./unicode-text";

export const CLUE_LIMITS = {
  clues: 8, hypotheses: 8, groups: 8, memberships: 8, effectsPerHypothesis: 4, effects: 16, totalEffects: 32,
  textBytes: 512, sourceBytes: 256, interpretationBytes: 256,
  unicodeBytes: 2048, constraintStringBytes: 3040, maxEditorId: 0xffff_ffff,
} as const;
export interface ClueDraft { editorId: number; text: string; source: string }
export type Clue = ClueDraft;
export interface AlternativeGroupDraft { editorId: number; clueEditorId: number; selectedHypothesisEditorId: number | null }
export type AlternativeGroup = AlternativeGroupDraft;
export interface ScenarioDraft { hypotheses: HypothesisDraft[]; groups: AlternativeGroupDraft[] }
export interface EffectDraft { editorId: number; constraint: DirectConstraintDraft["constraint"] }
export interface ConstraintEffect { editorId: number; constraint: DirectConstraint["constraint"] }
interface HypothesisMetadata {
  editorId: number; clueEditorId: number; interpretation: string; enabled: boolean; groupEditorId?: number;
  applicability: "hard" | "soft"; confidence: "low" | "medium" | "high";
}
export interface HypothesisDraft extends HypothesisMetadata { effects: EffectDraft[] }
export interface Hypothesis extends HypothesisMetadata { effects: ConstraintEffect[] }

export function createDefaultHypothesis(editorId: number, clueEditorId: number): HypothesisDraft {
  return { editorId, clueEditorId, interpretation: "", enabled: true, applicability: "hard", confidence: "medium", effects: [] };
}
export function createDefaultGroup(editorId: number, clueEditorId: number): AlternativeGroupDraft {
  return { editorId, clueEditorId, selectedHypothesisEditorId: null };
}
export function createDefaultEffect(editorId: number): EffectDraft {
  return { editorId, constraint: createDefaultDirectConstraint(editorId).constraint };
}

const stringBytes = (value: DirectConstraintDraft["constraint"]): number =>
  "literal" in value ? value.literal.length : "allowed" in value ? value.allowed.length : 0;

/** Validate every draft, including disabled and effect-free interpretations. */
export function validateClueHypotheses(clues: readonly ClueDraft[], hypotheses: readonly HypothesisDraft[], direct: readonly DirectConstraintDraft[] = [], groups: readonly AlternativeGroupDraft[] = []): string[] {
  const errors: string[] = [];
  if (clues.length > CLUE_LIMITS.clues) errors.push("Use at most 8 clues.");
  if (hypotheses.length > CLUE_LIMITS.hypotheses) errors.push("Use at most 8 clue-backed hypotheses, including disabled drafts.");
  if (!Array.isArray(groups)) return ["Alternative groups are invalid."];
  if (groups.length > CLUE_LIMITS.groups) errors.push("Use at most 8 alternative groups.");
  const ids = new Set<number>();
  function identity(id: number) {
    if (!Number.isInteger(id) || id < 1 || id > CLUE_LIMITS.maxEditorId || ids.has(id)) errors.push("Every editing record must have a distinct nonzero editor ID.");
    ids.add(id);
  }
  direct.forEach((row) => identity(row.editorId));
  let unicodeBytes = 0;
  function evidence(value: string, max: number, name: string) {
    const bytes = typeof value === "string" ? unicodeEvidenceBytes(value) : null;
    if (bytes === null || bytes > max) errors.push(`${name} must be Unicode scalar text without CR, at most ${max} UTF-8 bytes.`);
    if (bytes !== null) unicodeBytes += bytes;
  }
  for (const clue of clues) {
    identity(clue.editorId);
    evidence(clue.text, CLUE_LIMITS.textBytes, "Clue text");
    evidence(clue.source, CLUE_LIMITS.sourceBytes, "Clue source");
  }
  const clueIds = new Set(clues.map((clue) => clue.editorId));
  for (const group of groups) {
    identity(group.editorId);
    if (!clueIds.has(group.clueEditorId)) errors.push("Every alternative group must reference an existing clue.");
    if (!Object.hasOwn(group, "selectedHypothesisEditorId") || (group.selectedHypothesisEditorId !== null &&
      (!Number.isInteger(group.selectedHypothesisEditorId) || group.selectedHypothesisEditorId < 1 || group.selectedHypothesisEditorId > CLUE_LIMITS.maxEditorId))) errors.push("Alternative group selections are invalid.");
  }
  errors.push(...relationshipErrors(hypotheses, groups));
  let effects = 0;
  let constraintBytes = direct.reduce((sum, row) => sum + stringBytes(row.constraint), 0);
  for (const hypothesis of hypotheses) {
    identity(hypothesis.editorId);
    if (!clueIds.has(hypothesis.clueEditorId)) errors.push("Every clue-backed hypothesis must reference an existing clue.");
    evidence(hypothesis.interpretation, CLUE_LIMITS.interpretationBytes, "Interpretation");
    if (typeof hypothesis.enabled !== "boolean" || !["hard", "soft"].includes(hypothesis.applicability) || !["low", "medium", "high"].includes(hypothesis.confidence)) errors.push("Hypothesis settings are invalid.");
    if (hypothesis.effects.length > CLUE_LIMITS.effectsPerHypothesis) errors.push("Use at most 4 effects per hypothesis, including disabled drafts.");
    effects += hypothesis.effects.length;
    for (const effect of hypothesis.effects) {
      identity(effect.editorId);
      errors.push(...validateConstraint(effect.constraint).map((message) => message.replace("Condition 1", "Hypothesis effect")));
      constraintBytes += stringBytes(effect.constraint);
    }
  }
  if (effects > CLUE_LIMITS.effects) errors.push("Use at most 16 clue-backed effects in total.");
  if (effects + direct.length > CLUE_LIMITS.totalEffects) errors.push("Use at most 32 effects including Direct Constraints.");
  if (unicodeBytes > CLUE_LIMITS.unicodeBytes) errors.push("Clue text, sources, and interpretations must total at most 2,048 UTF-8 bytes.");
  if (constraintBytes > CLUE_LIMITS.constraintStringBytes) errors.push("Constraint strings must total at most 3,040 bytes.");
  return [...new Set(errors)];
}

const clueDto = (clue: Clue): Clue => ({ editorId: clue.editorId, text: clue.text, source: clue.source });
const metadata = (hypothesis: HypothesisMetadata): HypothesisMetadata => ({
  editorId: hypothesis.editorId, clueEditorId: hypothesis.clueEditorId, interpretation: hypothesis.interpretation,
  enabled: hypothesis.enabled, ...(Object.hasOwn(hypothesis, "groupEditorId") ? { groupEditorId: hypothesis.groupEditorId } : {}), applicability: hypothesis.applicability, confidence: hypothesis.confidence,
});
export function serializeClueHypotheses(clues: readonly ClueDraft[], hypotheses: readonly HypothesisDraft[], groups: readonly AlternativeGroupDraft[] = []): { clues: Clue[]; hypotheses: Hypothesis[]; groups: AlternativeGroup[] } {
  if (validateClueHypotheses(clues, hypotheses, [], groups).length) throw new Error("Invalid clue-backed hypotheses.");
  return {
    clues: clues.map(clueDto), groups: groups.map(groupDto),
    hypotheses: hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => ({ editorId: effect.editorId, constraint: serializeConstraint(effect.constraint) })) })),
  };
}

/** Project explicit canonical fields, rejecting invalid runtime numbers and references. */
export function canonicalClueHypotheses(clues: readonly Clue[], hypotheses: readonly Hypothesis[], groups: readonly AlternativeGroup[] = []): { clues: Clue[]; hypotheses: Hypothesis[]; groups: AlternativeGroup[] } {
  const canonical = hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => ({ editorId: effect.editorId, constraint: canonicalConstraint(effect.constraint) })) }));
  const drafts = clueHypothesesToDrafts(clues, canonical, groups);
  if (validateClueHypotheses(drafts.clues, drafts.hypotheses, [], groups).length) throw new Error("Invalid clue-backed hypotheses.");
  return { clues: clues.map(clueDto), hypotheses: canonical, groups: groups.map(groupDto) };
}

export function clueHypothesesToDrafts(clues: readonly Clue[], hypotheses: readonly Hypothesis[], groups: readonly AlternativeGroup[] = []): { clues: ClueDraft[]; hypotheses: HypothesisDraft[]; groups: AlternativeGroupDraft[] } {
  return { clues: clues.map(clueDto), groups: groups.map(groupDto), hypotheses: hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => {
    const value = effect.constraint;
    return { editorId: effect.editorId, constraint: "min" in value ? { ...value, min: String(value.min), max: String(value.max) } : "index" in value ? { ...value, index: String(value.index) } : { ...value } };
  }) })) };
}

const groupDto = (group: AlternativeGroup): AlternativeGroup => ({
  editorId: group.editorId, clueEditorId: group.clueEditorId, selectedHypothesisEditorId: group.selectedHypothesisEditorId,
});

/** Membership lives only on hypotheses; the group stores just its anchor and selection. */
function relationshipErrors(hypotheses: readonly HypothesisMetadata[], groups: readonly AlternativeGroup[]): string[] {
  const errors: string[] = [];
  let memberships = 0;
  for (const hypothesis of hypotheses) {
    if (!Object.hasOwn(hypothesis, "groupEditorId")) continue;
    memberships += 1;
    const group = groups.find((row) => row.editorId === hypothesis.groupEditorId);
    if (!Number.isInteger(hypothesis.groupEditorId) || (hypothesis.groupEditorId ?? 0) < 1 ||
      (hypothesis.groupEditorId ?? 0) > CLUE_LIMITS.maxEditorId || !group) errors.push("Every grouped hypothesis must reference an existing alternative group.");
    if (hypothesis.enabled !== false) errors.push("Grouped hypotheses must use disabled independent activation.");
    if (group && group.clueEditorId !== hypothesis.clueEditorId) errors.push("Group members must originate from the group's clue.");
  }
  if (memberships > CLUE_LIMITS.memberships) errors.push("Use at most 8 group memberships.");
  for (const group of groups) {
    if (group.selectedHypothesisEditorId === null) continue;
    const selected = hypotheses.find((row) => row.editorId === group.selectedHypothesisEditorId);
    if (!selected || selected.groupEditorId !== group.editorId) errors.push("Every group selection must identify a member of that group or None.");
  }
  return errors;
}

/** Used only for submitted-scenario consistency, never for constraint matching. */
export function isHypothesisActive(hypothesis: Pick<HypothesisMetadata, "editorId" | "enabled" | "groupEditorId">, groups: readonly AlternativeGroup[]): boolean {
  return hypothesis.groupEditorId === undefined ? hypothesis.enabled === true :
    groups.some((group) => group.editorId === hypothesis.groupEditorId && group.selectedHypothesisEditorId === hypothesis.editorId);
}

function assertRelationships(state: ScenarioDraft): void {
  const ids = [...state.hypotheses.map((row) => row.editorId), ...state.groups.map((row) => row.editorId)];
  const validId = (id: number) => Number.isInteger(id) && id >= 1 && id <= CLUE_LIMITS.maxEditorId;
  if (ids.some((id) => !validId(id)) || new Set(ids).size !== ids.length ||
    state.groups.some((group) => !validId(group.clueEditorId) || !Object.hasOwn(group, "selectedHypothesisEditorId")) ||
    relationshipErrors(state.hypotheses, state.groups).length) throw new Error("Invalid alternative group edit.");
}
function clearSelection(groups: AlternativeGroupDraft[], hypothesisEditorId: number): AlternativeGroupDraft[] {
  return groups.map((group) => group.selectedHypothesisEditorId === hypothesisEditorId ? { ...group, selectedHypothesisEditorId: null } : group);
}

/** Apply membership and selection changes together; entering or leaving a group never enables a hypothesis. */
export function setHypothesisGroup(state: ScenarioDraft, hypothesisEditorId: number, groupEditorId: number | undefined): ScenarioDraft {
  assertRelationships(state);
  const hypothesis = state.hypotheses.find((row) => row.editorId === hypothesisEditorId);
  const group = groupEditorId === undefined ? undefined : state.groups.find((row) => row.editorId === groupEditorId);
  if (!hypothesis || (groupEditorId !== undefined && (!group || group.clueEditorId !== hypothesis.clueEditorId))) throw new Error("Invalid alternative group edit.");
  if (hypothesis.groupEditorId === groupEditorId) return state;
  return {
    groups: clearSelection(state.groups, hypothesisEditorId),
    hypotheses: state.hypotheses.map((row) => {
      if (row.editorId !== hypothesisEditorId) return row;
      const { groupEditorId: _previous, ...independent } = row;
      return { ...independent, enabled: false, ...(groupEditorId === undefined ? {} : { groupEditorId }) };
    }),
  };
}
export function removeHypothesis(state: ScenarioDraft, hypothesisEditorId: number): ScenarioDraft {
  assertRelationships(state);
  if (!state.hypotheses.some((row) => row.editorId === hypothesisEditorId)) throw new Error("Invalid alternative group edit.");
  return { hypotheses: state.hypotheses.filter((row) => row.editorId !== hypothesisEditorId), groups: clearSelection(state.groups, hypothesisEditorId) };
}
export function removeAlternativeGroup(state: ScenarioDraft, groupEditorId: number): ScenarioDraft {
  assertRelationships(state);
  if (!state.groups.some((group) => group.editorId === groupEditorId)) throw new Error("Invalid alternative group edit.");
  return {
    groups: state.groups.filter((group) => group.editorId !== groupEditorId),
    hypotheses: state.hypotheses.map((row) => {
      if (row.groupEditorId !== groupEditorId) return row;
      const { groupEditorId: _previous, ...independent } = row;
      return { ...independent, enabled: false };
    }),
  };
}
export function selectAlternative(state: ScenarioDraft, groupEditorId: number, selectedHypothesisEditorId: number | null): ScenarioDraft {
  assertRelationships(state);
  const group = state.groups.find((row) => row.editorId === groupEditorId);
  if (!group || (selectedHypothesisEditorId !== null && !state.hypotheses.some((row) => row.editorId === selectedHypothesisEditorId && row.groupEditorId === groupEditorId))) throw new Error("Invalid alternative group edit.");
  if (group.selectedHypothesisEditorId === selectedHypothesisEditorId) return state;
  return { ...state, groups: state.groups.map((row) => row.editorId === groupEditorId ? { ...row, selectedHypothesisEditorId } : row) };
}
export function setHypothesisOrigin(state: ScenarioDraft, hypothesisEditorId: number, clueEditorId: number): ScenarioDraft {
  assertRelationships(state);
  const hypothesis = state.hypotheses.find((row) => row.editorId === hypothesisEditorId);
  if (!hypothesis || hypothesis.groupEditorId !== undefined) throw new Error("Ungroup the hypothesis before changing its clue.");
  return { ...state, hypotheses: state.hypotheses.map((row) => row.editorId === hypothesisEditorId ? { ...row, clueEditorId } : row) };
}
