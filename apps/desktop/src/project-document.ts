// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { PreviewInput, CaseVariant } from "./preview-input";
import type { ConstraintValue } from "./direct-constraints";
import { hasLoneSurrogate } from "./unicode-text";

export type ProjectId = string;
export type ConstraintDraftV1 = ConstraintValue<string>;
export interface AffixDraftV1 { includeEmpty: boolean; rows: { id: ProjectId; value: string }[] }
export interface ProjectDocumentV1 {
  format: "cluebench-project";
  schemaVersion: 1;
  clues: { id: ProjectId; text: string; source: string }[];
  hypotheses: {
    id: ProjectId; origin: { type: "clue"; clueId: ProjectId }; interpretation: string;
    applicability: "hard" | "soft"; confidence: "low" | "medium" | "high";
    activation: { type: "independent"; enabled: boolean } | { type: "grouped"; groupId: ProjectId };
    effects: { id: ProjectId; constraint: ConstraintDraftV1 }[];
  }[];
  groups: { id: ProjectId; anchorClueId: ProjectId; selectedHypothesisId: ProjectId | null }[];
  directConstraints: { id: ProjectId; enabled: boolean; applicability: "hard" | "soft"; confidence: "low" | "medium" | "high"; constraint: ConstraintDraftV1 }[];
  manualSeeds: { text: string };
  options: { caseVariants: CaseVariant[]; prefixes: AffixDraftV1; suffixes: AffixDraftV1 };
  limits: { attemptLimit: string; emissionLimit: string };
}
export interface AffixEditorDraft { includeEmpty: boolean; rows: { editorId: number; value: string }[] }
export interface AuthoringInput extends PreviewInput { prefixDraft: AffixEditorDraft; suffixDraft: AffixEditorDraft }
export const defaultAuthoringInput = (): AuthoringInput => ({
  seedText: "", caseVariants: ["original"], prefixes: [""], suffixes: [""], attemptLimit: "2048", emissionLimit: "50",
  directConstraints: [], clues: [], hypotheses: [], groups: [], prefixDraft: { includeEmpty: true, rows: [] }, suffixDraft: { includeEmpty: true, rows: [] },
});
export function previewAffixes(draft: AffixEditorDraft): string[] {
  return [...(draft.includeEmpty ? [""] : []), ...draft.rows.map((row) => row.value).filter((value) => value !== "")];
}
export function authoringDocument(input: AuthoringInput, identities: ReadonlyMap<number, ProjectId>): ProjectDocumentV1 {
  const id = (editorId: number): ProjectId => {
    const value = identities.get(editorId);
    if (!value) throw new Error("Invalid project identity.");
    return value;
  };
  const affix = (draft: AffixEditorDraft): AffixDraftV1 => ({ includeEmpty: draft.includeEmpty, rows: draft.rows.map((row) => ({ id: id(row.editorId), value: row.value })) });
  return {
    format: "cluebench-project", schemaVersion: 1,
    clues: input.clues.map((row) => ({ id: id(row.editorId), text: row.text, source: row.source })),
    hypotheses: input.hypotheses.map((row) => ({
      id: id(row.editorId), origin: { type: "clue", clueId: id(row.clueEditorId) }, interpretation: row.interpretation,
      applicability: row.applicability, confidence: row.confidence,
      activation: row.groupEditorId === undefined ? { type: "independent", enabled: row.enabled } : { type: "grouped", groupId: id(row.groupEditorId) },
      effects: row.effects.map((effect) => ({ id: id(effect.editorId), constraint: { ...effect.constraint } })),
    })),
    groups: (input.groups ?? []).map((row) => ({ id: id(row.editorId), anchorClueId: id(row.clueEditorId), selectedHypothesisId: row.selectedHypothesisEditorId === null ? null : id(row.selectedHypothesisEditorId) })),
    directConstraints: input.directConstraints.map((row) => ({ id: id(row.editorId), enabled: row.enabled, applicability: row.applicability, confidence: row.confidence, constraint: { ...row.constraint } })),
    manualSeeds: { text: input.seedText }, options: { caseVariants: [...input.caseVariants], prefixes: affix(input.prefixDraft), suffixes: affix(input.suffixDraft) },
    limits: { attemptLimit: input.attemptLimit, emissionLimit: input.emissionLimit },
  };
}

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const bytes = (value: string) => new TextEncoder().encode(value).byteLength;
const record = (value: unknown, keys: string[]): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
const boundedArray = (value: unknown, max: number): value is unknown[] => Array.isArray(value) && value.length <= max;
const string = (value: unknown, max: number, domain: "textarea" | "affix" | "constraint" = "textarea"): value is string => typeof value === "string" && !hasLoneSurrogate(value) && bytes(value) <= max &&
  !(domain === "constraint" ? /[\x00-\x1f\x7f]/ : domain === "affix" ? /[\r\n]/ : /\r/).test(value);
const metadata = (row: Record<string, unknown>): boolean => ["hard", "soft"].includes(row.applicability as string) && ["low", "medium", "high"].includes(row.confidence as string);
function constraint(value: unknown): boolean {
  if (typeof value !== "object" || value === null) return false;
  const row = value as Record<string, unknown>;
  switch (row.type) {
    case "length": return record(row, ["type", "min", "max"]) && string(row.min, 256, "constraint") && string(row.max, 256, "constraint");
    case "characterCount": return record(row, ["type", "class", "min", "max"]) && ["lowercase", "uppercase", "digit", "space", "symbol"].includes(row.class as string) && string(row.min, 256, "constraint") && string(row.max, 256, "constraint");
    case "position": return record(row, ["type", "index", "allowed"]) && string(row.index, 256, "constraint") && string(row.allowed, 95, "constraint");
    case "contains": case "notContains": case "prefix": case "suffix": return record(row, ["type", "literal"]) && string(row.literal, 64, "constraint");
    default: return false;
  }
}
/** Defensive IPC response validation; disk parsing and matching remain in Rust. */
export function isProjectDocument(value: unknown): value is ProjectDocumentV1 {
  let nodes = 0, objects = 0, decoded = 0;
  function budget(item: unknown, depth: number): boolean {
    if (++nodes > 4096 || depth > 16) return false;
    if (typeof item === "string") { decoded += bytes(item); return !hasLoneSurrogate(item) && decoded <= 96 * 1024; }
    if (Array.isArray(item)) return item.length <= 4096 && item.every((child) => budget(child, depth + 1));
    if (typeof item === "object" && item !== null) {
      if (++objects > 256) return false;
      return Object.entries(item).every(([key, child]) => bytes(key) <= 64 && budget(key, depth + 1) && budget(child, depth + 1));
    }
    return item === null || typeof item === "boolean" || (typeof item === "number" && Number.isFinite(item));
  }
  if (!budget(value, 0) || !record(value, ["format", "schemaVersion", "clues", "hypotheses", "groups", "directConstraints", "manualSeeds", "options", "limits"]) || value.format !== "cluebench-project" || value.schemaVersion !== 1 ||
    !boundedArray(value.clues, 8) || !boundedArray(value.hypotheses, 8) || !boundedArray(value.groups, 8) || !boundedArray(value.directConstraints, 16) ||
    !record(value.manualSeeds, ["text"]) || !string(value.manualSeeds.text, 16 * 1024) ||
    !record(value.limits, ["attemptLimit", "emissionLimit"]) || !string(value.limits.attemptLimit, 256, "constraint") || !string(value.limits.emissionLimit, 256, "constraint") ||
    !record(value.options, ["caseVariants", "prefixes", "suffixes"]) || !boundedArray(value.options.caseVariants, 3) || new Set(value.options.caseVariants).size !== value.options.caseVariants.length || value.options.caseVariants.some((entry) => !["original", "asciiLowercase", "asciiUppercase"].includes(entry as string))) return false;
  const ids = new Map<string, string>();
  const identity = (value: unknown, kind: string): value is string => {
    if (typeof value !== "string" || !uuid.test(value) || ids.has(value) || ids.size >= 72) return false;
    ids.set(value, kind); return true;
  };
  if (!value.clues.every((row) => record(row, ["id", "text", "source"]) && identity(row.id, "clue") && string(row.text, 512) && string(row.source, 256)) ||
    !value.directConstraints.every((row) => record(row, ["id", "enabled", "applicability", "confidence", "constraint"]) && identity(row.id, "direct") && typeof row.enabled === "boolean" && metadata(row) && constraint(row.constraint))) return false;
  let effects = 0;
  if (!value.hypotheses.every((row) => record(row, ["id", "origin", "interpretation", "applicability", "confidence", "activation", "effects"]) && identity(row.id, "hypothesis") && record(row.origin, ["type", "clueId"]) && row.origin.type === "clue" && typeof row.origin.clueId === "string" && string(row.interpretation, 256) && metadata(row) &&
    (record(row.activation, ["type", "enabled"]) && row.activation.type === "independent" && typeof row.activation.enabled === "boolean" || record(row.activation, ["type", "groupId"]) && row.activation.type === "grouped" && typeof row.activation.groupId === "string") &&
    boundedArray(row.effects, 4) && (effects += row.effects.length) <= 16 && row.effects.every((effect) => record(effect, ["id", "constraint"]) && identity(effect.id, "effect") && constraint(effect.constraint))) ||
    !value.groups.every((row) => record(row, ["id", "anchorClueId", "selectedHypothesisId"]) && identity(row.id, "group") && typeof row.anchorClueId === "string" && (row.selectedHypothesisId === null || typeof row.selectedHypothesisId === "string"))) return false;
  for (const affix of [value.options.prefixes, value.options.suffixes]) {
    if (!record(affix, ["includeEmpty", "rows"]) || typeof affix.includeEmpty !== "boolean" || !boundedArray(affix.rows, 8) || !affix.rows.every((row) => record(row, ["id", "value"]) && identity(row.id, "affix") && string(row.value, 256, "affix"))) return false;
  }
  const doc = value as unknown as ProjectDocumentV1;
  for (const row of doc.hypotheses) {
    if (ids.get(row.origin.clueId) !== "clue") return false;
    if (row.activation.type === "grouped") {
      const groupId = row.activation.groupId;
      const group = doc.groups.find((item) => item.id === groupId);
      if (!group || group.anchorClueId !== row.origin.clueId) return false;
    }
  }
  for (const group of doc.groups) {
    if (ids.get(group.anchorClueId) !== "clue") return false;
    if (group.selectedHypothesisId !== null && !doc.hypotheses.some((row) => row.id === group.selectedHypothesisId && row.activation.type === "grouped" && row.activation.groupId === group.id)) return false;
  }
  return bytes(JSON.stringify(value)) <= 256 * 1024;
}

/** Reserve the entire monotonically increasing ID range before constructing drafts. */
export function stageProject(value: unknown, lastEditorId: number): { input: AuthoringInput; identities: Map<number, ProjectId>; lastEditorId: number } {
  if (!isProjectDocument(value) || !Number.isInteger(lastEditorId) || lastEditorId < 0) throw new Error("Invalid project.");
  const doc = value;
  const allIds = [...doc.clues, ...doc.hypotheses, ...doc.groups, ...doc.directConstraints, ...doc.hypotheses.flatMap((row) => row.effects), ...doc.options.prefixes.rows, ...doc.options.suffixes.rows].map((row) => row.id);
  if (lastEditorId + allIds.length > 0xffff_ffff) throw new Error("Editor identity capacity exceeded.");
  const mapping = new Map(allIds.map((id, index) => [id, lastEditorId + index + 1]));
  const identities = new Map([...mapping].map(([id, editorId]) => [editorId, id]));
  const editorId = (id: ProjectId) => mapping.get(id)!;
  const affix = (draft: AffixDraftV1): AffixEditorDraft => ({ includeEmpty: draft.includeEmpty, rows: draft.rows.map((row) => ({ editorId: editorId(row.id), value: row.value })) });
  const prefixDraft = affix(doc.options.prefixes), suffixDraft = affix(doc.options.suffixes);
  return { identities, lastEditorId: lastEditorId + allIds.length, input: {
    seedText: doc.manualSeeds.text, caseVariants: [...doc.options.caseVariants], prefixDraft, suffixDraft,
    prefixes: previewAffixes(prefixDraft), suffixes: previewAffixes(suffixDraft), attemptLimit: doc.limits.attemptLimit, emissionLimit: doc.limits.emissionLimit,
    clues: doc.clues.map((row) => ({ editorId: editorId(row.id), text: row.text, source: row.source })),
    hypotheses: doc.hypotheses.map((row) => ({ editorId: editorId(row.id), clueEditorId: editorId(row.origin.clueId), interpretation: row.interpretation,
      applicability: row.applicability, confidence: row.confidence, enabled: row.activation.type === "independent" ? row.activation.enabled : false,
      ...(row.activation.type === "grouped" ? { groupEditorId: editorId(row.activation.groupId) } : {}), effects: row.effects.map((effect) => ({ editorId: editorId(effect.id), constraint: { ...effect.constraint } })),
    })),
    groups: doc.groups.map((row) => ({ editorId: editorId(row.id), clueEditorId: editorId(row.anchorClueId), selectedHypothesisEditorId: row.selectedHypothesisId === null ? null : editorId(row.selectedHypothesisId) })),
    directConstraints: doc.directConstraints.map((row) => ({ editorId: editorId(row.id), enabled: row.enabled, applicability: row.applicability, confidence: row.confidence, constraint: { ...row.constraint } })),
  } };
}
