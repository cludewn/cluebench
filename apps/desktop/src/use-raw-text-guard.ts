// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useEffect, useRef } from "react";
import type { ChangeEvent, ClipboardEvent, DragEvent } from "react";

type TextField = HTMLInputElement | HTMLTextAreaElement;
type TextDomain = "seeds" | "affix";

const messages = {
  seeds: "Input rejected. Use LF or CRLF between seed lines; a lone CR is not allowed.",
  affix: "Input rejected. Prefix and suffix options cannot contain CR or LF.",
} as const;

function forbiddenLineEnding(domain: TextDomain, text: string): boolean {
  return domain === "seeds" ? /\r(?!\n)/.test(text) : /[\r\n]/.test(text);
}

/** Inspect source text before textarea/input value sanitization loses it. */
export function useRawTextGuard<T extends TextField>(
  domain: TextDomain,
  value: string,
  onValueChange: (value: string) => void,
  onRejected: (message: string) => void,
) {
  const ref = useRef<T>(null);
  const rejectedInput = useRef<string | null>(null);

  useEffect(() => {
    const field = ref.current;
    if (!field) return;
    function beforeInput(event: Event) {
      if (!(event instanceof InputEvent)) return;
      rejectedInput.current = null;
      const rawTexts = [event.data, event.dataTransfer?.getData("text/plain")];
      const lineBreak = domain === "affix" &&
        (event.inputType === "insertLineBreak" || event.inputType === "insertParagraph");
      if (!lineBreak && !rawTexts.some((raw) => typeof raw === "string" && forbiddenLineEnding(domain, raw))) return;
      if (event.cancelable) event.preventDefault();
      else rejectedInput.current = messages[domain];
      onRejected(messages[domain]);
    }
    // React's onBeforeInput is polyfilled; use the actual native InputEvent.
    field.addEventListener("beforeinput", beforeInput);
    return () => field.removeEventListener("beforeinput", beforeInput);
  }, [domain, onRejected]);

  function screenTransfer(event: ClipboardEvent<T> | DragEvent<T>, raw: string) {
    rejectedInput.current = null;
    if (!forbiddenLineEnding(domain, raw)) return;
    event.preventDefault();
    onRejected(messages[domain]);
  }

  return {
    ref,
    onPaste: (event: ClipboardEvent<T>) => screenTransfer(event, event.clipboardData.getData("text/plain")),
    onDrop: (event: DragEvent<T>) => screenTransfer(event, event.dataTransfer.getData("text/plain")),
    onChange: (event: ChangeEvent<T>) => {
      const rejection = rejectedInput.current;
      rejectedInput.current = null;
      if (rejection) {
        // A noncancelable beforeinput may already have changed the DOM. Refuse
        // that edit and restore the controlled value, without claiming that
        // transient DOM mutation was prevented. Never reconstruct lost text.
        event.currentTarget.value = value;
        onRejected(rejection);
      } else onValueChange(event.currentTarget.value);
    },
  };
}
