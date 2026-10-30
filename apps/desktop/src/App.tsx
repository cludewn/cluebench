// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import CandidatePreview from "./CandidatePreview";
import CoreProbe from "./CoreProbe";

export default function App() {
  return <main>
    <header><p className="eyebrow">Manual seed exploration</p><h1>Cluebench</h1><p className="tagline">A visual workbench for password clues and candidate exploration.</p></header>
    <CandidatePreview />
    <CoreProbe />
    <footer>Clue, hypothesis, and constraint editors, project persistence, structure canvas, and wordlist/clipboard export are not available yet.</footer>
  </main>;
}
