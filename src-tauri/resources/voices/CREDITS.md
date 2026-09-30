# Tally's voice

Tally speaks with Kokoro's **af_heart** voice (Kokoro-82M by hexgrad, Apache-2.0),
slowed to 0.89x for a warm, unhurried pace.

- Free voice: Kokoro-82M int8 (via sherpa-onnx), speaker `af_heart`, length scale 1.12.
- Expressive (Pro) voice: Chatterbox Turbo (Resemble AI, MIT) conditioned on
  `tally-reference.wav`, a clip of the Kokoro af_heart voice, so Tally sounds the same
  on both tiers. `tally.cbt` is that conditioning, precomputed with the q4 speech encoder.
