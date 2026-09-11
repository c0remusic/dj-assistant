# M2a test fixtures (generated)

Run `node scripts/make-fixtures.mjs` to regenerate. These are fabricated via ffmpeg and are
NOT committed. For "authentic" anchors that can't be fabricated, drop real files here:

- `anchor_real_lossless.flac` (a genuine lossless rip)
- `anchor_real_320.mp3` (a genuine store-bought 320)
- `anchor_lame320.flac` (a real track transcoded by LAME 320 and re-wrapped as FLAC — e.g. a copy
  of `C:\sift-corpusake\src01_lame320.flac` — the MP3 bank of the quantization probe, #63,
  needs real music: the swept-sine fixtures are tonal and degenerate by design)

The characterization test skips anchors that are absent.
