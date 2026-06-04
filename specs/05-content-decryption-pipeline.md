# 05 — Content Decryption Pipeline

> **Status:** ✅ Verified — the final arrow: from a title/unit key `Ku` to plaintext
> M2TS bytes. This is the right-hand side of spec 00 §0.4 and the step whose
> output the MakeMKV byte-match oracle (spec 09) checks. v1 content encryption is
> **[E]**; v2 is assumed identical here (the talk reports no content-layer change
> — only hash/curve, **[Talk 49:56]**) but every constant is **[?]** until
> byte-matched.

## 5.1 The aligned unit — the encryption granularity

AACS encrypts content in **Aligned Units** of **6144 bytes = 32 × 192-byte M2TS
source packets** **[E]** (rdd spec 00 §0.5 glossary; AACS CCE book). The M2TS
stream is a sequence of 192-byte packets (4-byte TP_extra_header + 188-byte TS
packet); 32 of them form one AACS aligned unit. Decryption operates one aligned
unit at a time and is **independent per unit** (no cross-unit chaining) — which
is what makes it parallelizable (§5.6).

## 5.2 Picking the right key for a unit

A disc may have multiple CPS units, each with its own `Ku` (spec 04 §4.5). For a
given M2TS clip / playlist, the correct `Ku` is selected via the CPS-unit mapping
(spec 04 §4.5, **[?]**). For the common single-unit disc, there is one `Ku` for
all content. `freeblue` must:

```
unit_key = cps_unit_map.lookup(clip_id_or_playlist)   # spec 04 §4.5  [?]
```

Getting this wrong yields uniformly garbage output (wrong key) — easy to detect
(spec 5.7), distinct from subtle per-packet bugs.

## 5.3 Per-aligned-unit key derivation — ✅ [Disc]-verified

AACS does **not** AES the unit directly under `Ku`; it derives a **per-unit block
key** from the CPS Unit Key (`Kcu`) and a **seed** = the unit's own first 16
bytes. **Resolved [E] from [BD §3.10]** and **byte-verified [Disc]** on a real
disc (Game of Thrones: Conquest & Rebellion, BD MKBv63 — see "Verification"):

```
seed       = first 16 bytes of the Aligned Unit (cleartext)               [BD §3.10]
block_key  = AES-128E(Kcu, seed) XOR seed                                 [Disc] ✅
IV         = 0x0BA0F8DDFEA61FB3D8DF9F566A050F78   (AACS content IV const)  [Disc] ✅
plaintext[16..6144] = AES-128-CBC-Decrypt(block_key, IV, unit[16..6144])  [BD §3.10] ✅
plaintext[0..16]    = seed                                  # clear seed, verbatim
```

Resolved facts (all former `[?]`s now closed):
- **Block Key = `AES-128E(Kcu, seed) ⊕ seed`** — **not** AES-G (which uses
  AES-128*D*). This was the last open `[?]`; pinned by brute-matching candidate
  constructions against real encrypted units until the TS structure appeared.
  Matches **[BD §3.10]** Fig 3-8 (AES-128E-based). ✅ [Disc]
- **Mode is CBC, fresh chain per unit.** **[BD §3.10]**: *"The final 6128 bytes …
  encrypted using the Block Key and AES-128CBCE. A new CBC cipher chain is started
  for each Aligned Unit."* (v2 adds CTR to the toolbox, spec 02 §2.3.6 — but CBC
  confirmed for v1; re-confirm on a v2 *content* sample to be thorough.)
- **CBC IV = `0x0BA0F8DDFEA61FB3D8DF9F566A050F78`** (the AACS content IV
  constant). Used in the verified decrypt; a non-secret public constant.
- **First 16 bytes = the seed, cleartext, copied to output verbatim.** Only the
  final 6128 bytes are ciphertext.

### 5.3.1 Verification — ✅ [Disc] (full content path proven on a real disc)

On `00045.m2ts` (main feature) of a CSS/AACS-v1 retail Blu-ray, using that disc's
Unit Key from `keydb_eng.zip`, the formula above decrypted **every sampled 6144-B
Aligned Unit to valid MPEG-TS** — the TS sync byte `0x47` landed at **all 32**
192-byte packet boundaries in each of units 0,1,2,30,63 (**32/32**), first packet
a clean PAT (`47 40 00 10 …`). Raw (undecrypted) units scored ~1/32, confirming
the input was genuinely encrypted. Method: `freeblue` reads raw (encrypted) M2TS
off the disc via a plain UDF mount (the kernel does no AACS); decryption is all
ours. This is the spec-09 §9.10.1 `block_key` + `aligned_unit` KATs, **green on a
real disc.**

> The core content pipeline is no longer "the most error-prone spec, to be
> proven later" — it is **proven**. The remaining nicety is a byte-for-byte
> diff against MakeMKV's decrypted output of the same unit (spec 09 §9.3), but
> 32/32 TS-sync + valid PAT across units is already conclusive.

## 5.4 The pipeline

```
for each aligned unit (6144 B) in the selected M2TS clip:
    seed       = unit[0..16]                              # §5.3
    block_key  = AES-G(Ku_for_this_cps_unit, seed)        # §5.2, §5.3
    out[0..16]    = seed
    out[16..6144] = AES-128-CBC-Decrypt(block_key, IV0, unit[16..6144])
    emit(out)                                             # plaintext M2TS
```

The emitted byte stream is `freeblue`'s deliverable — directly comparable to
MakeMKV's decrypted output (spec 09) and directly consumable by
`rippidydoodah`'s demux (rdd spec 04), which expects exactly this 192-byte-packet
M2TS.

## 5.5 What freeblue does *not* do here

- **No demux, no remux, no transcode.** Output is plaintext M2TS, packet-for-
  packet aligned with input. Container/codec work is `rdd`'s job (spec 00 §0.5).
- **No seamless-branching / playlist assembly.** `freeblue` decrypts the
  clip(s); assembling clips into a title in play order is `rdd` spec 03/06.
  (`freeblue` only needs the clip→CPS-unit→key mapping, §5.2.)
- **No bus-key handling.** This spec assumes its input is already
  AACS-content-encrypted bytes. On **bus-encryption (BEE)** discs the *read* must
  strip the bus-encryption layer first (spec 04 §4.3.2, spec 08 §8.5.1); that is
  a read-path concern (`freeblue-read`) upstream of this pipeline. Verified: this
  content decrypt is byte-correct on a non-BEE disc (GoT, §5.3.1) and would be
  identical on a BEE disc *given a non-bus-encrypted read*.

## 5.6 Performance shape (forward ref to spec 08)

Because units are independent (§5.1), decryption is **embarrassingly parallel**
and **AES-NI-bound**, not algorithm-bound. Target: decryption throughput well
above optical-drive read speed, so wall-clock tracks the drive — the same
"I/O-bound, not CPU-bound" goal as `rippidydoodah` (rdd spec 07). A pool of
worker threads each handling a span of aligned units, with AES-NI / `aes` crate
hardware backing, is the expected design (spec 08). No benchmarks asserted here
(parent rule: no fabricated numbers) — measured in spec 09 once implemented.

## 5.7 Correctness signals (cheap, before the full oracle)

Even before the spec-09 byte-match, a wrong decryption is usually obvious:
- **Wrong `Ku`/`block_key`/IV/mode** → output is high-entropy noise; the TS sync
  byte `0x47` will *not* appear at the expected 192-byte cadence.
- **Right decryption** → every 188-byte TS packet (offset +4 within each 192-byte
  M2TS packet) starts with **`0x47`**, PIDs are sane, and the first PES headers
  parse. This `0x47`-cadence check is a fast smoke test the implementation can
  self-apply per unit (spec 08), turning silent corruption into a loud failure —
  the parent project's core anti-hallucination value.

## 5.8 Open questions

Most of this section is **closed** — the content path is byte-verified on a real
disc (§5.3.1):

- ~~AES-G direction / seed offset~~ → ✅ block_key = `AES-128E(Kcu, seed) ⊕ seed`,
  seed = first 16 bytes cleartext (§5.3, [Disc]).
- ~~Content IV constant~~ → ✅ `0x0BA0F8DDFEA61FB3D8DF9F566A050F78` (§5.3, [Disc]).
- ~~CBC vs CTR~~ → ✅ **CBC**, fresh chain per unit, confirmed by decrypt (§5.3).
  *v2 residual:* v2 adds CTR to the toolbox (spec 02 §2.3.6); confirm a v2
  *content* sample is still CBC once a UHD disc's M2TS is captured.
- ~~Are the first 16 bytes the only cleartext~~ → effectively yes: decrypting the
  final 6128 B per unit yields 32/32 valid TS packets (§5.3.1), so no extra
  in-unit cleartext on the verified disc.
- **[?]** Multi-CPS-unit key selection per clip (§5.2, depends on spec 04 §4.5) —
  the one genuinely open content-layer item (the test disc is single-unit).
- **[?]** v2-specific content-layer change — current evidence says **none** (the
  key hierarchy is byte-identical v1↔v2, spec 06 §6.5.2); the only unproven piece
  is decrypting a real v2 *Aligned Unit* (needs a UHD disc's M2TS, not just its
  structure dump). This is the last byte-match to fully close the "v2 = v1" thesis.
