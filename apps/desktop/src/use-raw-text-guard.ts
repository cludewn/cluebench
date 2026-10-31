// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useEffect, useRef, useState } from "react";
import type { ChangeEvent, ClipboardEvent, CompositionEvent, DragEvent } from "react";
import { hasLoneSurrogate, unicodeEvidenceBytes } from "./unicode-text";

type TextField = HTMLInputElement | HTMLTextAreaElement;
type TextDomain = "seeds" | "affix" | "constraint" | "evidence";
interface GuardOptions {
  maxBytes?: number;
  onCompositionChange?: (active: boolean) => void;
}
const messages = {
  seeds: "Input rejected. Use LF or CRLF between seed lines; a lone CR is not allowed.",
  affix: "Input rejected. Prefix and suffix options cannot contain CR or LF.",
  constraint: "Input rejected. Direct condition fields cannot contain control characters.",
  evidence: "Input rejected. Evidence requires Unicode scalar text and LF line endings; CR is not allowed.",
} as const;
const oversized = "Input rejected. This field exceeds its UTF-8 byte limit; the previous value was retained.";
function forbiddenSource(domain: TextDomain, text: string): boolean {
  if (hasLoneSurrogate(text)) return true;
  if (domain === "evidence") return /\r/.test(text);
  if (domain === "constraint") return /[\x00-\x1f\x7f]/.test(text);
  return domain === "seeds" ? /\r(?!\n)/.test(text) : /[\r\n]/.test(text);
}

/** Inspect observable source text before DOM sanitization; lost source bytes cannot be recovered. */
export function useRawTextGuard<T extends TextField>(
  domain: TextDomain,
  value: string,
  onValueChange: (value: string) => void,
  onRejected: (message: string) => void,
  options: GuardOptions = {},
) {
  const ref = useRef<T>(null);
  const rejectedInput = useRef<string | null>(null);
  const composing = useRef(false);
  const [compositionDraft, setCompositionDraft] = useState<string | null>(null);
  const latest = useRef({ onValueChange, onRejected, options, value });
  latest.current = { onValueChange, onRejected, options, value };

  useEffect(() => {
    const field = ref.current;
    if (!field) return;
    function beforeInput(event: Event) {
      if (!(event instanceof InputEvent)) return;
      rejectedInput.current = null;
      const rawTexts = [event.data, event.dataTransfer?.getData("text/plain")];
      const lineBreak = domain !== "seeds" && domain !== "evidence" &&
        (event.inputType === "insertLineBreak" || event.inputType === "insertParagraph");
      if (!lineBreak && !rawTexts.some((raw) => typeof raw === "string" && forbiddenSource(domain, raw))) return;
      if (event.cancelable) event.preventDefault();
      else rejectedInput.current = messages[domain];
      latest.current.onRejected(messages[domain]);
    }
    // React's onBeforeInput is polyfilled; inspect the native InputEvent directly.
    field.addEventListener("beforeinput", beforeInput);
    return () => field.removeEventListener("beforeinput", beforeInput);
  }, [domain]);
  useEffect(() => () => {
    if (composing.current) latest.current.options.onCompositionChange?.(false);
    composing.current = false;
    rejectedInput.current = null;
  }, []);

  function screenTransfer(event: ClipboardEvent<T> | DragEvent<T>, raw: string) {
    rejectedInput.current = null;
    if (!forbiddenSource(domain, raw)) return;
    event.preventDefault();
    onRejected(messages[domain]);
  }
  function accept(field: T) {
    const next = field.value;
    const invalid = forbiddenSource(domain, next);
    // Check UTF-16 scalars before TextEncoder can replace a lone surrogate.
    const bytes = invalid ? null : domain === "evidence" ? unicodeEvidenceBytes(next) : new TextEncoder().encode(next).byteLength;
    const rejection = rejectedInput.current ?? (invalid || bytes === null ? messages[domain] :
      options.maxBytes !== undefined && bytes > options.maxBytes ? oversized : null);
    rejectedInput.current = null;
    if (rejection) {
      // Noncancelable events may already mutate the DOM. Restore the last
      // accepted controlled value without claiming to prevent that transient edit.
      field.value = value;
      onRejected(rejection);
    } else onValueChange(next);
  }
  return {
    ref,
    value: compositionDraft ?? value,
    onPaste: (event: ClipboardEvent<T>) => screenTransfer(event, event.clipboardData.getData("text/plain")),
    onDrop: (event: DragEvent<T>) => screenTransfer(event, event.dataTransfer.getData("text/plain")),
    onCompositionStart: () => {
      composing.current = true;
      setCompositionDraft(value);
      options.onCompositionChange?.(true);
    },
    onCompositionEnd: (event: CompositionEvent<T>) => {
      // A reset or removed input cannot finish a composition on a fresh guard.
      if (!composing.current) {
        event.currentTarget.value = value;
        return;
      }
      composing.current = false;
      setCompositionDraft(null);
      accept(event.currentTarget);
      options.onCompositionChange?.(false);
    },
    onChange: (event: ChangeEvent<T>) => {
      if (composing.current && rejectedInput.current) {
        const rejection = rejectedInput.current;
        rejectedInput.current = null;
        event.currentTarget.value = value;
        setCompositionDraft(value);
        onRejected(rejection);
        return;
      }
      if (composing.current) {
        setCompositionDraft(event.currentTarget.value);
        return;
      }
      accept(event.currentTarget);
    },
  };
}
