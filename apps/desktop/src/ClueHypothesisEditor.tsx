// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { ConstraintFields } from "./DirectConstraintEditor";
import { CONSTRAINT_TYPES, defaultConstraint } from "./direct-constraints";
import type { ConstraintType } from "./direct-constraints";
import type { ClueDraft, HypothesisDraft } from "./clue-hypotheses";
import { useRawTextGuard } from "./use-raw-text-guard";

interface EditorProps {
  clues: ClueDraft[];
  hypotheses: HypothesisDraft[];
  onCluesChange: (clues: ClueDraft[]) => void;
  onHypothesesChange: (hypotheses: HypothesisDraft[]) => void;
  onAddClue: () => void;
  onAddHypothesis: (clueEditorId: number) => void;
  onAddEffect: (hypothesisEditorId: number) => void;
  onRejected: (message: string) => void;
  onCompositionChange: (field: string, active: boolean) => void;
  canAdd: boolean;
}
function EvidenceInput({ label, value, maxBytes, fieldId, onChange, onRejected, onCompositionChange }: {
  label: string;
  value: string;
  maxBytes: number;
  fieldId: string;
  onChange: (value: string) => void;
  onRejected: EditorProps["onRejected"];
  onCompositionChange: EditorProps["onCompositionChange"];
}) {
  const guard = useRawTextGuard<HTMLTextAreaElement>("evidence", value, onChange, onRejected, {
    maxBytes, onCompositionChange: (active) => onCompositionChange(fieldId, active),
  });
  return <><label>{label}<textarea {...guard} rows={2} spellCheck={false} autoCorrect="off" autoCapitalize="off" autoComplete="off" /></label>
    <small>Up to {maxBytes} UTF-8 bytes. Empty text, spaces, LF, TAB, and Unicode are preserved.</small></>;
}

export default function ClueHypothesisEditor({ clues, hypotheses, onCluesChange, onHypothesesChange,
  onAddClue, onAddHypothesis, onAddEffect, onRejected, onCompositionChange, canAdd }: EditorProps) {
  const updateClue = (editorId: number, patch: Partial<ClueDraft>) => onCluesChange(clues.map((clue) => clue.editorId === editorId ? { ...clue, ...patch } : clue));
  const updateHypothesis = (editorId: number, patch: Partial<HypothesisDraft>) => onHypothesesChange(hypotheses.map((hypothesis) => hypothesis.editorId === editorId ? { ...hypothesis, ...patch } : hypothesis));
  const totalEffects = hypotheses.reduce((sum, hypothesis) => sum + hypothesis.effects.length, 0);
  return <section className="clue-editor" aria-labelledby="clue-editor-title">
    <h3 id="clue-editor-title">Clues and hypotheses</h3>
    <p className="hint">Keep original evidence separate from your interpretation. Multiple hypotheses can originate from the same Clue; equal values remain separate records.</p>
    <h4>Original Clues</h4>
    {clues.map((clue, index) => {
      const references = hypotheses.filter((hypothesis) => hypothesis.clueEditorId === clue.editorId).length;
      return <fieldset key={clue.editorId} className="clue-row"><legend>Clue {index + 1}</legend>
        <EvidenceInput label={`Clue ${index + 1} original text`} value={clue.text} maxBytes={512} fieldId={`clue-${clue.editorId}-text`}
          onChange={(text) => updateClue(clue.editorId, { text })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
        <EvidenceInput label={`Clue ${index + 1} source`} value={clue.source} maxBytes={256} fieldId={`clue-${clue.editorId}-source`}
          onChange={(source) => updateClue(clue.editorId, { source })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
        <p id={`clue-${clue.editorId}-references`} className="hint">{references} referencing hypotheses, including disabled drafts and hypotheses with zero effects.
          {references > 0 && " To remove this Clue, first remove its hypotheses or explicitly change their originating Clue."}</p>
        <div className="actions"><button type="button" className="secondary" disabled={references > 0} aria-describedby={`clue-${clue.editorId}-references`}
          aria-label={`Remove clue ${index + 1}`} onClick={() => {
            if (hypotheses.some((hypothesis) => hypothesis.clueEditorId === clue.editorId)) return;
            onCluesChange(clues.filter((current) => current.editorId !== clue.editorId));
          }}>Remove Clue</button>
          <button type="button" className="secondary" disabled={!canAdd || hypotheses.length >= 8} onClick={() => onAddHypothesis(clue.editorId)}>Add hypothesis for clue {index + 1}</button></div>
      </fieldset>;
    })}
    <button type="button" className="secondary" disabled={!canAdd || clues.length >= 8} onClick={onAddClue}>Add Clue</button>
    <p className="hint">{clues.length}/8 Clues. Evidence and interpretations share a 2,048-byte Unicode text budget. The entire canonical request must also fit within 16 KiB.</p>
    <h4>Interpretations and constraint effects</h4>
    {hypotheses.map((hypothesis, index) => <fieldset key={hypothesis.editorId} className="hypothesis-row">
      <legend>Hypothesis {index + 1} — {hypothesis.enabled ? "Enabled" : "Disabled"}</legend>
      <label>Hypothesis {index + 1} originating Clue<select value={hypothesis.clueEditorId} onChange={(event) => updateHypothesis(hypothesis.editorId, { clueEditorId: Number(event.target.value) })}>
        {clues.map((clue, clueIndex) => <option key={clue.editorId} value={clue.editorId}>Clue {clueIndex + 1}</option>)}
      </select></label>
      <EvidenceInput label={`Hypothesis ${index + 1} interpretation`} value={hypothesis.interpretation} maxBytes={256} fieldId={`hypothesis-${hypothesis.editorId}-interpretation`}
        onChange={(interpretation) => updateHypothesis(hypothesis.editorId, { interpretation })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
      <div className="constraint-settings">
        <label className="check"><input type="checkbox" checked={hypothesis.enabled} onChange={(event) => updateHypothesis(hypothesis.editorId, { enabled: event.target.checked })} />Enable hypothesis {index + 1}</label>
        <label>Hypothesis {index + 1} applicability<select value={hypothesis.applicability} onChange={(event) => updateHypothesis(hypothesis.editorId, { applicability: event.target.value as HypothesisDraft["applicability"] })}>
          <option value="hard">Hard — filter</option><option value="soft">Soft — relative points</option>
        </select></label>
        <label>Hypothesis {index + 1} confidence<select value={hypothesis.confidence} onChange={(event) => updateHypothesis(hypothesis.editorId, { confidence: event.target.value as HypothesisDraft["confidence"] })}>
          <option value="low">Low (1)</option><option value="medium">Medium (2)</option><option value="high">High (3)</option>
        </select></label>
      </div>
      {hypothesis.effects.length === 0 && <p className="hint">Zero effects: this hypothesis does not filter candidates or contribute Soft points. You may keep it as an interpretation draft.</p>}
      {hypothesis.effects.map((effect, effectIndex) => {
        const label = `Hypothesis ${index + 1} effect ${effectIndex + 1}`;
        return <fieldset key={effect.editorId} className="effect-row"><legend>{label}</legend>
          <label>{label} type<select value={effect.constraint.type} onChange={(event) => updateHypothesis(hypothesis.editorId, {
            effects: hypothesis.effects.map((current) => current.editorId === effect.editorId ? { ...current, constraint: defaultConstraint(event.target.value as ConstraintType) } : current),
          })}>{CONSTRAINT_TYPES.map((type) => <option key={type.value} value={type.value}>{type.label}</option>)}</select></label>
          <div className="constraint-fields"><ConstraintFields key={`${effect.editorId}:${effect.constraint.type}`} constraint={effect.constraint} label={label} fieldId={`effect-${effect.editorId}`} onCompositionChange={onCompositionChange}
            onChange={(constraint) => updateHypothesis(hypothesis.editorId, { effects: hypothesis.effects.map((current) => current.editorId === effect.editorId ? { ...current, constraint } : current) })} onRejected={onRejected} /></div>
          <button type="button" className="secondary" aria-label={`Remove hypothesis ${index + 1} effect ${effectIndex + 1}`}
            onClick={() => updateHypothesis(hypothesis.editorId, { effects: hypothesis.effects.filter((current) => current.editorId !== effect.editorId) })}>Remove effect</button>
        </fieldset>;
      })}
      <div className="actions"><button type="button" className="secondary" disabled={!canAdd || hypothesis.effects.length >= 4 || totalEffects >= 16}
        onClick={() => onAddEffect(hypothesis.editorId)}>Add effect to hypothesis {index + 1}</button>
        <button type="button" className="secondary" aria-label={`Remove hypothesis ${index + 1}`}
          onClick={() => onHypothesesChange(hypotheses.filter((current) => current.editorId !== hypothesis.editorId))}>Remove hypothesis</button></div>
    </fieldset>)}
    <p className="hint">{hypotheses.length}/8 hypotheses; {totalEffects}/16 Clue-backed effects, up to 4 per hypothesis. All effects of an enabled Hard hypothesis must match. An enabled Soft hypothesis scores once only when all its effects match: Low 1, Medium 2, High 3. Hard confidence has no effect. Disabled hypotheses and all their effects must still be valid before Preview.</p>
  </section>;
}
