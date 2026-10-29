// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import App from "./App";
import { ProbeError, runCoreProbe } from "./core-bridge";
import type { CoreProbeResult } from "./core-bridge";

vi.mock("./core-bridge", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./core-bridge")>()),
  runCoreProbe: vi.fn(),
}));

afterEach(cleanup);

const result = { acceptedCaseEligible: true, rejectedCaseEligible: false };

describe("Desktop shell", () => {
  it("starts idle and identifies features that are still unavailable", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Cluebench" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Not checked yet.");
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(false);
    expect(screen.getByText(/wordlist export are not available yet/)).toBeTruthy();
    expect(runCoreProbe).not.toHaveBeenCalled();
  });

  it("disables the action during Loading then displays the actual response", async () => {
    let finish: (value: CoreProbeResult) => void = () => {};
    vi.mocked(runCoreProbe).mockReturnValue(new Promise((resolve) => (finish = resolve)));
    render(<App />);
    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("Checking the Rust Core connection…")).toBeTruthy();
    await act(async () => finish(result));
    expect(screen.getByText("Core check completed.")).toBeTruthy();
    expect(screen.getByText("true")).toBeTruthy();
    expect(screen.getByText("false")).toBeTruthy();
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("false");
  });

  it("clears Loading after rejection and allows a successful retry", async () => {
    vi.mocked(runCoreProbe)
      .mockRejectedValueOnce(new ProbeError("INVOCATION_FAILED"))
      .mockResolvedValueOnce(result);
    render(<App />);
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("The Core connection check failed. You can try again.")).toBeTruthy();
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("false");
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("Core check completed.")).toBeTruthy();
    expect(runCoreProbe).toHaveBeenCalledTimes(2);
    expect(screen.queryByText("The Core connection check failed. You can try again.")).toBeNull();
  });

  it("shows a safe disconnected state for a browser-only session", async () => {
    vi.mocked(runCoreProbe).mockRejectedValue(new ProbeError("DESKTOP_UNAVAILABLE"));
    render(<App />);
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText(/Desktop connection unavailable/)).toBeTruthy();
    expect(screen.getByRole("button").hasAttribute("disabled")).toBe(false);
    expect(screen.queryByText("Core check completed.")).toBeNull();
  });
});
