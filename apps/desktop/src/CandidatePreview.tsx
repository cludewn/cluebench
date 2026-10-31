// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useEffect, useId, useRef, useState } from "react";
import { CASE_OPTIONS, PREVIEW_LIMITS, createPreviewRequest, validatePreviewInput } from "./preview-input";
import type { CaseVariant, PreviewInput } from "./preview-input";
import { PreviewError, runCandidatePreview, safePreviewError } from "./preview-bridge";
import type { PreviewResponse } from "./preview-bridge";
import { useRawTextGuard } from "./use-raw-text-guard";
import DirectConstraintEditor from "./DirectConstraintEditor";
import ClueHypothesisEditor from "./ClueHypothesisEditor";
import { createDefaultEffect, createDefaultHypothesis } from "./clue-hypotheses";
import { createDefaultDirectConstraint, describeDirectConstraint } from "./direct-constraints";

const defaults = (): PreviewInput => ({ seedText: "", caseVariants: ["original"], prefixes: [""], suffixes: [""], attemptLimit: "2048", emissionLimit: "50", directConstraints: [], clues: [], hypotheses: [] });
const stopLabels = { completed: "Completed", attemptLimit: "AttemptLimit", outputLimit: "OutputLimit", cancelled: "Cancelled", consumerStopped: "ConsumerStopped" } as const;
const caseLabel = (value: CaseVariant) => CASE_OPTIONS.find((option) => option.value === value)?.label;

function SeedInput({ value, onChange, onRejected, onCompositionChange }: { value: string; onChange: (value: string) => void; onRejected: (message: string) => void; onCompositionChange: (field: string, active: boolean) => void }) {
  const fieldId = useId();
  const guard = useRawTextGuard<HTMLTextAreaElement>("seeds", value, onChange, onRejected, { onCompositionChange: (active) => onCompositionChange(`seeds-${fieldId}`, active) });
  return <textarea {...guard} id="seeds" rows={5} spellCheck={false} autoCorrect="off" autoCapitalize="off" autoComplete="off" aria-describedby="seed-help" />;
}

function AffixInput({ value, onChange, onRejected, onCompositionChange }: { value: string; onChange: (value: string) => void; onRejected: (message: string) => void; onCompositionChange: (field: string, active: boolean) => void }) {
  const fieldId = useId();
  const guard = useRawTextGuard<HTMLInputElement>("affix", value, onChange, onRejected, { onCompositionChange: (active) => onCompositionChange(`affix-${fieldId}`, active) });
  return <input {...guard} spellCheck={false} autoCorrect="off" autoCapitalize="off" autoComplete="off" />;
}

function AffixOptions({ name, onChange, onRejected, onCompositionChange }: { name: "Prefix" | "Suffix"; onChange: (values: string[], hasBlankRows: boolean) => void; onRejected: (message: string) => void; onCompositionChange: (field: string, active: boolean) => void }) {
  const [noAffix, setNoAffix] = useState(true);
  const [rows, setRows] = useState<{ id: number; value: string }[]>([]);
  const rowSequence = useRef(0);
  function change(nextRows: typeof rows, nextNoAffix = noAffix) {
    setRows(nextRows); setNoAffix(nextNoAffix);
    onChange([...(nextNoAffix ? [""] : []), ...nextRows.map((row) => row.value).filter((value) => value !== "")], nextRows.some((row) => row.value === ""));
  }
  return <fieldset>
    <legend>{name} options</legend>
    <label className="check"><input type="checkbox" checked={noAffix} onChange={(event) => change(rows, event.target.checked)} />No {name.toLowerCase()}</label>
    {rows.map((row, index) => <div className="affix-row" key={row.id}>
      <label>{name} {index + 1}<AffixInput value={row.value} onRejected={onRejected} onCompositionChange={onCompositionChange} onChange={(value) => change(rows.map((current) => current.id === row.id ? { ...current, value } : current))} /></label>
      <button type="button" className="secondary" aria-label={`Remove ${name.toLowerCase()} ${index + 1}`} onClick={() => change(rows.filter((current) => current.id !== row.id))}>Remove</button>
    </div>)}
    <button type="button" className="secondary" disabled={rows.length + Number(noAffix) >= 8} onClick={() => change([...rows, { id: ++rowSequence.current, value: "" }])}>Add {name.toLowerCase()}</button>
    <p className="hint">Up to 8 ordered options, 0–64 printable ASCII bytes each. Spaces are preserved. The explicit no-affix option comes first. New rows require text; remove unused rows.</p>
  </fieldset>;
}

function PreviewResult({ result, request }: { result: PreviewResponse; request: ReturnType<typeof createPreviewRequest> }) {
  const empty = result.candidates.length === 0;
  const limited = result.stop === "attemptLimit" || result.stop === "outputLimit";
  return <section aria-label="Preview result">
    <p className={limited ? "limited" : "success"}>{empty ? "Empty: no candidates were emitted." : "Success: preview results received."}{limited && " Limited: a configured allowance stopped the preview."}</p>
    <dl className="stats">
      <dt>Termination reason</dt><dd>{stopLabels[result.stop]}</dd>
      <dt>Attempted paths</dt><dd>{result.stats.attemptedPaths}</dd>
      <dt>Domain skips</dt><dd>{result.stats.domainSkips}</dd>
      <dt>Hard rejections</dt><dd>{result.stats.hardRejections}</dd>
      <dt>Emitted candidates</dt><dd>{result.stats.emittedCandidates}</dd>
      <dt>Potential paths</dt><dd>{result.potentialPaths}</dd>
      <dt>Effective attempt limit</dt><dd>{result.effectiveLimits.attemptLimit}</dd>
      <dt>Effective emission limit</dt><dd>{result.effectiveLimits.emissionLimit}</dd>
    </dl>
    <p className="hint">Completed means all selected generation paths were examined. It does not cover the entire password space. Zero candidates does not prove unsatisfiability.</p>
    {result.candidates.length > 0 && <div className="candidate-scroll"><table>
      <caption>Candidate records in Core generation order</caption>
      <thead><tr><th>Candidate</th><th>Seed / path</th><th>Transformation</th><th>Relative points</th><th>Matched Soft conditions</th></tr></thead>
      <tbody>{result.candidates.map((candidate) => <tr key={[candidate.path.seedIndex, candidate.path.caseIndex, candidate.path.prefixIndex, candidate.path.suffixIndex].join(":")}>
        <td><code className="secret-text">{candidate.text}</code></td>
        <td>Seed {candidate.seedId} (line {candidate.path.seedIndex + 1})<br /><small>Path {candidate.path.seedIndex}/{candidate.path.caseIndex}/{candidate.path.prefixIndex}/{candidate.path.suffixIndex}</small></td>
        <td>{caseLabel(candidate.path.caseVariant)}<br />Prefix: <code className="secret-text">{candidate.prefix === "" ? "(none)" : candidate.prefix}</code><br />Suffix: <code className="secret-text">{candidate.suffix === "" ? "(none)" : candidate.suffix}</code></td>
        <td>{candidate.relativePoints}</td>
        <td>{candidate.matchedSoftHypothesisIds.length === 0 ? "None" : <ul>{candidate.matchedSoftHypothesisIds.map((id) => {
          const binding = result.constraintBindings.find((item) => item.hypothesisId === id);
          if (binding) {
            const condition = request.directConstraints.find((item) => item.editorId === binding.editorId)!;
            return <li key={id}>Condition {request.directConstraints.indexOf(condition) + 1}: <span className="secret-text">{describeDirectConstraint(condition)}</span> ({condition.confidence})</li>;
          }
          const hypothesisBinding = result.hypothesisBindings.find((item) => item.hypothesisId === id)!;
          const hypothesis = request.hypotheses.find((item) => item.editorId === hypothesisBinding.editorId)!;
          const clue = request.clues.find((item) => item.editorId === hypothesis.clueEditorId)!;
          return <li key={id}>Hypothesis {request.hypotheses.indexOf(hypothesis) + 1} ({hypothesis.confidence}):
            <dl className="provenance"><dt>Interpretation</dt><dd className="secret-text">{hypothesis.interpretation}</dd>
              <dt>Original Clue {request.clues.indexOf(clue) + 1}</dt><dd className="secret-text">{clue.text}</dd>
              <dt>Source</dt><dd className="secret-text">{clue.source}</dd></dl>
          </li>;
        })}</ul>}</td>
      </tr>)}</tbody>
    </table></div>}
    <p className="hint">Seed and Core hypothesis IDs belong only to this request. Path indices are zero-based. Equal candidate text from different paths stays separate. Soft confidence contributes Low 1, Medium 2, or High 3 relative points once per matched hypothesis when all its effects match; points are not success probabilities and do not change generation order. Hard rejections are aggregate counts; individual reasons are unavailable.</p>
  </section>;
}

export default function CandidatePreview() {
  const [input, setInput] = useState<PreviewInput>(defaults);
  const [dirty, setDirty] = useState(false);
  const [affixBlank, setAffixBlank] = useState({ prefix: false, suffix: false });
  const [clearSequence, setClearSequence] = useState(0);
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<{ response: PreviewResponse; request: ReturnType<typeof createPreviewRequest> } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [inputRejection, setInputRejection] = useState<string | null>(null);
  const [composingFields, setComposingFields] = useState<Set<string>>(() => new Set());
  const revision = useRef(0);
  const active = useRef<{ id: string; revision: number } | null>(null);
  const mounted = useRef(false);
  const editorSequence = useRef(0);
  const requestSequence = useRef(0);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const errors = validatePreviewInput(input, `preview-${requestSequence.current + 1}`);
  if (composingFields.size > 0) errors.push("Finish IME composition before Preview; intermediate text is not submitted evidence.");
  if (inputRejection) errors.push(inputRejection);
  if (affixBlank.prefix) errors.push("Enter text in every prefix row, or remove the row.");
  if (affixBlank.suffix) errors.push("Enter text in every suffix row, or remove the row.");
  function edit(patch: Partial<PreviewInput>) {
    revision.current += 1;
    setInput((current) => ({ ...current, ...patch }));
    setDirty(true); setResult(null); setError(null); setInputRejection(null);
  }
  function rejectInput(message: string) {
    revision.current += 1;
    setDirty(true); setResult(null); setError(null); setInputRejection(message);
  }
  function compositionChange(field: string, composing: boolean) {
    if (!mounted.current) return;
    if (composing) {
      revision.current += 1;
      setDirty(true); setResult(null); setError(null); setInputRejection(null);
    }
    setComposingFields((current) => {
      const next = new Set(current);
      if (composing) next.add(field); else next.delete(field);
      return next;
    });
  }
  function allocateEditorId(): number | null {
    if (editorSequence.current >= 0xffff_ffff) return null;
    editorSequence.current += 1;
    return editorSequence.current;
  }
  function addConstraint() {
    if (input.directConstraints.length >= PREVIEW_LIMITS.directConstraints) return;
    const editorId = allocateEditorId();
    if (editorId !== null) edit({ directConstraints: [...input.directConstraints, createDefaultDirectConstraint(editorId)] });
  }
  function addClue() {
    if (input.clues.length >= 8) return;
    const editorId = allocateEditorId();
    if (editorId !== null) edit({ clues: [...input.clues, { editorId, text: "", source: "" }] });
  }
  function addHypothesis(clueEditorId: number) {
    if (input.hypotheses.length >= 8 || !input.clues.some((clue) => clue.editorId === clueEditorId)) return;
    const editorId = allocateEditorId();
    if (editorId !== null) edit({ hypotheses: [...input.hypotheses, createDefaultHypothesis(editorId, clueEditorId)] });
  }
  function addEffect(hypothesisEditorId: number) {
    const hypothesis = input.hypotheses.find((row) => row.editorId === hypothesisEditorId);
    if (!hypothesis || hypothesis.effects.length >= 4 || input.hypotheses.reduce((sum, row) => sum + row.effects.length, 0) >= 16) return;
    const editorId = allocateEditorId();
    if (editorId !== null) edit({ hypotheses: input.hypotheses.map((row) => row.editorId === hypothesisEditorId ?
      { ...row, effects: [...row.effects, createDefaultEffect(editorId)] } : row) });
  }
  function clear() {
    revision.current += 1;
    setInput(defaults()); setComposingFields(new Set()); setAffixBlank({ prefix: false, suffix: false }); setClearSequence((value) => value + 1); setDirty(false); setResult(null); setError(null); setInputRejection(null);
  }
  async function preview() {
    if (active.current || errors.length > 0) return;
    const request = createPreviewRequest(input, `preview-${++requestSequence.current}`);
    const current = { id: request.requestId, revision: revision.current };
    active.current = current;
    setPending(true); setResult(null); setError(null);
    try {
      const response = await runCandidatePreview(request);
      if (response.requestId !== current.id) throw new PreviewError("INVALID_RESPONSE");
      if (mounted.current && active.current?.id === current.id && revision.current === current.revision) setResult({ response, request });
    } catch (failure: unknown) {
      if (mounted.current && active.current?.id === current.id && revision.current === current.revision) setError(safePreviewError(failure));
    } finally {
      if (mounted.current && active.current?.id === current.id) {
        active.current = null;
        setPending(false);
      }
    }
  }
  const status = pending ? "Loading" : dirty && errors.length > 0 ? "Invalid" : error ? "Error" : result ? (result.response.candidates.length === 0 ? "Empty" : result.response.stop === "attemptLimit" || result.response.stop === "outputLimit" ? "Limited" : "Success") : "Idle";
  return <section className="preview" aria-labelledby="preview-title">
    <h2 id="preview-title">Candidate Preview</h2>
    <p>Enter manual seeds and preview a bounded set of candidates generated by Rust Core.</p>
    <label htmlFor="seeds">Manual seeds — one per line</label>
    <SeedInput key={clearSequence} value={input.seedText} onChange={(seedText) => edit({ seedText })} onRejected={rejectInput} onCompositionChange={compositionChange} />
    <p id="seed-help" className="hint">1–64 seeds, each 1–64 printable ASCII bytes (4,096 bytes total). Spaces and duplicate lines are preserved. LF/CRLF separate lines; one final newline is a terminator. Internal empty lines, tabs, controls, and non-ASCII text are invalid.</p>
    <fieldset><legend>Case variants</legend>{CASE_OPTIONS.map((option) => <label className="check" key={option.value}><input type="checkbox" checked={input.caseVariants.includes(option.value)} onChange={(event) => edit({ caseVariants: event.target.checked ? [...input.caseVariants, option.value] : input.caseVariants.filter((value) => value !== option.value) })} />{option.label}</label>)}</fieldset>
    <div className="option-grid"><AffixOptions key={`prefix-${clearSequence}`} name="Prefix" onRejected={rejectInput} onCompositionChange={compositionChange} onChange={(prefixes, blank) => { setAffixBlank((current) => ({ ...current, prefix: blank })); edit({ prefixes }); }} /><AffixOptions key={`suffix-${clearSequence}`} name="Suffix" onRejected={rejectInput} onCompositionChange={compositionChange} onChange={(suffixes, blank) => { setAffixBlank((current) => ({ ...current, suffix: blank })); edit({ suffixes }); }} /></div>
    <p className="hint">Only selected case variants are used, in the order shown. Generation order: Seed → Case → Prefix → Suffix. Affixes keep their case. Results outside the 1–64-byte domain are skipped, never truncated.</p>
    <DirectConstraintEditor constraints={input.directConstraints} onAdd={addConstraint} canAdd={editorSequence.current < 0xffff_ffff} onChange={(directConstraints) => edit({ directConstraints })} onRejected={rejectInput} onCompositionChange={compositionChange} />
    <ClueHypothesisEditor clues={input.clues} hypotheses={input.hypotheses} onCluesChange={(clues) => edit({ clues })}
      onHypothesesChange={(hypotheses) => edit({ hypotheses })} onAddClue={addClue} onAddHypothesis={addHypothesis} onAddEffect={addEffect}
      onRejected={rejectInput} onCompositionChange={compositionChange} canAdd={editorSequence.current < 0xffff_ffff} />
    <div className="option-grid"><label>Attempt limit<input inputMode="numeric" value={input.attemptLimit} onChange={(event) => edit({ attemptLimit: event.target.value })} /><small>Default {PREVIEW_LIMITS.defaultAttempts}; maximum {PREVIEW_LIMITS.maxAttempts}.</small></label><label>Emission limit<input inputMode="numeric" value={input.emissionLimit} onChange={(event) => edit({ emissionLimit: event.target.value })} /><small>Default {PREVIEW_LIMITS.defaultEmissions}; maximum {PREVIEW_LIMITS.maxEmissions}.</small></label></div>
    {dirty && errors.length > 0 && <div role="alert"><ul>{errors.map((message) => <li key={message}>{message}</li>)}</ul></div>}
    <div className="actions"><button type="button" onClick={preview} disabled={pending || errors.length > 0}>{pending ? "Preview running…" : "Preview candidates"}</button><button type="button" className="secondary" onClick={clear}>Clear inputs and results</button></div>
    <div role="status" aria-label="Preview status" aria-live="polite" aria-busy={pending}>
      <p>State: {status}</p>
      {pending && <p>Loading: Rust is processing the request. Edits and Clear discard obsolete results; they do not cancel generation. Wait for the request to finish before running another preview.</p>}
      {error && <p className="error">{error}</p>}
      {result && <PreviewResult result={result.response} request={result.request} />}
    </div>
    <p className="hint">One-shot local IPC: at most 16 KiB per request, 64 KiB per response, 100 candidate records, and one Rust preview at a time with no waiting queue. Attempts and emissions are independent limits. Interactive cancellation is unavailable.</p>
    <p className="hint">Seeds, Clues, sources, interpretations, conditions, and candidates are kept in memory for this view, without logging, automatic saving, clipboard copying, or external transmission. Clear reduces retained references; it does not guarantee secure memory erasure. Transport allocations precede the adapter's raw-body limit.</p>
  </section>;
}
