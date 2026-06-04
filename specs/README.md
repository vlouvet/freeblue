# freeblue — Specification Series

A FLOSS specification of **AACS 2.0** (the copy-protection scheme on Ultra HD /
4K Blu-ray), reconstructed from public research and from the observed behavior
of a known-good MakeMKV rip, with the goal of a clean-room, GPL-compatible
description complete enough to build an interoperable decryptor that matches
MakeMKV byte-for-byte.

This is the sister project to [`rippidydoodah`](../../rippidydoodah/), which
remuxes an *already-decrypted* stream into Matroska and explicitly defers UHD /
AACS 2.0 as a non-goal ("no working FLOSS decryption exists; deferred",
[rdd spec 00 §0.3](../../rippidydoodah/specs/00-overview.md)). `freeblue` is the
work that would close that gap and let `rdd` drop the "no UHD" caveat.

## ✅ Validation status — the decryption core is proven on real discs

Every cryptographic step has been **byte-verified against real discs** (a v1
Blu-ray, *Game of Thrones: Conquest & Rebellion* MKBv63, and a v2 UHD structure
dump, *The Warning* MKBv82) using the public AACS books, `libaacs` source, and
the community `keydb.cfg`. The whole pipeline reproduces ground truth exactly:

```
processing key ──[MKB 0x04/0x05 + verify 0x81]──► Media Key   ✅ derived == keydb M   (GoT)
Media Key      ──[Kvu = AES-G(Km, IDv)]─────────► VUK         ✅ AES-G(M,I)==V         (GoT + UHD)
VUK            ──[Kcu = AES-128D(Kvu, ·)]───────► Unit Key     ✅ in Unit_Key_RO.inf    (UHD)
Unit Key+seed  ──[AES-128E⊕seed, AES-128-CBC]───► plaintext    ✅ 32/32 TS-sync         (GoT)
```

AACS 2.0's on-disc formats (MKB record types, Unit Key file, certs) are confirmed
from a real UHD disc. The "v2 = v1 crypto" thesis is therefore proven at the byte
level for the key hierarchy; the **only** unproven step is decrypting one real
*v2 content* Aligned Unit, which needs a UHD disc's `STREAM/*.m2ts`. **No
production code exists yet** — these were reference validations; spec 08 / the
integration roadmap (bottom of this file) is the remaining engineering.

## What this is and is not

`freeblue` is, in the first instance, a **specification** — a written, cited
reconstruction of how AACS 2.0 turns an encrypted UHD disc plus a set of device
keys into plaintext M2TS. A reference implementation is a downstream goal
(spec 08), but the specification is the deliverable that has to be right first.

We do **no** new key extraction. The hard secret — the AACS 2.0 device-key set —
was already extracted and published by the academic *sgx.fail* / "AACSess" work
(see `res/`). `freeblue` takes those public keys as an input and documents the
**decryption math that consumes them**, the part that is "just" AES, ECC, and a
subset-difference tree once the keys are in hand. See spec 10 for legal posture.

## Primary sources

Everything in this series is grounded in, and must stay consistent with, the
material in [`../res/`](../res/):

- **`talk-transcript.txt`** — full transcript of the 37c3 (2023) talk
  *"AACSess: Exposing and exploiting AACSv2 UHD DRM for your viewing pleasure"*
  by the SGX.fail team. The authoritative public account of how AACS 2.0 was
  broken and what it actually contains. Cited throughout as **[Talk]** with a
  timestamp, e.g. **[Talk 49:50]**.
- **`AACS 2.0 Architecture elements draft 02_23_2014.pptx.pdf`** — circulated
  AACS LA architecture slide deck (a 2014 *draft*, watermarked "proposal" /
  "TO REVISE"). Cited as **[Arch]**. Text extracted to
  **`res/arch-deck-extracted.txt`** (via `pdftotext -layout`) for citation;
  high-value slides: the AACS 2.0 crypto-algorithm list, the key-derivation
  diagram, and the Host Certificate field layout (specs 01–04).
**Now obtained** (downloaded into `res/`, the v1 `[E]` ground truth — these
turned most of specs 02–05's `[?]` tags into cited `[E]`):

- **`AACS_Spec_Common_0.91.pdf`** — AACS "Introduction and Common Cryptographic
  Elements" book (extracted: `res/common.txt`). Defines AES-G, AES-G3 (incl. the
  seed constant `0x7B10…3BD9`), the key hierarchy, and the eight MKB records.
  Cited as **[CCE §x]**.
- **`AACS_Spec_BD_Prerecorded_Final_0_953.pdf`** — AACS Blu-ray Disc
  Pre-recorded Book (extracted: `res/bd.txt`). `Kvu = AES-G(Km, IDv)`, the CPS
  Unit Key wrap `AES-128E(Kvu, Kcu)`, and Aligned-Unit CBC content encryption.
  Cited as **[BD §x]**.
- **`AACS_Spec_Prerecorded_Final_0.953.pdf`** — AACS Pre-recorded Video Book
  (title-key / transaction layer).
- **`libaacs_crypto.c`** — VideoLAN `libaacs` crypto source (host has runtime
  `libaacs.so.0` v0.11.1 only). The working clean-room oracle; confirms AES-G3
  byte-for-byte. Cited as **[libaacs]**.
- **`sgxfail24.pdf`** — the peer-reviewed *SGX.Fail* SoK paper (Purdue/Michigan):
  the first public written description of AACSv2 internals, key derivation, and
  v2 revocation/traitor-tracing — the authoritative companion to the talk.

> All five are **v1 / public-research** documents. They establish the `[E]`
> baseline; the v2-specific deltas (spec 02 §2.5) still need byte-match against
> a real disc + MakeMKV (spec 07/09).

**Real AACS 2.0 fixtures** (user-provided — these moved several `[?]`/`[E]` tags
to **[Disc]**-verified; **git-ignored**, contain keys/structures per spec 10):

- **`MKB20_v82_THE_WARNING…tgz`** — a complete real **MKBv82 UHD disc structure
  dump** (MakeMKV community keydb submission): `AACS/MKB_RO.inf`,
  `Unit_Key_RO.inf`, content + DH-pairing certs, revocation list, BDMV. Pinned the
  v2 on-disc layout (spec 04 §4.1) and the **observed v2 MKB record table** (spec
  03 §3.3.1). `page-167.md` is its submission post (disc-id, drive, MakeMKV ver).
- **`keydb_eng.zip`** (`keydb.cfg`, 182k entries incl. **20,228 UHD**) — the
  community key DB. Pinned the `KEYDB.cfg` format and field semantics (spec 06
  §6.5) and, with the disc dump above, **byte-verified** `Kvu = AES-G(Km, IDv)`
  and the `AES-128E(Kvu, Kcu)` unit-key wrap on real v2 (spec 06 §6.5.2, spec 09
  §9.10.1).

## Reading order

**Status** legend: 📄 Reference · 📋 Design · 🚧 Draft · ⬜ Outline only ·
✅ **[Disc]-verified** (math reproduced byte-for-byte against a real disc). No
production code exists yet; status tracks how complete/cited each spec is.

| # | File | What it covers | Status |
|---|------|----------------|--------|
| 00 | [00-overview.md](00-overview.md) | Vision, goals, non-goals, glossary, the known/reported/RE confidence model | 📄 Reference |
| 01 | [01-aacs-history-and-threat-model.md](01-aacs-history-and-threat-model.md) | CSS → AACSv1 → AACS 2.0, what v2 changed, the SGX threat model, why it broke | 📄 Reference |
| 02 | [02-key-hierarchy-and-crypto-primitives.md](02-key-hierarchy-and-crypto-primitives.md) | The key hierarchy, AES-G / AES-G3, SHA-256, P-256; device → processing → media → volume-unique → title key | ✅ Verified |
| 03 | [03-media-key-block-and-revocation.md](03-media-key-block-and-revocation.md) | MKB record format, subset-difference tree, **processing-key → media-key (verified)**, revocation | ✅ Verified |
| 04 | [04-on-disc-structures.md](04-on-disc-structures.md) | Real v2 `/AACS/` layout, Volume ID, content certs, Unit Key file, CPS units | ✅ Verified |
| 05 | [05-content-decryption-pipeline.md](05-content-decryption-pipeline.md) | Aligned-unit block-key + AES-CBC content decrypt → plaintext M2TS | ✅ Verified |
| 06 | [06-key-sources-and-sgx-provisioning.md](06-key-sources-and-sgx-provisioning.md) | SGX/EPID key provisioning (background); **KEYDB.cfg format (verified)** | 📄 Reference |
| 07 | [07-makemkv-reverse-engineering.md](07-makemkv-reverse-engineering.md) | RE methodology: Ghidra targets, MakeMKV oracle, byte-match protocol | 📋 Design |
| 08 | [08-reference-implementation.md](08-reference-implementation.md) | Proposed clean-room implementation: module layout, libbluray/libaacs relationship, language | 📋 Design |
| 09 | [09-validation-and-test-vectors.md](09-validation-and-test-vectors.md) | KATs (**6 green on real discs**), the MakeMKV golden-diff oracle, the disc corpus | 📋 Design |
| 10 | [10-legal-and-licensing.md](10-legal-and-licensing.md) | Clean-room discipline, DMCA / interoperability posture, no-keys-in-repo policy, license | 📄 Reference |

## One-paragraph summary

AACS 2.0 is, cryptographically, **AACS v1 with the hashes and curve swapped**
(SHA-1 → SHA-256, a custom 160-bit curve → NIST P-256) and the player's secrets
moved into an Intel SGX enclave — **[Talk 49:43]**. The SGX wrapper, not the
cipher, was the actual barrier, and it was defeated by a Foreshadow-class side
channel that leaked the EPID attestation key, letting the researchers emulate
the whole remote-attestation flow and download a complete **253-key** AACS 2.0
device-key set in Python on a Raspberry Pi (**[Talk 47:18]**). Once you hold
those device keys, decryption is the familiar v1 pipeline: process the Media Key
Block down a subset-difference tree to a **processing key**, derive the **media
key**, combine it with the disc's **Volume ID** to get the **volume unique key**,
unwrap the **title/unit keys**, and AES-decrypt the 6144-byte aligned units of
the M2TS stream. This series specifies each of those steps precisely enough to
implement, marking every claim as established public AACS, reported-in-the-talk,
or still-to-be-pinned-by-reverse-engineering-against-MakeMKV.

## How to contribute to this series

This project inherits rippidydoodah's working agreement (see that repo's
`CLAUDE.md`): **spec-first**, **cite don't recall**, and **when unsure, say so**.
For `freeblue` specifically, the third rule is load-bearing: AACS 2.0's spec is
under NDA, so large parts of this series are *reconstruction*, not transcription.
Every technical claim therefore carries a confidence tag (spec 00 §0.6). Do not
upgrade a claim's confidence without the citation or the byte-match that earns
it.

## Roadmap: making `rdd` rip UHD via `freeblue`

The algorithms are proven; the **library is not written**. Work, in dependency
order (✅ = algorithm already verified, just needs implementing):

**Phase 1 — Implement the `freeblue` library** (Rust, spec 08). Each crate lands
test-first against the vectors already proven on real discs (spec 09 §9.10.1):
1. `freeblue-crypto` ✅ — AES-128 E/D/CBC, `AES-G = AES-128D(k,d)⊕d`, `AES-G3`
   (seed `0x7B10…3BD9`, middle output = Kpc). Tiny; all constants pinned.
2. `freeblue-keys` ✅ — `KEYDB.cfg` parser (disc-id + `D/M/I/V/U`; `DK/PK/HC`
   records), zeroized store. Format known (spec 06 §6.5).
3. `freeblue-mkb` ✅ — TLV parser + processing-key→media-key (spec 03 §3.4.2:
   `mk=AES-128D(Kpc,cvalue)`, `mk[12:16]^=uv`, verify `0x81`/`0x86`).
4. `freeblue-content` ✅ — aligned-unit `block_key=AES-128E(Kcu,seed)⊕seed`,
   AES-128-CBC, IV `0x0BA0F8DD…`, clear 16-B seed (spec 05 §5.3).
5. `freeblue-disc` — read `/AACS/MKB_RO.inf`, `Unit_Key_RO.inf`, certs; **read
   raw (encrypted) `STREAM/*.m2ts`** off the UDF volume (no libaacs).
6. `freeblue-core` — orchestrate to plaintext M2TS (the spec 00 §0.4 contract).

**Phase 2 — Close the v2-only gaps.**
- v2 MKB types (`0x86` verify etc.) in the parser (spec 03 §3.3.1) — additive.
- One **v2 content byte-match** (the last unproven step) — decrypt a real UHD
  `STREAM/*.m2ts` Aligned Unit (spec 05 §5.8). Needs a UHD disc in hand.
- **Key/Volume-ID acquisition strategy** (the real fork):
  - *Disc in the community keydb* → `V`(VUK)+`U`(Unit Key)+`I`(Volume ID) are
    supplied directly; **no drive auth, no device keys needed** — the 80% path,
    fully working today for ~20k UHD titles (spec 06 §6.5.1).
  - *Disc not in keydb* → need the **v2 device keys** (user-sourced) + the
    **Volume ID**, which requires either a v2 P-256 host cert for AACS drive↔host
    auth or a LibreDrive-style raw read (spec 04 §4.3). Hard path.

**Phase 3 — `rdd` integration** (touches rdd spec 02 *disc-access-and-decryption*
and spec 11 *arm-integration*):
- Add a **decryption-backend seam** in rdd: today rdd leans on `libbluray`
  (→`libaacs`) for decrypted reads; for UHD, route to `freeblue` instead.
- **UHD detection** → pick the `freeblue` path; parse BDMV playlists with
  `libbluray`/`libudfread` (the `.mpls/.clpi` are unencrypted) to choose the main
  title and its clips.
- Read **raw encrypted** `m2ts` and pipe through `freeblue` per Aligned Unit →
  plaintext M2TS into rdd's **existing** TS-demux → MKV-remux pipeline (unchanged).
- Map playlist/clip → CPS unit → `Unit Key` (trivial for single-CPS-unit discs;
  spec 04 §4.5 for the multi-unit `[?]`).

**Phase 4 — Validate & harden.**
- Regression: rip the *v1* GoT disc through the freeblue path; byte-match vs
  MakeMKV (closes the loop on already-proven math in real code).
- End-to-end UHD rip → playable `.mkv`; then multi-CPS-unit, seamless branching
  (rdd already handles), performance (AES-NI, stay I/O-bound, rdd spec 07).

**Decision (settled):** `freeblue` is a **standalone, all-original Rust library**,
*not* a patch to `libaacs` (spec 08 §8.6). `libaacs` is a clean-room reference
oracle only — nothing is copied from it. The library is independently authored
so it isn't gated on upstream review, and pure-Rust keeps it provably original
(spec 10 §10.2) and easy to test-drive. The `libaacs` maintainers may *read*
this library + spec as a reference for adding AACS 2.0 to their C code — a
downstream benefit, not a goal. `rdd` consumes `freeblue` as an external crate.
