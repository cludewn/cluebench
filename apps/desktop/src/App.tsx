// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useState } from "react";
import { ProbeError, runCoreProbe } from "./core-bridge";
import type { CoreProbeResult } from "./core-bridge";

type ProbeState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; result: CoreProbeResult }
  | { status: "error"; message: string };

export default function App() {
  const [state, setState] = useState<ProbeState>({ status: "idle" });

  async function checkConnection() {
    setState({ status: "loading" });
    try {
      const result = await runCoreProbe();
      setState({ status: "success", result });
    } catch (error: unknown) {
      setState({
        status: "error",
        message:
          error instanceof ProbeError
            ? error.message
            : "The Core connection check failed. You can try again.",
      });
    }
  }

  return (
    <main>
      <header>
        <p className="eyebrow">Desktop foundation</p>
        <h1>Cluebench</h1>
        <p className="tagline">A visual workbench for password clues and candidate exploration.</p>
      </header>

      <section aria-labelledby="connection-title" className="connection">
        <h2 id="connection-title">Rust Core connection</h2>
        <p>Run a read-only check using fixed synthetic data.</p>
        <button type="button" onClick={checkConnection} disabled={state.status === "loading"}>
          {state.status === "loading" ? "Checking…" : "Check Core connection"}
        </button>
        <div role="status" aria-live="polite" aria-busy={state.status === "loading"}>
          {state.status === "idle" && <p>Not checked yet.</p>}
          {state.status === "loading" && <p>Checking the Rust Core connection…</p>}
          {state.status === "success" && (
            <>
              <p className="success">Core check completed.</p>
              <dl>
                <dt>Accepted case eligible</dt>
                <dd>{String(state.result.acceptedCaseEligible)}</dd>
                <dt>Rejected case eligible</dt>
                <dd>{String(state.result.rejectedCaseEligible)}</dd>
              </dl>
            </>
          )}
          {state.status === "error" && <p className="error">{state.message}</p>}
        </div>
      </section>

      <footer>
        This initial shell verifies the desktop connection. Clue editing, candidate preview, and
        wordlist export are not available yet.
      </footer>
    </main>
  );
}
