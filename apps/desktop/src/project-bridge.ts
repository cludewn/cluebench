// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { isProjectDocument } from "./project-document";
import type { ProjectDocumentV1 } from "./project-document";
import { hasLoneSurrogate } from "./unicode-text";

const messages = {
  DESKTOP_UNAVAILABLE: "Desktop connection unavailable. Open the native Cluebench application to use project files.",
  PROJECT_BUSY: "A project operation is still running. Wait for it to finish.",
  PROJECT_INVALID_DOCUMENT: "The project document is invalid or exceeds its resource limits.",
  PROJECT_INVALID_BODY: "The desktop rejected the project operation.",
  PROJECT_INVALID_PASSPHRASE: "Enter a nonempty passphrase of at most 1,024 UTF-8 bytes, without CR or LF.",
  PROJECT_AUTHENTICATION_FAILED: "The passphrase could not authenticate this file, or the file is damaged.",
  PROJECT_UNSUPPORTED_FILESYSTEM: "This location does not support the required safe project file operations.",
  PROJECT_EXTERNAL_CHANGE: "The associated file changed outside Cluebench. Open it again before saving.",
  PROJECT_DESTINATION_EXISTS: "Choose a new destination. Existing files cannot be overwritten by Save As.",
  PROJECT_INVALID_TOKEN: "The project file association is no longer available. Open the file again.",
  PROJECT_IO: "The project file operation failed. The current editing project was retained.",
  PROJECT_UNSUPPORTED_VERSION: "This project schema version is unsupported. Only version 1 is supported.",
  PROJECT_TOO_LARGE: "The project file exceeds the resource limits.",
  PROJECT_BUDGET_EXCEEDED: "The project file exceeds the resource limits.",
  PROJECT_PASSPHRASE_MISMATCH: "The passphrases do not match.",
  PROJECT_CRYPTO_FAILED: "The encrypted project operation failed.",
  PROJECT_INVALID_REQUEST: "The desktop rejected the project operation.",
  PROJECT_UNAVAILABLE: "Project file operations are unavailable in this environment.",
  PROJECT_UNSAFE_DESTINATION: "This location does not support the required safe project file operations.",
  PROJECT_OUTCOME_UNKNOWN: "The file operation outcome is unknown. Open the file before attempting another save.",
  PROJECT_WORKER_FAILED: "The project worker failed. The operation did not report success.",
  INVALID_RESPONSE: "The desktop returned an unexpected project response. No editing project was replaced.",
  INVOCATION_FAILED: "The project operation failed. No success was reported.",
} as const;
export type ProjectErrorCode = keyof typeof messages;
export class ProjectError extends Error {
  constructor(readonly code: ProjectErrorCode) { super(messages[code]); this.name = "ProjectError"; }
}
export function projectErrorFromCode(code: unknown): ProjectError {
  return new ProjectError(typeof code === "string" && Object.hasOwn(messages, code) ? code as ProjectErrorCode : "INVOCATION_FAILED");
}
export function safeProjectError(error: unknown): string {
  return new ProjectError(error instanceof ProjectError && Object.hasOwn(messages, error.code) ? error.code : "INVOCATION_FAILED").message;
}
export function validPassphrase(value: string): boolean {
  return value.length > 0 && !hasLoneSurrogate(value) && !/[\r\n]/.test(value) && new TextEncoder().encode(value).byteLength <= 1024;
}
export type OpenAction = { action: "select" } | { action: "read"; token: string; passphrase: string } | { action: "adopt" | "finalize" | "discard"; token: string };
export type SaveAction = { action: "select" } | { action: "write"; token: string; passphrase: string; confirmation: string | null; document: ProjectDocumentV1 };
export type ProjectResponse =
  | { status: "selected"; token: string }
  | { status: "opened"; token: string; document: ProjectDocumentV1 }
  | { status: "cancelled" | "adopted" | "finalized" | "discarded" | "saved" | "committedDurabilityUncertain" | "outcomeUnknown" }
  | { status: "notCommitted"; code: string };
const token = (value: unknown): value is string => typeof value === "string" && /^[A-Za-z0-9_-]{1,128}$/.test(value);
function validResponse(value: unknown): value is ProjectResponse {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const row = value as Record<string, unknown>;
  if (row.status === "selected") return Object.keys(row).length === 2 && token(row.token);
  if (row.status === "opened") return Object.keys(row).length === 3 && token(row.token) && isProjectDocument(row.document);
  if (row.status === "notCommitted") return Object.keys(row).length === 2 && typeof row.code === "string" && /^[A-Z_]{1,64}$/.test(row.code);
  return Object.keys(row).length === 1 && ["cancelled", "adopted", "finalized", "discarded", "saved", "committedDurabilityUncertain", "outcomeUnknown"].includes(row.status as string);
}
async function operation(command: "project_open" | "project_save", action: OpenAction | SaveAction): Promise<ProjectResponse> {
  if (!isTauri()) throw new ProjectError("DESKTOP_UNAVAILABLE");
  const payload = new TextEncoder().encode(JSON.stringify(action));
  if (payload.byteLength > 272 * 1024) throw new ProjectError("PROJECT_TOO_LARGE");
  let response: unknown;
  try { response = await invoke<unknown>(command, payload); }
  catch (error: unknown) {
    // Losing the write reply does not establish that the rename never happened.
    if (command === "project_save" && action.action === "write" &&
      (typeof error !== "string" || !error.startsWith("PROJECT_") || !Object.hasOwn(messages, error) || error === "PROJECT_OUTCOME_UNKNOWN" || error === "PROJECT_WORKER_FAILED")) return { status: "outcomeUnknown" };
    throw projectErrorFromCode(error);
  }
  if (command === "project_save" && action.action === "write" &&
    (!validResponse(response) || !["notCommitted", "saved", "committedDurabilityUncertain", "outcomeUnknown"].includes(response.status))) return { status: "outcomeUnknown" };
  if (!validResponse(response)) throw new ProjectError("INVALID_RESPONSE");
  return response;
}
export const projectOpen = (action: OpenAction): Promise<ProjectResponse> => operation("project_open", action);
export const projectSave = (action: SaveAction): Promise<ProjectResponse> => operation("project_save", action);
