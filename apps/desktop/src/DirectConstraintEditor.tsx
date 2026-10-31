// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { CHARACTER_CLASSES, CONSTRAINT_TYPES, DIRECT_CONSTRAINT_LIMITS, changeConstraintType } from "./direct-constraints";
import type { CharacterClass, ConstraintType, DirectConstraintDraft } from "./direct-constraints";
import { useRawTextGuard } from "./use-raw-text-guard";

interface EditorProps {
  constraints: DirectConstraintDraft[];
  onChange: (rows: DirectConstraintDraft[]) => void;
  onAdd: () => void;
  onRejected: (message: string) => void;
  canAdd?: boolean;
  onCompositionChange?: (field: string, active: boolean) => void;
}
interface FieldProps {
  label: string;
  value: string;
  numeric?: boolean;
  onChange: (value: string) => void;
  onRejected: (message: string) => void;
  maxBytes?: number;
  onCompositionChange?: (active: boolean) => void;
}
function ConstraintInput({ label, value, numeric = false, onChange, onRejected, maxBytes, onCompositionChange }: FieldProps) {
  const guard = useRawTextGuard<HTMLInputElement>("constraint", value, onChange, onRejected, { maxBytes, onCompositionChange });
  return <label>{label}<input {...guard} type="text" inputMode={numeric ? "numeric" : undefined}
    spellCheck={false} autoCorrect="off" autoCapitalize="off" autoComplete="off" /></label>;
}

export function ConstraintFields({ constraint: value, label, onChange, onRejected, fieldId, onCompositionChange }: {
  constraint: DirectConstraintDraft["constraint"];
  label: string;
  fieldId?: string;
  onCompositionChange?: (field: string, active: boolean) => void;
  onChange: (constraint: DirectConstraintDraft["constraint"]) => void;
  onRejected: (message: string) => void;
}) {
  const field = (name: string, text: string, change: (text: string) => void, numeric = false, maxBytes?: number) =>
    <ConstraintInput label={`${label} ${name}`} value={text} numeric={numeric} maxBytes={maxBytes} onChange={change} onRejected={onRejected}
      onCompositionChange={(active) => onCompositionChange?.(`${fieldId}:${name}`, active)} />;
  switch (value.type) {
    case "length":
    case "characterCount":
      return <>
        {value.type === "characterCount" && <label>{label} character class<select value={value.class}
          onChange={(event) => onChange({ ...value, class: event.target.value as CharacterClass })}>
          {CHARACTER_CLASSES.map((characterClass) => <option value={characterClass} key={characterClass}>{characterClass}</option>)}
        </select></label>}
        {field("minimum", value.min, (min) => onChange({ ...value, min }), true)}
        {field("maximum", value.max, (max) => onChange({ ...value, max }), true)}
        <p className="hint">Inclusive whole-number range: {value.type === "length" ? "1" : "0"} ≤ min ≤ max ≤ 64.{value.type === "characterCount" && " Space is separate from symbol."}</p>
      </>;
    case "position":
      return <>
        {field("position", value.index, (index) => onChange({ ...value, index }), true)}
        {field("allowed characters", value.allowed, (allowed) => onChange({ ...value, allowed }), false, 95)}
        <p className="hint">One-based position, 1–64. Allowed characters are alternatives (1–95 printable ASCII bytes), not a substring or regular expression. Duplicates are permitted.</p>
      </>;
    default:
      return <>
        {field("literal", value.literal, (literal) => onChange({ ...value, literal }), false, 64)}
        <p className="hint">1–64 printable ASCII bytes. Case and surrounding spaces are preserved.</p>
      </>;
  }
}

export default function DirectConstraintEditor({ constraints, onChange, onAdd, onRejected, canAdd = true, onCompositionChange }: EditorProps) {
  const update = (editorId: number, row: DirectConstraintDraft) => onChange(constraints.map((current) => current.editorId === editorId ? row : current));
  return <section className="direct-constraints" aria-labelledby="direct-constraints-title">
    <h3 id="direct-constraints-title">Direct Constraints</h3>
    <p className="hint">Hard conditions filter candidates. Soft conditions add relative points: Low 1, Medium 2, High 3. Points are weights, not probabilities. Hard confidence has no effect.</p>
    {constraints.map((row, index) => <fieldset className="constraint-row" key={row.editorId}>
      <legend>Condition {index + 1} — {row.enabled ? "Enabled" : "Disabled"}</legend>
      <div className="constraint-settings">
        <label className="check"><input type="checkbox" checked={row.enabled} onChange={(event) => update(row.editorId, { ...row, enabled: event.target.checked })} />Enable condition {index + 1}</label>
        <label>Condition {index + 1} type<select value={row.constraint.type} onChange={(event) => update(row.editorId, changeConstraintType(row, event.target.value as ConstraintType))}>
          {CONSTRAINT_TYPES.map((type) => <option value={type.value} key={type.value}>{type.label}</option>)}
        </select></label>
        <label>Condition {index + 1} applicability<select value={row.applicability} onChange={(event) => update(row.editorId, { ...row, applicability: event.target.value as DirectConstraintDraft["applicability"] })}>
          <option value="hard">Hard — filter</option><option value="soft">Soft — relative points</option>
        </select></label>
        <label>Condition {index + 1} confidence<select value={row.confidence} onChange={(event) => update(row.editorId, { ...row, confidence: event.target.value as DirectConstraintDraft["confidence"] })}>
          <option value="low">Low (1)</option><option value="medium">Medium (2)</option><option value="high">High (3)</option>
        </select></label>
      </div>
      <div className="constraint-fields"><ConstraintFields key={`${row.editorId}:${row.constraint.type}`} constraint={row.constraint} label={`Condition ${index + 1}`} fieldId={`direct-${row.editorId}`} onCompositionChange={onCompositionChange}
        onChange={(constraint) => update(row.editorId, { ...row, constraint })} onRejected={onRejected} /></div>
      <button type="button" className="secondary" aria-label={`Remove condition ${index + 1}`} onClick={() => onChange(constraints.filter((current) => current.editorId !== row.editorId))}>Remove</button>
    </fieldset>)}
    <button type="button" className="secondary" onClick={onAdd} disabled={!canAdd || constraints.length >= DIRECT_CONSTRAINT_LIMITS.rows}>Add condition</button>
    <p className="hint">{constraints.length}/16 conditions, including disabled rows. Disabled conditions do not filter or score; every row must still be valid before Preview. Correct or remove invalid disabled rows. No conditions preserves the manual-seed workflow.</p>
  </section>;
}
