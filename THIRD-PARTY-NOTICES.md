# Third-party notices

Blurt itself is proprietary (see LICENSE). It builds on the work below, which
remains under the licences named here. Nothing in Blurt's own licence changes
the terms of these components.

## Models

These are **not** bundled with Blurt. The app downloads them from their
publishers on first use, and they stay on the user's own machine.

| Component | Author | Licence |
|---|---|---|
| Parakeet TDT 0.6B v3 (speech to text) | NVIDIA | CC BY 4.0 |
| emotion2vec+ base (tone) | Ma et al. / FunASR | FunASR model licence |
| Smart Turn v3.2 (end of turn) | Pipecat | BSD-2-Clause |

## Libraries

| Component | Licence |
|---|---|
| Tauri, and its plugins | MIT OR Apache-2.0 |
| ONNX Runtime, and the `ort` crate | MIT |
| Handy — hotkey handling and model plumbing this app learned from | MIT |
| `transcribe-rs`, `cpal`, `rubato`, `realfft`, `arboard`, `enigo`, `keyring` | MIT OR Apache-2.0 |
| React | MIT |

The full dependency tree and its licences can be produced from the lockfiles
with `cargo license` and `pnpm licenses list`.
