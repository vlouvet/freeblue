# 00 — Overview

> **Status:** 📄 Reference — vision, goals, non-goals, glossary, and the
> confidence model the rest of the series uses. Nothing here is implemented;
> it is the framing the technical specs (02–05) deliver on.

## 0.1 Problem statement

There is **no working Free/Libre Open-Source decryptor for AACS 2.0**, the
copy-protection on Ultra HD (4K) Blu-ray. The FLOSS stack that handles DVD (CSS,
via `libdvdcss`) and ordinary Blu-ray (AACS v1 + BD+, via `libaacs` /
`libbdplus`) stops at the UHD boundary. `libaacs` implements the v1 key
hierarchy but has no v2 support, and the v2 device keys it would need were, until
recently, sealed inside an Intel SGX enclave and never present in extractable
form on any consumer machine.

That last barrier fell in 2023. The SGX.fail team's 37c3 talk *"AACSess"*
(`res/talk-transcript.txt`) demonstrated extraction of a complete AACS 2.0
device-key set and showed that, **with those keys in hand, decryption is the old
v1 pipeline with cosmetic crypto changes** (**[Talk 49:43]**). What does *not*
exist is a written, FLOSS, clean-room **specification** of that pipeline detailed
enough to (re)implement against. Existing accounts are either a one-hour
conference talk, an NDA'd AACS LA document set, or the closed source of MakeMKV.

`freeblue` is that missing specification.

## 0.2 Goals

1. **A complete, cited AACS 2.0 decryption spec.** From a device-key set + an
   encrypted UHD disc to plaintext M2TS, every transformation specified to the
   level of "an engineer could implement it without seeing MakeMKV's source."
2. **Honest confidence labelling.** Because the authoritative spec is under NDA,
   every claim is tagged Established / Reported / RE-pending (§0.6). The
   specification's *integrity* is as important as its completeness: a confident
   wrong claim about a key derivation is worse than a labelled gap.
3. **MakeMKV byte-match as the correctness oracle.** "Correct" is defined
   operationally: `freeblue`'s decryption of a given disc produces M2TS
   byte-identical to MakeMKV's decryption of the same disc (spec 09). No
   appeal to "the algorithm looks right."
4. **A clean-room reference implementation (downstream).** Spec 08 describes a
   GPL-compatible implementation, ideally as a v2 extension to `libaacs`'s model
   or a standalone library `rippidydoodah` can consume to lift its UHD non-goal.
5. **Clean-room and legal discipline by construction.** Reverse-engineering is
   for interoperability and is documented as such (spec 10). No AACS LA NDA
   material is used; no keys ship in the repo.

## 0.3 Non-goals

- **New key extraction.** `freeblue` does **not** develop SGX side channels,
  attack enclaves, or extract keys from any player. The device keys are a
  *published input* (spec 06). We document the math that consumes them, not the
  exploit that produced them. The exploit is already public and is out of scope.
- **Defeating AACS 2.1 traitor tracing.** v2.1's per-user watermarking /
  "sequence key" tracing (**[Talk 51:14]**) is documented for awareness (spec 01
  §1.7) but circumventing it is out of scope.
- **BD+ / BD-J / Cinavia.** Stackable schemes that may sit on top of AACS
  (**[Talk 58:51]**). Not present on the discs studied here; not in scope.
- **Distributing keys or media.** No `KEYDB.cfg` contents, no device keys, no
  ripped media in the repo, ever (spec 10 §10.4).
- **Re-mux / transcode / playback.** That is `rippidydoodah`'s and HandBrake's
  job. `freeblue` ends at "plaintext M2TS bytes."

## 0.4 Scope boundary (the one-line contract)

```
freeblue input:   AACS-content-encrypted M2TS  +  AACS key material
freeblue output:  plaintext M2TS elementary-stream bytes, == MakeMKV's output
```

Everything to the right of `output:` is the **decryption pipeline** (specs 02, 03,
05) — `[Disc]`-verified end-to-end (spec 09 §9.10.1). The key material is
user-supplied (spec 06). On-disc structure parsing is spec 04.

**The input boundary has a wrinkle the project learned from a real disc (spec 04
§4.3.2):** on **bus-encryption (BEE)** discs — most 2013+ Blu-rays *and* the UHD
disc tested — the drive bus-encrypts content on transfer, so a plain read does
**not** yield AACS-content-encrypted bytes. Getting clean input for those discs
needs a **non-bus-encrypted read path** (LibreDrive-style, or AACS auth + bus
key; spec 08 §8.5.1) — a drive-interaction concern that sits *before* the
decryption core, not inside it. The crypto core is correct regardless; the
read-path is the live-disc last mile.

## 0.5 Relationship to rippidydoodah

| | `rippidydoodah` | `freeblue` |
|---|---|---|
| Input | already-decrypted stream (via `libbluray`) | encrypted UHD volume + keys |
| Output | `.mkv` (remux, no transcode) | plaintext M2TS bytes |
| AACS 2.0 | **non-goal** — deferred | **the entire point** |
| Deliverable | a tool | a spec (then a library) |

If `freeblue` succeeds, its reference library becomes a decryption backend
`rdd` can call, and `rdd` spec 00 §0.3 / §2.4's "UHD deferred / AACS bus
encryption blocked" caveats can be revisited. The two projects share a working
agreement, a coding style (Rust; see spec 08), and a test philosophy
(golden-diff against reference tools).

## 0.6 The confidence model (load-bearing)

AACS 2.0's defining document is under NDA (**[Talk 13:57]** "the specifications
are now under NDA"). Therefore most of this series is *reconstruction*. To keep
that honest, **every non-trivial technical claim carries one of three tags**:

| Tag | Meaning | Source of truth |
|-----|---------|-----------------|
| **[E] Established** | Public, documented AACS v1 (or a standard primitive) that v2 reuses unchanged. | AACS LA *Common Cryptographic Elements* book; NIST/RFC standards; `libaacs` source. |
| **[R] Reported** | Stated in the 37c3 talk or another public RE write-up, but not independently re-derived here. | `res/talk-transcript.txt` with a timestamp; the SGX.fail paper. |
| **[Arch]** | Stated in the leaked 2014 AACS LA architecture *draft* deck (`res/arch-deck-extracted.txt`). Firmer than a guess, softer than `[E]` (it is a watermarked proposal). | The deck. |
| **[Disc]** | **Observed/verified directly from a real AACS 2.0 disc** — the MKBv82 UHD dump in `res/` (+ its keydb keys). The strongest evidence for on-disc format and for "v2 = v1" claims that have been byte-reproduced. Caveat: usually a single disc; cross-check a second. | `res/MKB20_v82_…tgz`, `res/keydb_eng.zip`. |
| **[?] RE-pending** | Not yet pinned; an assumption or open question to be resolved by reverse-engineering MakeMKV and byte-matching (spec 07/09). | The MakeMKV oracle; Ghidra. |

Rules for the tag:
- A claim **may not** be written without a tag (or in a context where the
  surrounding tag clearly applies).
- **[?] does not get silently promoted.** Upgrading `[?]` → `[R]` needs a
  citation; `[R]`/`[?]` → `[E]` needs a byte-match or a standards/source
  reference recorded inline.
- This mirrors the parent project's "when unsure, say so" and "cite, don't
  recall" rules — here it is the central discipline, not a footnote.

## 0.7 Glossary

| Term | Meaning |
|------|---------|
| **AACS** | Advanced Access Content System. v1 = HD DVD / Blu-ray (2006). v2 = UHD / 4K Blu-ray (2015). |
| **AACS LA** | AACS Licensing Administrator, the consortium that defines and licenses AACS. |
| **Device Key (Kd)** | A set of secret keys issued to a licensed player; the bottom input to the whole hierarchy. AACS 2.0 set = **253 keys [R]** (**[Talk 47:18]**). |
| **MKB** | Media Key Block. The on-disc data structure a device processes with its device keys to derive the media key; also the revocation vehicle. |
| **Subset-Difference (SD) tree** | The revocation construction (binary tree of 128-bit node labels, one-way-derived) used by the MKB. v2 keeps v1's scheme **[R]**. |
| **Processing Key (Kpc)** | The 128-bit value a device extracts from the MKB by walking the SD tree; the immediate pre-media-key value. |
| **Media Key (Km)** | Derived from the processing key + MKB; one per MKB version. Compromise of a media/processing key can decrypt every disc up to the next MKB revision. |
| **Volume ID** | A per-volume value in a special disc area the drive only releases after drive↔host authentication. Mixed with Km to bind keys to a physical disc. |
| **Volume Unique Key (Kvu)** | `Kvu = AES-G(Km, Volume ID)` **[E]** — binds the media key to this disc. |
| **Title / Unit Key (Kt / Ku)** | Per-CPS-unit content key, stored encrypted in the Unit Key File, unwrapped with Kvu. |
| **CPS Unit** | Content Protection System unit — the granularity at which a title gets its own unit key. |
| **Aligned Unit** | AACS content-encryption granularity: **6144 bytes = 32 × 192-byte M2TS source packets [E]**. |
| **AES-G / AES-G3** | AACS's AES-based functions. AES-G = `AES-128D(k,d) ⊕ d` (one-way) **[E]**; AES-G3 ("Triple AES Generator") = three chained rounds w/ a hardcoded seed, used for SD-tree node derivation **[R/Arch]** (**[Talk 6:48]**, **[Arch]**). |
| **AES-H** | v2-new "SHA-256-based AES hashing function" named in the architecture deck **[Arch]**; construction and role **[?]** (spec 02 §2.3.7). |
| **KCD** | Key Conversion Data — the MKB's media-key material; `Km = AES-G(Kpc, KCD)` **[Arch]** (spec 02 §2.4.2). |
| **SGX** | Intel Software Guard Extensions — the hardware enclave AACS 2.0 used to hide the device keys **[R]**. |
| **EPID** | Enhanced Privacy ID — the group signature scheme SGX uses for remote attestation; leaking its key enabled key download (spec 06) **[R]**. |
| **Remote Attestation** | The SGX protocol by which CyberLink's server provisions keys into a verified enclave (spec 06). |
| **M2TS** | Blu-ray's MPEG-2 Transport Stream variant; 192-byte packets. The plaintext `freeblue` produces. |
| **MakeMKV** | The closed-source reference ripper used here purely as a **behavioral oracle** (spec 07), never as a source to copy. |

## 0.8 Success criteria (definition of "the spec is done")

- A reader who has never seen MakeMKV's source or the AACS LA documents can
  implement the §0.4 contract from specs 02–05 alone.
- Every `[?]` in specs 02–05 has either been resolved (and re-tagged with its
  evidence) or is listed as an explicit open question in that spec's "Open
  questions" section.
- The implementation built from the spec (spec 08) passes the byte-match oracle
  (spec 09) on the disc corpus.
- The legal/clean-room posture (spec 10) is reviewed and the repo is verified to
  contain no keys or media.
