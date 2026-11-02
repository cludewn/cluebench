// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { ConstraintFields } from "./DirectConstraintEditor";
import { CONSTRAINT_TYPES, defaultConstraint } from "./direct-constraints";
import type { ConstraintType } from "./direct-constraints";
import { isHypothesisActive, removeAlternativeGroup, removeHypothesis, selectAlternative, setHypothesisGroup, setHypothesisOrigin } from "./clue-hypotheses";
import type { AlternativeGroupDraft, ClueDraft, HypothesisDraft, ScenarioDraft } from "./clue-hypotheses";
import { useRawTextGuard } from "./use-raw-text-guard";

interface EditorProps {
  clues: ClueDraft[];
  hypotheses: HypothesisDraft[];
  groups: AlternativeGroupDraft[];
  onScenarioChange: (scenario: ScenarioDraft) => void;
  onAddGroup: (clueEditorId: number) => void;
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

export default function ClueHypothesisEditor({ clues, hypotheses, groups, onScenarioChange, onAddGroup, onCluesChange, onHypothesesChange,
  onAddClue, onAddHypothesis, onAddEffect, onRejected, onCompositionChange, canAdd }: EditorProps) {
  const updateClue = (editorId: number, patch: Partial<ClueDraft>) => onCluesChange(clues.map((clue) => clue.editorId === editorId ? { ...clue, ...patch } : clue));
  const updateHypothesis = (editorId: number, patch: Partial<HypothesisDraft>) => onHypothesesChange(hypotheses.map((hypothesis) => hypothesis.editorId === editorId ? { ...hypothesis, ...patch } : hypothesis));
  const scenario = { hypotheses, groups };
  const changeScenario = (next: ScenarioDraft) => { if (next !== scenario) onScenarioChange(next); };
  const totalEffects = hypotheses.reduce((sum, hypothesis) => sum + hypothesis.effects.length, 0);
  return <section className="clue-editor" aria-labelledby="clue-editor-title">
    <h3 id="clue-editor-title">Clues and hypotheses</h3>
    <p className="hint">Keep original evidence separate from your interpretation. Multiple hypotheses can originate from the same Clue; equal values remain separate records. Alternative Groups select one interpretation or None for one scenario. They do not evaluate logical OR or every combination.</p>
    <h4>Original Clues</h4>
    {clues.map((clue, index) => {
      const references = hypotheses.filter((hypothesis) => hypothesis.clueEditorId === clue.editorId).length;
      const anchoredGroups = groups.filter((group) => group.clueEditorId === clue.editorId);
      return <fieldset key={clue.editorId} className="clue-row"><legend>Clue {index + 1}</legend>
        <EvidenceInput label={`Clue ${index + 1} original text`} value={clue.text} maxBytes={512} fieldId={`clue-${clue.editorId}-text`}
          onChange={(text) => updateClue(clue.editorId, { text })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
        <EvidenceInput label={`Clue ${index + 1} source`} value={clue.source} maxBytes={256} fieldId={`clue-${clue.editorId}-source`}
          onChange={(source) => updateClue(clue.editorId, { source })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
        <p id={`clue-${clue.editorId}-references`} className="hint">{references} referencing hypotheses, including disabled drafts and hypotheses with zero effects.
          {references > 0 && " To remove this Clue, first remove its hypotheses or explicitly change their originating Clue."}
          {anchoredGroups.length > 0 && ` ${anchoredGroups.length} anchored Alternative Groups, including empty groups. Remove these groups before removing this Clue.`}</p>
        <div className="actions"><button type="button" className="secondary" disabled={references > 0 || anchoredGroups.length > 0} aria-describedby={`clue-${clue.editorId}-references`}
          aria-label={`Remove clue ${index + 1}`} onClick={() => {
            if (hypotheses.some((hypothesis) => hypothesis.clueEditorId === clue.editorId) || groups.some((group) => group.clueEditorId === clue.editorId)) return;
            onCluesChange(clues.filter((current) => current.editorId !== clue.editorId));
          }}>Remove Clue</button>
          <button type="button" className="secondary" disabled={!canAdd || hypotheses.length >= 8} onClick={() => onAddHypothesis(clue.editorId)}>Add hypothesis for clue {index + 1}</button>
          <button type="button" className="secondary" disabled={!canAdd || groups.length >= 8} onClick={() => onAddGroup(clue.editorId)}>Add group for clue {index + 1}</button></div>
        {anchoredGroups.map((group) => {
          const members = hypotheses.filter((hypothesis) => hypothesis.groupEditorId === group.editorId);
          const selected = members.find((hypothesis) => hypothesis.editorId === group.selectedHypothesisEditorId);
          return <fieldset key={group.editorId} className="alternative-group"><legend>Group {group.editorId}</legend>
            <p className="hint">Anchored to Clue {index + 1}. Select one member or None. Membership is edited on the Hypothesis cards below.</p>
            <label className="check"><input type="radio" name={`group-${group.editorId}-selection`} aria-label={`Group ${group.editorId} select None`}
              checked={group.selectedHypothesisEditorId === null} onChange={() => changeScenario(selectAlternative(scenario, group.editorId, null))} />None — no member active</label>
            {members.map((hypothesis) => <label className="check" key={hypothesis.editorId}><input type="radio" name={`group-${group.editorId}-selection`}
              aria-label={`Group ${group.editorId} select hypothesis ${hypotheses.indexOf(hypothesis) + 1}`} checked={group.selectedHypothesisEditorId === hypothesis.editorId}
              onChange={() => changeScenario(selectAlternative(scenario, group.editorId, hypothesis.editorId))} />Hypothesis {hypotheses.indexOf(hypothesis) + 1} — {group.selectedHypothesisEditorId === hypothesis.editorId ? "Selected" : "Unselected"}</label>)}
            {members.length === 0 && <p className="hint">Empty group: None is the only selection.</p>}
            <p>Selected interpretation: <span className="secret-text">{selected ? selected.interpretation || "(empty interpretation)" : "None"}</span></p>
            <button type="button" className="secondary" aria-label={`Remove group ${group.editorId}`} onClick={() => changeScenario(removeAlternativeGroup(scenario, group.editorId))}>Remove Group</button>
          </fieldset>;
        })}
      </fieldset>;
    })}
    <button type="button" className="secondary" disabled={!canAdd || clues.length >= 8} onClick={onAddClue}>Add Clue</button>
    <p className="hint">{clues.length}/8 Clues. Evidence and interpretations share a 2,048-byte Unicode text budget. The entire canonical request must also fit within 16 KiB.</p>
    <p className="hint">{groups.length}/8 Alternative Groups. Each Group belongs to one Clue. Joining a Group does not select its member. Leaving or deleting a Group returns affected members to Independent Disabled; no member is selected automatically.</p>
    <h4>Interpretations and constraint effects</h4>
    {hypotheses.map((hypothesis, index) => <fieldset key={hypothesis.editorId} className="hypothesis-row">
      <legend>Hypothesis {index + 1} — {hypothesis.groupEditorId === undefined ? `Independent ${hypothesis.enabled ? "Enabled" : "Disabled"}` : `Group ${hypothesis.groupEditorId} ${isHypothesisActive(hypothesis, groups) ? "Selected" : "Unselected"}`}</legend>
      <label>Hypothesis {index + 1} group<select value={hypothesis.groupEditorId ?? "independent"} onChange={(event) => changeScenario(setHypothesisGroup(scenario, hypothesis.editorId, event.target.value === "independent" ? undefined : Number(event.target.value)))}>
        <option value="independent">Independent</option>
        {groups.filter((group) => group.clueEditorId === hypothesis.clueEditorId).map((group) => <option key={group.editorId} value={group.editorId}>Group {group.editorId}</option>)}
      </select></label>
      {hypothesis.groupEditorId !== undefined && <p className="hint">Grouped activation uses the Group selection. Ungroup explicitly before changing the originating Clue; leaving returns this hypothesis to Independent Disabled.</p>}
      <label>Hypothesis {index + 1} originating Clue<select value={hypothesis.clueEditorId} disabled={hypothesis.groupEditorId !== undefined} onChange={(event) => changeScenario(setHypothesisOrigin(scenario, hypothesis.editorId, Number(event.target.value)))}>
        {clues.map((clue, clueIndex) => <option key={clue.editorId} value={clue.editorId}>Clue {clueIndex + 1}</option>)}
      </select></label>
      <EvidenceInput label={`Hypothesis ${index + 1} interpretation`} value={hypothesis.interpretation} maxBytes={256} fieldId={`hypothesis-${hypothesis.editorId}-interpretation`}
        onChange={(interpretation) => updateHypothesis(hypothesis.editorId, { interpretation })} onRejected={onRejected} onCompositionChange={onCompositionChange} />
      <div className="constraint-settings">
        {hypothesis.groupEditorId === undefined && <label className="check"><input type="checkbox" checked={hypothesis.enabled} onChange={(event) => updateHypothesis(hypothesis.editorId, { enabled: event.target.checked })} />Enable hypothesis {index + 1}</label>}
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
          onClick={() => changeScenario(removeHypothesis(scenario, hypothesis.editorId))}>Remove hypothesis</button></div>
    </fieldset>)}
    <p className="hint">{hypotheses.length}/8 hypotheses; {totalEffects}/16 Clue-backed effects, up to 4 per hypothesis. All effects of an active Hard hypothesis must match. An active Soft hypothesis scores once only when all its effects match: Low 1, Medium 2, High 3. Hard confidence has no effect. Disabled and unselected hypotheses and all their effects must still be valid before Preview.</p>
  </section>;
}
