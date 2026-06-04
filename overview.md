# freeblue

Reverse-engineer MakeMKV and a known-working UHD Blu-ray rip to create a FLOSS
specification of **AACS 2.0** decryption — then a clean-room implementation that
matches MakeMKV byte-for-byte. Use public documentation, Ghidra, and black-box
behavioral analysis to get there.

This is the sister project to [`rippidydoodah`](../rippidydoodah/), which remuxes
an *already-decrypted* stream and explicitly defers UHD/AACS 2.0 as a non-goal.
`freeblue` is the work that closes that gap.

## Start here

→ **[specs/README.md](specs/README.md)** — the specification series (00–12) and
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

Specs 00–12 complete; the **decrypt core is verified on real discs through live
SCSI captures** — v1 (GoT) **byte-identical to MakeMKV** (spec 11 §11.4.5) and the
**first real UHD/AACS 2.0 content** (TURBO) decrypted to valid TS, video PID
`0x1011` (spec 11 §11.4.6). The Rust workspace is built out — `freeblue-crypto`,
`-mkb`, `-content`, `-keys`, `-disc`, `-read`, `-core` are implemented (36 passing
tests); only `-cli` is a stub. **freeblue now reads protected discs on its own — no
MakeMKV:** the LibreDrive "unlock" is a static, read-only `READ BUFFER` (`0x3C`)
sequence that `freeblue-read` replays over `SG_IO`, flipping the drive to raw mode;
then `content_decrypt` (LibreDrive returns no bus layer, spec 11 §11.4.6–7).
Verified on the **cold** TURBO UHD disc: 8/8 Aligned Units decrypt 32/32, MakeMKV
nowhere in the loop. Remaining glue is **clip extent resolution** (UDF/BDMV → LBA)
and a CLI. Open items live in [spec 12](specs/12-known-issues-and-deferred-work.md). See the
[confidence model](specs/00-overview.md) (spec 00 §0.6) and the
[roadmap](specs/README.md).
