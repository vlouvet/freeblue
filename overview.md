# freeblue

Reverse-engineer MakeMKV and a known-working UHD Blu-ray rip to create a FLOSS
specification of **AACS 2.0** decryption — then a clean-room implementation that
matches MakeMKV byte-for-byte. Use public documentation, Ghidra, and black-box
behavioral analysis to get there.

This is the sister project to [`rippidydoodah`](../rippidydoodah/), which remuxes
an *already-decrypted* stream and explicitly defers UHD/AACS 2.0 as a non-goal.
`freeblue` is the work that closes that gap.

## Start here

→ **[specs/README.md](specs/README.md)** — the specification series (00–10) and
reading order. The specs are the deliverable; the implementation is downstream.

Core technical path: [02 key hierarchy](specs/02-key-hierarchy-and-crypto-primitives.md)
→ [03 MKB & revocation](specs/03-media-key-block-and-revocation.md)
→ [04 on-disc structures](specs/04-on-disc-structures.md)
→ [05 content decryption](specs/05-content-decryption-pipeline.md).
How we pin the unknowns: [07 MakeMKV RE](specs/07-makemkv-reverse-engineering.md)
+ [09 validation](specs/09-validation-and-test-vectors.md).

## The one-line thesis

AACS 2.0 is **AACSv1 with SHA-1→SHA-256 and a custom curve→P-256**, plus the
player's keys hidden in an Intel SGX enclave (37c3 talk, [49:43]). The SGX
wrapper — not the cipher — was the real barrier, and it was already broken and
the **253-key device set published** by the SGX.fail team. So `freeblue` never
touches SGX: it consumes the public keys and specifies the v1-shaped math that
turns an encrypted UHD disc into plaintext M2TS.

## Known resources

In [`res/`](res/) (cited throughout the specs):

- **`talk-transcript.txt`** — 37c3 (2023) talk *"AACSess: Exposing and
  exploiting AACSv2 UHD DRM for your viewing pleasure"* (SGX.fail team). The
  authoritative public account. Cited as `[Talk <timestamp>]`.
- **`37c3-12296-…_sd.mp4`** / **`…_opus.opus`** — the talk's video and audio.
- **`AACS 2.0 Architecture elements draft 02_23_2014.pptx.pdf`** — circulated
  AACS LA architecture deck (2014 draft). Cited as `[Arch]`. Extracted text:
  `res/arch-deck-extracted.txt`.

To obtain and cite (not recall) — see [spec 02 §2.1](specs/02-key-hierarchy-and-crypto-primitives.md):

- AACS LA **"Common Cryptographic Elements"** book — the v1 crypto v2 reuses.
- **`libaacs`** source (VideoLAN) — the FLOSS v1 clean-room oracle.
- The published **SGX.fail** paper (`sgx.fail`).

## Status

Specification drafted (specs 00–10); no code yet. Most technical claims are
tagged `[?]` (RE-pending) and become `[E]` only when a MakeMKV byte-match earns
it — see the [confidence model](specs/00-overview.md) (spec 00 §0.6).
