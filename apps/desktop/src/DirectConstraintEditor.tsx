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
}
interface FieldProps {
  label: string;
  value: string;
  numeric?: boolean;
  onChange: (value: string) => void;
  onRejected: (message: string) => void;
}
function ConstraintInput({ label, value, numeric = false, onChange, onRejected }: FieldProps) {
  const guard = useRawTextGuard<HTMLInputElement>("constraint", value, onChange, onRejected);
  return <label>{label}<input {...guard} type="text" inputMode={numeric ? "numeric" : undefined} value={value}
    spellCheck={false} autoCorrect="off" autoCapitalize="off" autoComplete="off" /></label>;
}

function ConstraintFields({ row, number, onChange, onRejected }: {
  row: DirectConstraintDraft;
  number: number;
  onChange: (row: DirectConstraintDraft) => void;
  onRejected: (message: string) => void;
}) {
  const value = row.constraint;
  const field = (label: string, text: string, change: (text: string) => void, numeric = false) =>
    <ConstraintInput label={`Condition ${number} ${label}`} value={text} numeric={numeric} onChange={change} onRejected={onRejected} />;
  switch (value.type) {
    case "length":
    case "characterCount":
      return <>
        {value.type === "characterCount" && <label>Condition {number} character class<select value={value.class}
          onChange={(event) => onChange({ ...row, constraint: { ...value, class: event.target.value as CharacterClass } })}>
          {CHARACTER_CLASSES.map((characterClass) => <option value={characterClass} key={characterClass}>{characterClass}</option>)}
        </select></label>}
        {field("minimum", value.min, (min) => onChange({ ...row, constraint: { ...value, min } }), true)}
        {field("maximum", value.max, (max) => onChange({ ...row, constraint: { ...value, max } }), true)}
        <p className="hint">Inclusive whole-number range: {value.type === "length" ? "1" : "0"} ≤ min ≤ max ≤ 64.{value.type === "characterCount" && " Space is separate from symbol."}</p>
      </>;
    case "position":
      return <>
        {field("position", value.index, (index) => onChange({ ...row, constraint: { ...value, index } }), true)}
        {field("allowed characters", value.allowed, (allowed) => onChange({ ...row, constraint: { ...value, allowed } }))}
        <p className="hint">One-based position, 1–64. Allowed characters are alternatives (1–95 printable ASCII bytes), not a substring or regular expression. Duplicates are permitted.</p>
      </>;
    default:
      return <>
        {field("literal", value.literal, (literal) => onChange({ ...row, constraint: { ...value, literal } }))}
        <p className="hint">1–64 printable ASCII bytes. Case and surrounding spaces are preserved.</p>
      </>;
  }
}

export default function DirectConstraintEditor({ constraints, onChange, onAdd, onRejected, canAdd = true }: EditorProps) {
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
      <div className="constraint-fields"><ConstraintFields key={`${row.editorId}:${row.constraint.type}`} row={row} number={index + 1}
        onChange={(next) => update(row.editorId, next)} onRejected={onRejected} /></div>
      <button type="button" className="secondary" aria-label={`Remove condition ${index + 1}`} onClick={() => onChange(constraints.filter((current) => current.editorId !== row.editorId))}>Remove</button>
    </fieldset>)}
    <button type="button" className="secondary" onClick={onAdd} disabled={!canAdd || constraints.length >= DIRECT_CONSTRAINT_LIMITS.rows}>Add condition</button>
    <p className="hint">{constraints.length}/16 conditions, including disabled rows. Disabled conditions do not filter or score; every row must still be valid before Preview. Correct or remove invalid disabled rows. No conditions preserves the manual-seed workflow.</p>
  </section>;
}
