// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { invoke, isTauri } from "@tauri-apps/api/core";

export const CORE_PROBE_COMMAND = "core_probe";

// The command takes no request payload and returns only these two fields.
export interface CoreProbeResult {
  acceptedCaseEligible: boolean;
  rejectedCaseEligible: boolean;
}

export type ProbeErrorCode =
  | "DESKTOP_UNAVAILABLE"
  | "CORE_VALIDATION_FAILED"
  | "CORE_EVALUATION_FAILED"
  | "INVALID_RESPONSE"
  | "INVOCATION_FAILED";

const messages: Record<ProbeErrorCode, string> = {
  DESKTOP_UNAVAILABLE:
    "Desktop connection unavailable. Open the native Cluebench application to run this check.",
  CORE_VALIDATION_FAILED: "The Core connection check could not validate its synthetic project.",
  CORE_EVALUATION_FAILED: "The Core connection check could not evaluate its synthetic cases.",
  INVALID_RESPONSE: "The desktop returned an unexpected connection-check response.",
  INVOCATION_FAILED: "The Core connection check failed. You can try again.",
};

export class ProbeError extends Error {
  constructor(readonly code: ProbeErrorCode) {
    super(messages[code]);
    this.name = "ProbeError";
  }
}

function isProbeResult(value: unknown): value is CoreProbeResult {
  return (
    typeof value === "object" &&
    value !== null &&
    Object.keys(value).length === 2 &&
    "acceptedCaseEligible" in value &&
    typeof value.acceptedCaseEligible === "boolean" &&
    "rejectedCaseEligible" in value &&
    typeof value.rejectedCaseEligible === "boolean"
  );
}

export async function runCoreProbe(): Promise<CoreProbeResult> {
  if (!isTauri()) {
    throw new ProbeError("DESKTOP_UNAVAILABLE");
  }

  let response: unknown;
  try {
    response = await invoke<unknown>(CORE_PROBE_COMMAND);
  } catch (error: unknown) {
    // Show only recognized codes; never echo an arbitrary IPC rejection.
    if (error === "CORE_VALIDATION_FAILED" || error === "CORE_EVALUATION_FAILED") {
      throw new ProbeError(error);
    }
    throw new ProbeError("INVOCATION_FAILED");
  }

  if (!isProbeResult(response)) {
    throw new ProbeError("INVALID_RESPONSE");
  }
  return response;
}
