// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { canonicalConstraint, createDefaultDirectConstraint, serializeConstraint, validateConstraint } from "./direct-constraints";
import type { DirectConstraint, DirectConstraintDraft } from "./direct-constraints";
import { unicodeEvidenceBytes } from "./unicode-text";

export const CLUE_LIMITS = {
  clues: 8, hypotheses: 8, effectsPerHypothesis: 4, effects: 16, totalEffects: 32,
  textBytes: 512, sourceBytes: 256, interpretationBytes: 256,
  unicodeBytes: 2048, constraintStringBytes: 3040, maxEditorId: 0xffff_ffff,
} as const;
export interface ClueDraft { editorId: number; text: string; source: string }
export type Clue = ClueDraft;
export interface EffectDraft { editorId: number; constraint: DirectConstraintDraft["constraint"] }
export interface ConstraintEffect { editorId: number; constraint: DirectConstraint["constraint"] }
interface HypothesisMetadata {
  editorId: number; clueEditorId: number; interpretation: string; enabled: boolean;
  applicability: "hard" | "soft"; confidence: "low" | "medium" | "high";
}
export interface HypothesisDraft extends HypothesisMetadata { effects: EffectDraft[] }
export interface Hypothesis extends HypothesisMetadata { effects: ConstraintEffect[] }

export function createDefaultHypothesis(editorId: number, clueEditorId: number): HypothesisDraft {
  return { editorId, clueEditorId, interpretation: "", enabled: true, applicability: "hard", confidence: "medium", effects: [] };
}
export function createDefaultEffect(editorId: number): EffectDraft {
  return { editorId, constraint: createDefaultDirectConstraint(editorId).constraint };
}

const stringBytes = (value: DirectConstraintDraft["constraint"]): number =>
  "literal" in value ? value.literal.length : "allowed" in value ? value.allowed.length : 0;

/** Validate every draft, including disabled and effect-free interpretations. */
export function validateClueHypotheses(clues: readonly ClueDraft[], hypotheses: readonly HypothesisDraft[], direct: readonly DirectConstraintDraft[] = []): string[] {
  const errors: string[] = [];
  if (clues.length > CLUE_LIMITS.clues) errors.push("Use at most 8 clues.");
  if (hypotheses.length > CLUE_LIMITS.hypotheses) errors.push("Use at most 8 clue-backed hypotheses, including disabled drafts.");
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
  enabled: hypothesis.enabled, applicability: hypothesis.applicability, confidence: hypothesis.confidence,
});
export function serializeClueHypotheses(clues: readonly ClueDraft[], hypotheses: readonly HypothesisDraft[]): { clues: Clue[]; hypotheses: Hypothesis[] } {
  if (validateClueHypotheses(clues, hypotheses).length) throw new Error("Invalid clue-backed hypotheses.");
  return {
    clues: clues.map(clueDto),
    hypotheses: hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => ({ editorId: effect.editorId, constraint: serializeConstraint(effect.constraint) })) })),
  };
}

/** Project explicit canonical fields, rejecting invalid runtime numbers and references. */
export function canonicalClueHypotheses(clues: readonly Clue[], hypotheses: readonly Hypothesis[]): { clues: Clue[]; hypotheses: Hypothesis[] } {
  const canonical = hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => ({ editorId: effect.editorId, constraint: canonicalConstraint(effect.constraint) })) }));
  const drafts = clueHypothesesToDrafts(clues, canonical);
  if (validateClueHypotheses(drafts.clues, drafts.hypotheses).length) throw new Error("Invalid clue-backed hypotheses.");
  return { clues: clues.map(clueDto), hypotheses: canonical };
}

export function clueHypothesesToDrafts(clues: readonly Clue[], hypotheses: readonly Hypothesis[]): { clues: ClueDraft[]; hypotheses: HypothesisDraft[] } {
  return { clues: clues.map(clueDto), hypotheses: hypotheses.map((hypothesis) => ({ ...metadata(hypothesis), effects: hypothesis.effects.map((effect) => {
    const value = effect.constraint;
    return { editorId: effect.editorId, constraint: "min" in value ? { ...value, min: String(value.min), max: String(value.max) } : "index" in value ? { ...value, index: String(value.index) } : { ...value } };
  }) })) };
}
