# 02 — Key Hierarchy and Cryptographic Primitives

> **Status:** ✅ Verified — the chain from a device
> key set to a title key, plus the primitives (AES-G, AES-G3, SHA-256, P-256)
> each step uses. Tags per spec 00 §0.6. Steps marked **[E]** are public AACS v1
> that v2 reuses; **[R]** are talk-reported v2 changes; **[?]** must be pinned by
> RE (spec 07) and byte-match (spec 09).

## 2.1 Authoritative sources to resolve `[?]` against

Per the parent project's "cite, don't recall" rule, the `[E]` claims here must
be checked against — **not recalled from** — these. **These are now obtained and
stored in `res/`** (downloaded from the public AACS LA site / VideoLAN), so the
`[E]` claims below are cited to real documents, not memory:

1. **AACS "Introduction and Common Cryptographic Elements" book** v0.91 —
   `res/AACS_Spec_Common_0.91.pdf` (extracted: `res/common.txt`). The canonical
   v1 definitions of AES-G (§2.1.3), AES-G3 (§3.2.2), the key hierarchy, and the
   MKB record set (§3.2.5). Cited below as **[CCE §x]**.
2. **AACS Blu-ray Disc Pre-recorded Book** v0.953 —
   `res/AACS_Spec_BD_Prerecorded_Final_0_953.pdf` (extracted: `res/bd.txt`).
   BD-ROM Volume Unique Key (§3.3), CPS Unit Key file (§3.9.x), and content
   encryption / Aligned Unit (§3.10). Cited as **[BD §x]**.
3. **AACS Pre-recorded Video Book** v0.953 —
   `res/AACS_Spec_Prerecorded_Final_0.953.pdf`. Title-key / transaction layer.
4. **`libaacs` source** — host has the runtime (`libaacs.so.0`, v0.11.1) but no
   source pkg; `res/libaacs_crypto.c` fetched from VideoLAN as the working
   clean-room oracle. Cited as **[libaacs]**.
5. **NIST FIPS 197** (AES), **FIPS 180-4** (SHA-256), **FIPS 186 / SEC 2**
   (P-256 / `secp256r1`).

These are all **v1**: they are the `[E]` baseline. v2 deltas (§2.5) still need
byte-match (spec 07/09), but the baseline they diff against is now solid.

## 2.2 The hierarchy at a glance

Bottom-up — each arrow is a transformation specified below. This is the v1
hierarchy **[E]**; v2 changes only the primitives inside the arrows (§2.4), not
the shape. The architecture deck's own key-derivation diagram (**[Arch]**,
`res/arch-deck-extracted.txt`, "on-line key delivery"/"Device Binding" slides)
shows the *exact same flow* using AACS's primitive names — it is reproduced as
the right-hand column below and is the strongest single confirmation that the v2
shape equals v1:

```
  freeblue / v1 hierarchy            [Arch] deck diagram (verbatim operations)
  ─────────────────────────         ──────────────────────────────────────────
  Device Key set (Kd)               Set of Device Keys ─┐
        │  process MKB (spec 03)                        ├─► Process MKB ─► (Kpc)
        ▼                           MKB ────────────────┘
  Processing Key (Kpc)
        │  AES-G w/ KCD (§2.4.2)     KCD ──► AES_G ──► Media Key
        ▼
  Media Key (Km)
        │  AES-G w/ Volume ID        Volume ID ──► AES_G ──► (Kvu)
        ▼
  Volume Unique Key (Kvu)
        │  decrypt Unit Key File     CPS Unit Key File ──► AES_D ──► CPS Unit Key
        ▼                            (note: "(Empty)" on disc in the *enhanced*
  Title / Unit Key (Ku)               online-key case — keys come from the
        │  content decrypt            Title Key Server; spec 01 §1.4, spec 06)
        ▼
  Block / Content Key  →  decrypt    Encrypted AV Stream ──► AES_D ──► AV Stream
   6144-B aligned units (spec 05)
```

Two facts the deck pins here: (1) the **media key is produced via `AES_G` over
`KCD`** (Key Conversion Data — the MKB's media-key material; §2.4.2), and (2)
content/Unit-Key decryption is drawn as **`AES_D`** (AES-128 decrypt). The deck
abstracts away chaining mode; whether content is CBC or CTR is still **[?]**
(§2.3.6, spec 05 §5.8).

## 2.3 The primitives

The 2014 AACS LA architecture draft (`res/arch-deck-extracted.txt`, "AACS 2.0
Crypto-algorithms" slide) lists the **complete v2 primitive set** — our best
public confirmation of which building blocks exist. Items the deck marks as
*new/changed for v2* are flagged below. This is **[Arch]** (a draft proposal
deck with "TO REVISE"/"proposal" watermarks — firmer than a guess, softer than
the final NDA spec):

| Primitive | Deck name | v2 status | Spec |
|---|---|---|---|
| AES-128 ECB encrypt/decrypt | `AES-128E` / `AES-128D` | unchanged | §2.3.1 |
| AES-128 CBC encrypt/decrypt | `AES-128CBCE` / `AES-128CBCD` | unchanged | §2.3.1, spec 05 |
| **AES-128 Counter mode** | `Counter Mode (AES-128)` | **new in v2** | §2.3.6, spec 05 §5.8 |
| AES-based one-way function | `AES-G` | unchanged | §2.3.2 |
| Triple AES generator | `AES-G3` | unchanged (name confirmed) | §2.3.3 |
| **SHA-256-based AES hash** | `AES-H` | **new in v2** | §2.3.7 |
| SHA-256 hash | `SHA-256` | replaces SHA-1 | §2.3.4 |
| CMAC (AES-based) | `CMAC` | unchanged | — |
| Digital signature | `AACS_Sign`/`AACS_Verify` = **ECDSA 256-bit + SHA-256** | replaces v1 curve | §2.3.5 |

The deck's note — *"UHD Players must continue to support the [v1] crypto-
algorithms [to] playback legacy discs"* (**[Arch]**) — confirms spec 01 §1.4's
"v2 = v1 + new primitives, both retained" thesis at the primitive level.

### 2.3.1 AES-128 (E and D) — [E]

Standard FIPS-197 AES, 128-bit key/block. Notation:
- `AES-128E(k, p)` = encrypt block `p` (16 bytes) under key `k`.
- `AES-128D(k, c)` = decrypt block `c` under key `k`.
ECB unless a chaining mode is stated. CBC is used for content (spec 05).

### 2.3.2 AES-G — the AACS one-way function — [E]

AACS's core one-way function, used for key wrapping/derivation:

```
AES-G(k, d) = AES-128D(k, d) XOR d
```

i.e. decrypt the 16-byte data `d` under key `k`, then XOR the result with `d`.
This is a Davies–Meyer-style one-way compression: given the output you cannot
recover `k` or invert to a parent. **Resolved [E]** — verbatim from **[CCE
§2.1.3]**: *"AES-G(x1, x2) = AES-128D(x1, x2) ⊕ x2"* where `x1` is the key and
`x2` the data. Direction is **decrypt (D)**; the D-vs-E question is closed.

### 2.3.3 AES-G3 — the SD-tree node-derivation function — [R]

Named **"Triple AES Generator (AES-G3)"** (**[CCE §3.2.2]**, **[Arch]**, **[Talk
6:48]**). Derives child node labels in the subset-difference tree (spec 03).
**Resolved [E]** — exact definition from **[CCE §3.2.2 / Fig 3-2]**, confirmed
byte-for-byte against **[libaacs]** `_aesg3()` in `res/libaacs_crypto.c`:

```
Given a 128-bit Device Key k and the 128-bit seed constant
    s0 = 0x7B103C5DCB08C4E51A27B01799053BD9        # [CCE §3.2.2]; matches libaacs seed[]
run the loop 3×, incrementing the seed's last byte each time (s0, s0+1, s0+2),
each step = AES-G(k, sN) = AES-128D(k, sN) ⊕ sN, yielding 384 output bits:
    bits[  0:128] = AES-128D(k, s0)   ⊕ s0    → left-child subsidiary Device Key
    bits[128:256] = AES-128D(k, s0+1) ⊕ (s0+1) → PROCESSING KEY        (spec 03)
    bits[256:384] = AES-128D(k, s0+2) ⊕ (s0+2) → right-child subsidiary Device Key
```

The left/right outputs are ignored when `k` is a leaf Device Key. The **middle**
output is the processing key — confirming the talk's "center value is your
processing key" (**[Talk 7:13]**). `libaacs` implements exactly this: seed
`{0x7B,0x10,0x3C,0x5D,0xCB,0x08,0xC4,0xE5,0x1A,0x27,0xB0,0x17,0x99,0x05,0x3B,
0xD9}`, `seed[15] += inc`, `AES-128D` then XOR seed. **All `[?]` here closed for
v1.** v2 reuses the SD-tree scheme (**[Talk 50:12]**); whether v2 keeps this
exact s0 is the only residual **[?]** (pin by SD-tree byte-match, spec 03 §3.5).

### 2.3.4 SHA-256 — [R]

v2 replaces v1's SHA-1 with **SHA-256** (**[Talk 49:56]**). Used for signature
hashing / content-certificate verification. FIPS 180-4. Where exactly SHA-256 is
applied (cert chain? MKB signature? record digests?) is **[?]** pending spec 04
structure work.

### 2.3.5 P-256 ECDSA — [R]

v2 replaces v1's custom ~160-bit curve with **NIST P-256 / `secp256r1`**
(**[Talk 50:07]**). Used for the drive↔host authentication signatures and
content-certificate / MKB signatures. Standard ECDSA-over-P-256 with SHA-256
(deck: *"ECDSA 256-bit and SHA-256"*, **[Arch]**). The **public keys** (AACS LA
root, drive/host certs) are **[?]** — they must be recovered from disc structures
/ MakeMKV / `libaacs`-style cert handling. The host-cert *layout* is partially
known from **[Arch]** — spec 04 §4.3.1 shows the 40-byte (v1) → 64-byte (v2) key
and signature widths, i.e. the ~160-bit-curve → P-256 transition made concrete.

### 2.3.6 AES-128 Counter mode — [Arch], v2 addition

The deck lists `Counter Mode (AES-128)` as a **new v2 primitive** (**[Arch]**,
flagged red on the slide). v1 content is AES-128-**CBC** (spec 05 §5.3); CTR's
presence in the v2 toolbox means content (or some sub-stream) **may** use CTR.
This does **not** resolve spec 05's CBC-vs-CTR question — it *widens* it: both
modes are now known-available, so the content mode must be pinned by byte-match
(spec 05 §5.8, spec 09). Do not assume CBC.

### 2.3.7 AES-H — AES-based hash — [E], not net-new

**Correction from the source docs:** AES-H is **not** a v2 addition. It is a v1
primitive, **[CCE §2.1.4]** "AES-based Hashing Function (AES_H)", used in key
calculations. The architecture deck's `SHA-256 based AES Hashing Function
(AES-H)` (**[Arch]**) is therefore the *same* function with its inner hash moved
SHA-1 → SHA-256 in v2 — consistent with the general v1→v2 hash swap (§2.3.4),
not a new building block. **Where it matters for `freeblue`:** AES_H appears in
the *downloaded / Virtual-File-System* title-key formula `AES-128E(Kvu, Kt ⊕
Nonce ⊕ AES_H(Volume ID ‖ title_id))` (**[BD §3.x]**, spec 04 §4.5) — i.e. the
*enhanced* (online-key) disc path, not the basic on-disc title-key path. So for
the basic-disc target (spec 01 §1.4.1), AES_H is **off the critical path**; it
becomes relevant only if a corpus disc uses downloaded title keys.

## 2.4 The derivation steps, precisely

### 2.4.1 Device keys → Processing key — spec 03

The device key set + the disc's MKB → a 128-bit processing key `Kpc`, by walking
the subset-difference tree. Fully specified in **spec 03**; summarized here only
to place it in the chain. **[E]** for the v1 mechanism; **[R]** that v2 keeps it.

### 2.4.2 Processing key → Media key — [E], v2 constants [?]

The MKB contains **Media Key Data** records — the deck names this input **KCD
(Key Conversion Data)** (**[Arch]**). `AES-G(Kpc, KCD)` yields a candidate media
key `Km` (deck diagram: `KCD ──► AES_G ──► Media Key`, §2.2). The MKB also
contains a **Verify Media Key Record**; `Km` is correct iff decrypting that
record's known constant validates (spec 03 §3.4). v1 mechanism **[E]**, KCD/AES_G
step **[Arch]-confirmed**; the v2 verify-constant and any new record type are
**[?]**.

```
Km_candidate = AES-G(Kpc, MediaKeyData_for_our_subset)
assert verify_media_key(Km_candidate, MKB.VerifyMediaKeyRecord)   # spec 03 §3.4
```

### 2.4.3 Media key → Volume Unique Key — [E]

```
Kvu = AES-G(Km, Volume_ID)
```

`Volume_ID` is 16 bytes read from the disc's special area *after* drive↔host
auth (spec 04 §4.3, **[Talk 4:58]**). This binds the (revocable, shareable)
media key to *this physical disc*. **[E]**, verbatim from **[BD §3.3]**: *"the
Volume Identifier (IDv) is combined with the Media Key (Km) to produce the
Volume Unique Key (Kvu) as follows: Kvu = AES-G(Km, IDv)"* (for BD-ROM, IDv is
stored in the disc's ROM-Mark).

> **✅ [Disc] — verified on real AACS 2.0.** On the MKBv82 UHD disc
> `res/MKB20_v82_…tgz` (spec 06 §6.5.2), computing `AES-G(Km, IDv)` from the
> disc's Media Key and Volume ID reproduces its Volume Unique Key **byte-exactly**.
> So this step is **not** "assumed unchanged for v2" — it is *confirmed* unchanged.

> **v2 caveat — §2.5.** The talk notes v2 issues *per-EPID* unique key sets
> **[R 50:23]**; whether that inserts an extra derivation between `Km` and `Kvu`,
> or is purely a server-side key-issuance / tracing concern, is **[?]**. Default
> assumption: it does **not** change the on-disc `Km → Kvu` math (the disc cannot
> know which EPID will read it). Confirm via byte-match.

### 2.4.4 Volume Unique Key → Title/Unit keys — spec 04 §4.5

The **Unit Key File** on the disc holds the per-CPS-unit title keys, encrypted
under `Kvu`. Decrypt to recover each `Ku`. Exact file layout, cipher mode, and
record framing are specified in **spec 04 §4.5**; tagged **[?]** there.

### 2.4.5 Title/Unit key → content — spec 05

Each `Ku` drives per-aligned-unit content decryption. See **spec 05**.

## 2.5 v2-specific open issues (the diffs to pin)

The v1 baseline is now fully pinned from the source docs (§2.1); these are the
deltas where **v2** might diverge. Each is an RE target (spec 07) with a
byte-match acceptance test (spec 09). **Resolved-for-v1 items are struck through
with their citation; what remains is the v2-only residual.**

1. ~~AES-G "D vs E"~~ → **[E]** decrypt, **[CCE §2.1.3]** (§2.3.2). *v2: assumed
   same.*
2. ~~AES-G3 constants / ordering~~ → **[E]** s0=`0x7B10…3BD9`, middle output =
   processing key, **[CCE §3.2.2]** + **[libaacs]** (§2.3.3). *v2 residual: does
   v2 keep this exact s0?*
3. **[?]** Whether SHA-256 / P-256 appear only in signature/cert paths (most
   likely) or anywhere in the key-derivation path (would change byte output).
4. **[?]** The v2 Verify-Media-Key constant (Dv) and the **two new v2 MKB type
   IDs** (spec 03 §3.3). *v1 record set now known **[CCE §3.2.5]**.*
5. **[?]** Whether per-EPID key issuance perturbs `Km → Kvu` (§2.4.3 caveat).
6. **[?]** Endianness / byte-order as consumed by AES-G (pin by KAT, spec 09).
7. ~~Where AES-H is applied~~ → **[E]** it's a v1 primitive on the *downloaded*
   title-key path, off the basic-disc critical path, **[CCE §2.1.4]** (§2.3.7).
8. **[?]** **Content chaining mode in v2**. v1 is **CBC** (**[BD §3.10]**,
   resolved — spec 05 §5.3); v2 *adds* CTR to the toolbox (§2.3.6) so v2 content
   mode still needs byte-match confirmation.

## 2.6 Worked example placeholder (to be filled by KATs)

Once a corpus disc + key set is available, spec 09 §9.2 will record a **known-
answer vector** for *every* arrow above (device keys → Kpc → Km → Kvu → Ku),
captured from a MakeMKV-confirmed rip, so the hierarchy is testable end-to-end
without re-running the disc. Until then this section is intentionally empty;
**no fabricated vectors** (parent rule: "no fabricated benchmarks/values").

## 2.7 Implementation notes (forward ref to spec 08)

- All keys are 16 bytes; represent as fixed `[u8; 16]`, never `String`/hex in
  the hot path. Hex only at the I/O boundary (KEYDB parse, spec 06 §6.5).
- AES-G is the single most-reused function; implement and KAT it *first* (it is
  the dependency of §2.4.2–2.4.4 and spec 03 and spec 05). A wrong AES-G fails
  everything silently downstream — exactly the class of bug the parent project's
  TDD rule exists to catch.
- Zeroize key material after use (`zeroize` crate); these are secrets.

## 2.8 Open questions

All of §2.5, plus:
- **[?]** Does v2 use more than one CPS unit / unit key per disc in practice,
  and how is the unit→key index encoded (spec 04 §4.5, spec 05 §5.2)?
- **[?]** Is there a content-certificate validation step that *must* pass before
  decryption (v1 has one), and does failing it change behavior vs. MakeMKV
  (which may skip it)?
