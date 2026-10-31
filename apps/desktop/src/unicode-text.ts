// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** Detect ill-formed UTF-16 before TextEncoder can replace it with U+FFFD. */
export function hasLoneSurrogate(text: string): boolean {
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = text.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return true;
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) return true;
  }
  return false;
}

export const isUnicodeEvidence = (text: string): boolean => !text.includes("\r") && !hasLoneSurrogate(text);

export function unicodeEvidenceBytes(text: string): number | null {
  return isUnicodeEvidence(text) ? new TextEncoder().encode(text).byteLength : null;
}
