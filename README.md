# Blurt

**Hold a key and talk.** Clean text appears wherever your cursor is. Add ⇧ and your
voice becomes a prompt for AI, with anything you've highlighted as context.

Free for Mac and Windows — **[blurt.soonbuilt.com](https://blurt.soonbuilt.com)**

By [soonbuilt](https://soonbuilt.com), with Tally.

---

| | macOS | Windows |
|---|---|---|
| Talk key | hold Right ⌥ (add ⇧ for AI) | hold Right Ctrl (add Shift for AI) |
| Speech to text | Parakeet TDT 0.6B v3, on-device | same |
| Ask AI | Apple Intelligence, Ollama, Claude, OpenAI and others | Ollama, Claude, OpenAI and others |
| Tone awareness | emotion2vec+ base | same |
| Hands-free | double-tap the talk key | same |

Your voice is turned into text on your own computer and is never uploaded. Ask AI keeps
a short-term memory of the last few exchanges (10 minutes, in memory only). No account,
nothing to subscribe to.

## Licence

Blurt is **free to use but not open source**. The source is published so you can check
what the app does with your voice — not as permission to reuse it. See [LICENSE](LICENSE),
and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for the components Blurt builds on.

## Credits

Speech recognition: NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0), via ONNX Runtime.
Tone: emotion2vec+ base by Ma et al. Turn detection: Pipecat Smart Turn v3.2 (BSD-2).
Hotkey handling builds on Handy (MIT).
