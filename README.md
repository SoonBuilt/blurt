# Blurt

Hold a key and talk. Clean text appears wherever your cursor is. Add ⇧ and your voice
becomes a prompt for AI, with anything you've highlighted as context. By soonbuilt, with Tally.

| | macOS | Windows |
|---|---|---|
| Talk key | hold Right ⌥ (add ⇧ for AI) | hold Right Ctrl (add Shift for AI) |
| Speech to text | Parakeet TDT 0.6B v3, on-device | same |
| Ask AI | Apple Intelligence (on-device), Ollama, Claude or OpenAI-compatible | Ollama, Claude or OpenAI-compatible |
| Listening bar | non-activating NSPanel, floats over full-screen apps | always-on-top, non-focusable window |
| Tone awareness | emotion2vec+ base | same |
| Hands-free | double-tap the talk key; Smart Turn v3.2 ends the turn | same |

Everything runs on the user's computer. Optional models are downloadable packs
(`src-tauri/src/models.rs`). Ask AI keeps a short-term memory of the last few exchanges
(10 minutes, in RAM only).

## Develop

```bash
pnpm install
pnpm tauri dev
```

Needs Rust, Node 24 and pnpm. On macOS, full Xcode (for the Swift bridge in
`src-tauri/swift/`). The voice model (~670 MB) downloads on first run into the app data folder.

- Speech pipeline without the UI: `cd src-tauri && cargo run --example transcribe -- <data-dir> <file.wav>`
- UI design review in a browser: `pnpm dev` and open http://localhost:1420 (sample data, `?settings` for Settings)
- Tests: `cd src-tauri && cargo test --lib`

## Layout

- `src-tauri/src/keys.rs`: hold-to-talk state machine (one key, two modes)
- `audio.rs`: mic capture, resampled to 16 kHz · `stt.rs`: model download + Parakeet
- `engine.rs`: key → record → transcribe → clean up or ask AI → paste
- `insert.rs`: paste via clipboard, read the highlighted text (macOS Accessibility first)
- `ai.rs` + `apple.rs` + `swift/BlurtApple.swift`: AI providers and the Apple Intelligence bridge
- `hud.rs` + `src/hud/`: the listening bar · `tray.rs`: Tally in the menu bar/tray
- `src/app/`: first-run setup and Settings · `src/tally/Tally.tsx`: Tally, ported from TallyReel

## Builds

`.github/workflows/blurt.yml` (repo root) builds macOS (Apple Silicon + Intel) and Windows installers on
every push, and drafts a GitHub release for `blurt-v*` tags. Builds are unsigned for now.

## Credits

Speech: NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0) via ONNX Runtime and transcribe-rs.
required). Turn detection: Pipecat Smart Turn v3.2 (BSD-2).
Hotkeys (handy-keys) and several platform approaches come from Handy (MIT).

## Website (blurt.soonbuilt.com)

`web/public/` is the static site, served by the `blurt-site` Cloudflare Worker (assets only)
on the `blurt.soonbuilt.com` custom domain. Deploy from the repo root with
`pnpm deploy:blurt-site`. When installers are published, fill in `RELEASE` at the top of
`web/public/main.js` and the download buttons go live.
