// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { projectOpen, projectSave, safeProjectError, validPassphrase, ProjectError } from "./project-bridge";
import { authoringDocument, defaultAuthoringInput } from "./project-document";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => { vi.clearAllMocks(); vi.mocked(isTauri).mockReturnValue(true); });
describe("Project Raw IPC bridge", () => {
  it("sends tagged actions and opaque tokens with bounded Raw JSON and no filesystem path", async () => {
    vi.mocked(invoke).mockResolvedValue({ status: "saved" });
    const document = authoringDocument(defaultAuthoringInput(), new Map()); document.manualSeeds.text = "\nsynthetic\t試験\n";
    await projectSave({ action: "write", token: "opaque_token", passphrase: " phrase ", confirmation: " phrase ", document });
    const [command, raw] = vi.mocked(invoke).mock.calls[0];
    expect(command).toBe("project_save"); expect(ArrayBuffer.isView(raw)).toBe(true);
    const request = JSON.parse(new TextDecoder().decode(raw as Uint8Array));
    expect(request).toEqual({ action: "write", token: "opaque_token", passphrase: " phrase ", confirmation: " phrase ", document });
    expect(Object.hasOwn(request, "path")).toBe(false);
  });
  it.each([new Error("private phrase /private/path"), { message: "secret" }, "secret parser error"])("sanitizes raw transport failures", async (failure) => {
    vi.mocked(invoke).mockRejectedValue(failure);
    await expect(projectOpen({ action: "select" })).rejects.toMatchObject({ code: "INVOCATION_FAILED" });
    expect(safeProjectError(failure)).not.toContain("secret");
    expect(safeProjectError(failure)).not.toContain("private");
  });
  it("maps fixed errors and rejects unavailable native transport", async () => {
    vi.mocked(invoke).mockRejectedValue("PROJECT_AUTHENTICATION_FAILED");
    await expect(projectOpen({ action: "select" })).rejects.toMatchObject({ code: "PROJECT_AUTHENTICATION_FAILED" });
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(projectOpen({ action: "select" })).rejects.toMatchObject({ code: "DESKTOP_UNAVAILABLE" });
  });
  it("classifies lost or malformed write replies as unknown, while preserving fixed precommit errors", async () => {
    const action = { action: "write", token: "token", passphrase: "phrase", confirmation: null, document: authoringDocument(defaultAuthoringInput(), new Map()) } as const;
    for (const failure of [new Error("secret transport failure"), "unknown failure", "PROJECT_OUTCOME_UNKNOWN", "PROJECT_WORKER_FAILED"]) {
      vi.mocked(invoke).mockRejectedValueOnce(failure);
      await expect(projectSave(action)).resolves.toEqual({ status: "outcomeUnknown" });
    }
    vi.mocked(invoke).mockResolvedValueOnce({ status: "unexpected" });
    await expect(projectSave(action)).resolves.toEqual({ status: "outcomeUnknown" });
    vi.mocked(invoke).mockRejectedValueOnce("PROJECT_AUTHENTICATION_FAILED");
    await expect(projectSave(action)).rejects.toMatchObject({ code: "PROJECT_AUTHENTICATION_FAILED" });
  });
  it.each([
    { status: "selected", token: "/some/path" },
    { status: "opened", token: "token", document: {} },
    { status: "saved", path: "/secret" },
    { status: "notCommitted", code: "parser secret" },
    { status: "surprise" },
  ])("rejects invalid native responses without exposing their content (%#)", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(projectOpen({ action: "select" })).rejects.toMatchObject({ code: "INVALID_RESPONSE" });
  });
  it("checks exact UTF-8 phrase boundaries without trimming or Unicode normalization", () => {
    expect(validPassphrase(" ")).toBe(true); expect(validPassphrase("é".repeat(512))).toBe(true);
    expect(validPassphrase("é".repeat(513))).toBe(false); expect(validPassphrase("")).toBe(false);
    for (const value of ["\r", "\n", "a\r\nb", "\ud800"]) expect(validPassphrase(value)).toBe(false);
    expect(safeProjectError(new ProjectError("PROJECT_BUDGET_EXCEEDED"))).toContain("resource limits");
  });
});
