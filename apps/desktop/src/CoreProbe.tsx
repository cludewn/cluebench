// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useEffect, useRef, useState } from "react";
import { ProbeError, runCoreProbe } from "./core-bridge";
import type { CoreProbeResult } from "./core-bridge";

type ProbeState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; result: CoreProbeResult }
  | { status: "error"; message: string };

export default function CoreProbe() {
  const [state, setState] = useState<ProbeState>({ status: "idle" });

  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  async function checkConnection() {
    setState({ status: "loading" });
    try {
      const result = await runCoreProbe();
      if (mounted.current) setState({ status: "success", result });
    } catch (error: unknown) {
      if (mounted.current) setState({
        status: "error",
        message:
          error instanceof ProbeError
            ? error.message
            : "The Core connection check failed. You can try again.",
      });
    }
  }

  return (
    <details className="connection">
      <summary>Rust Core connection check</summary>
      <p>Run a read-only check using fixed synthetic data.</p>
      <button type="button" onClick={checkConnection} disabled={state.status === "loading"}>
        {state.status === "loading" ? "Checking…" : "Check Core connection"}
      </button>
      <div role="status" aria-label="Core connection status" aria-live="polite" aria-busy={state.status === "loading"}>
        {state.status === "idle" && <p>Not checked yet.</p>}
        {state.status === "loading" && <p>Checking the Rust Core connection…</p>}
        {state.status === "success" && <p>Core check completed. Accepted: {String(state.result.acceptedCaseEligible)}; rejected: {String(state.result.rejectedCaseEligible)}.</p>}
        {state.status === "error" && <p className="error">{state.message}</p>}
      </div>
    </details>
  );
}
