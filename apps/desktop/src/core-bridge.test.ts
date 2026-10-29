// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { CORE_PROBE_COMMAND, runCoreProbe } from "./core-bridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  isTauri: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
});

describe("Core IPC boundary", () => {
  it("invokes the no-input command and returns the two validated booleans", async () => {
    const result = { acceptedCaseEligible: true, rejectedCaseEligible: false };
    vi.mocked(invoke).mockResolvedValue(result);
    await expect(runCoreProbe()).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(CORE_PROBE_COMMAND);
  });

  it("rejects browser-only execution without invoking or faking a Core result", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(runCoreProbe()).rejects.toMatchObject({ code: "DESKTOP_UNAVAILABLE" });
    expect(invoke).not.toHaveBeenCalled();
  });

  it.each([
    null,
    { acceptedCaseEligible: true },
    { acceptedCaseEligible: "true", rejectedCaseEligible: false },
    { acceptedCaseEligible: true, rejectedCaseEligible: false, unexpected: "private data" },
  ])("rejects an invalid response shape: %j", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(runCoreProbe()).rejects.toMatchObject({ code: "INVALID_RESPONSE" });
  });

  it.each(["CORE_VALIDATION_FAILED", "CORE_EVALUATION_FAILED"])(
    "maps the fixed adapter error %s",
    async (code) => {
      vi.mocked(invoke).mockRejectedValue(code);
      await expect(runCoreProbe()).rejects.toMatchObject({ code });
    },
  );

  it("replaces arbitrary invocation errors with a fixed safe message", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("private domain contents"));
    await expect(runCoreProbe()).rejects.toMatchObject({
      code: "INVOCATION_FAILED",
      message: "The Core connection check failed. You can try again.",
    });
  });
});
