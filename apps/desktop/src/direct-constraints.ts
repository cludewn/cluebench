// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

export const DIRECT_CONSTRAINT_LIMITS = {
  rows: 16,
  literalBytes: 64,
  allowedBytes: 95,
  stringBytes: 1520,
  maxEditorId: 0xffff_ffff,
} as const;

export const CONSTRAINT_TYPES = [
  { value: "length", label: "Length" },
  { value: "position", label: "Position" },
  { value: "contains", label: "Contains" },
  { value: "notContains", label: "NotContains" },
  { value: "prefix", label: "Prefix" },
  { value: "suffix", label: "Suffix" },
  { value: "characterCount", label: "CharacterCount" },
] as const;
export const CHARACTER_CLASSES = ["lowercase", "uppercase", "digit", "space", "symbol"] as const;
export type ConstraintType = (typeof CONSTRAINT_TYPES)[number]["value"];
export type CharacterClass = (typeof CHARACTER_CLASSES)[number];
type ConstraintValue<T> =
  | { type: "length"; min: T; max: T }
  | { type: "position"; index: T; allowed: string }
  | { type: "contains" | "notContains" | "prefix" | "suffix"; literal: string }
  | { type: "characterCount"; class: CharacterClass; min: T; max: T };
interface ConditionMetadata {
  editorId: number;
  enabled: boolean;
  applicability: "hard" | "soft";
  confidence: "low" | "medium" | "high";
}
export interface DirectConstraintDraft extends ConditionMetadata { constraint: ConstraintValue<string> }
export interface DirectConstraint extends ConditionMetadata { constraint: ConstraintValue<number> }

function defaultConstraint(type: ConstraintType): ConstraintValue<string> {
  switch (type) {
    case "length": return { type, min: "1", max: "64" };
    case "position": return { type, index: "1", allowed: "" };
    case "characterCount": return { type, class: "digit", min: "0", max: "64" };
    default: return { type, literal: "" };
  }
}
export function createDefaultDirectConstraint(editorId: number): DirectConstraintDraft {
  return { editorId, enabled: true, applicability: "hard", confidence: "medium", constraint: defaultConstraint("length") };
}
export function changeConstraintType(row: DirectConstraintDraft, type: ConstraintType): DirectConstraintDraft {
  return { ...row, constraint: defaultConstraint(type) };
}
const integer = (value: string, min: number, max: number): boolean =>
  typeof value === "string" && /^[0-9]+$/.test(value) && Number.isSafeInteger(Number(value)) && Number(value) >= min && Number(value) <= max;
const printable = (value: string, max: number): boolean => typeof value === "string" && value.length >= 1 && value.length <= max && /^[\x20-\x7e]+$/.test(value);

/** Validate authoring syntax and resource bounds only; Core owns all matching. */
export function validateDirectConstraints(rows: readonly DirectConstraintDraft[]): string[] {
  const errors: string[] = [];
  if (rows.length > DIRECT_CONSTRAINT_LIMITS.rows) errors.push("Use at most 16 direct conditions, including disabled rows.");
  const ids = new Set<number>();
  let stringBytes = 0;
  rows.forEach((row, index) => {
    const name = `Condition ${index + 1}`;
    if (!Number.isInteger(row.editorId) || row.editorId < 1 || row.editorId > DIRECT_CONSTRAINT_LIMITS.maxEditorId || ids.has(row.editorId)) {
      errors.push(`${name} must have a distinct nonzero editor ID.`);
    }
    ids.add(row.editorId);
    if (typeof row.enabled !== "boolean" || !["hard", "soft"].includes(row.applicability) || !["low", "medium", "high"].includes(row.confidence)) {
      errors.push(`${name} has invalid condition settings.`);
    }
    const value = row.constraint;
    switch (value.type) {
      case "length":
      case "characterCount": {
        const minimum = value.type === "length" ? 1 : 0;
        if (!integer(value.min, minimum, 64) || !integer(value.max, minimum, 64) || Number(value.min) > Number(value.max)) {
          errors.push(`${name} requires whole-number bounds with ${minimum} ≤ min ≤ max ≤ 64.`);
        }
        if (value.type === "characterCount" && !CHARACTER_CLASSES.includes(value.class)) errors.push(`${name} requires a supported character class.`);
        break;
      }
      case "position":
        if (!integer(value.index, 1, 64)) errors.push(`${name} requires a whole-number position from 1 to 64.`);
        if (!printable(value.allowed, DIRECT_CONSTRAINT_LIMITS.allowedBytes)) errors.push(`${name} allowed characters must contain 1–95 printable ASCII bytes.`);
        stringBytes += new TextEncoder().encode(value.allowed).byteLength;
        break;
      case "contains":
      case "notContains":
      case "prefix":
      case "suffix":
        if (!printable(value.literal, DIRECT_CONSTRAINT_LIMITS.literalBytes)) errors.push(`${name} literal must contain 1–64 printable ASCII bytes.`);
        stringBytes += new TextEncoder().encode(value.literal).byteLength;
        break;
      default: errors.push(`${name} requires a supported condition type.`);
    }
  });
  if (stringBytes > DIRECT_CONSTRAINT_LIMITS.stringBytes) errors.push("Direct condition strings must total at most 1,520 bytes.");
  return errors;
}

function metadata(row: ConditionMetadata): ConditionMetadata {
  return { editorId: row.editorId, enabled: row.enabled, applicability: row.applicability, confidence: row.confidence };
}

/** Construct the explicit canonical DTO after validation; no stale values survive. */
export function serializeDirectConstraints(rows: readonly DirectConstraintDraft[]): DirectConstraint[] {
  if (validateDirectConstraints(rows).length > 0) throw new Error("Invalid direct conditions.");
  return rows.map((row) => {
    const value = row.constraint;
    let constraint: DirectConstraint["constraint"];
    switch (value.type) {
      case "length": constraint = { type: value.type, min: Number(value.min), max: Number(value.max) }; break;
      case "characterCount": constraint = { type: value.type, class: value.class, min: Number(value.min), max: Number(value.max) }; break;
      case "position": constraint = { type: value.type, index: Number(value.index), allowed: value.allowed }; break;
      default: constraint = { type: value.type, literal: value.literal };
    }
    return { ...metadata(row), constraint };
  });
}

export function canonicalDirectConstraints(rows: readonly DirectConstraint[]): DirectConstraint[] {
  const drafts: DirectConstraintDraft[] = rows.map((row) => {
    const value = row.constraint;
    let constraint: DirectConstraintDraft["constraint"];
    switch (value.type) {
      case "length": constraint = { type: value.type, min: String(value.min), max: String(value.max) }; break;
      case "characterCount": constraint = { type: value.type, class: value.class, min: String(value.min), max: String(value.max) }; break;
      case "position": constraint = { type: value.type, index: String(value.index), allowed: value.allowed }; break;
      default: constraint = { type: value.type, literal: value.literal };
    }
    // Runtime numbers must remain numbers; strings must not be coerced into the DTO.
    if (("min" in value && (typeof value.min !== "number" || typeof value.max !== "number")) ||
      ("index" in value && typeof value.index !== "number")) throw new Error("Invalid direct conditions.");
    return { ...metadata(row), constraint };
  });
  return serializeDirectConstraints(drafts);
}

/** Describe submitted condition data for provenance, never infer a match. */
export function describeDirectConstraint(row: DirectConstraint): string {
  const value = row.constraint;
  switch (value.type) {
    case "length": return `Length ${value.min}–${value.max}`;
    case "position": return `Position ${value.index}: allowed “${value.allowed}”`;
    case "characterCount": return `CharacterCount ${value.class}: ${value.min}–${value.max}`;
    default: return `${CONSTRAINT_TYPES.find((type) => type.value === value.type)?.label}: “${value.literal}”`;
  }
}
