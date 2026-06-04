# 03 — Media Key Block and Revocation

> **Status:** ✅ Verified — how a device key set is turned into a processing key by
> walking the Media Key Block's subset-difference (SD) tree, and how revocation
> works. This is spec 02 §2.4.1's "process the MKB" arrow, in full. Tags per
> spec 00 §0.6.

## 3.1 What the MKB is and why it exists

The **Media Key Block** is the on-disc structure that lets *only unrevoked
devices* derive a media key (**[Talk 5:44]**). It is simultaneously:

- the **input** a device processes (with its device keys) to reach the
  processing key, and
- the **revocation vehicle**: a new MKB version, mastered onto new discs, can
  exclude leaked device keys — at the cost of requiring physical remastering
  (**[Talk 5:55]**, §1.3.1's asymmetry).

A leaked **processing/media key** decrypts every disc up to the MKB revision
that revokes it; a leaked **device key** can be revoked by a future MKB
(**[Talk 6:01]**). `freeblue` therefore must read and process the MKB on *each*
disc — never assume a cached processing key (spec 01 §1.8).

v2 keeps v1's MKB + SD-tree scheme with **new device keys** and **some new
record types** (**[Talk 49:56, 50:12]**). The mechanism below is **[E]** v1; the
v2 record-type deltas are **[?]**.

## 3.2 The subset-difference tree — model

The SD method (Naor–Naor–Lotspiech, 2001, **[Talk 6:16]**) underpins the MKB.
Mental model, from the talk's clearer re-explanation (**[Talk 6:29–9:51]**):

- A **binary tree** where every node is a **128-bit label**.
- A node's **left/right children are one-way-derived** from it via **AES-G3**
  (spec 02 §2.3.3) — derivation only flows *down*; you cannot invert a child back
  to a parent.
- A node may also yield a **center value**; selecting the center *stops descent*,
  and that 128-bit center label **is the processing key** (**[Talk 7:13]**).
- A **device key set** is the labels handed to a device at certain tree
  positions. Every device shares much of the tree but holds **exactly one unique
  leaf** no other device has (**[Talk 7:45]**) — the basis of per-device tracing.
- From its labels a device can **derive all descendants** of those labels, but
  **never** any node on the unique path from its own leaf up to the root
  (**[Talk 8:52]**). That excluded path is what makes the device individually
  revocable.

### 3.2.1 Revocation by subset difference

To revoke device(s), the MKB encrypts the media-key material under a node label
that **every device except the revoked one(s) can derive** (**[Talk 9:20]**).
For multiple revocations, AACS uses **multiple subset-difference subtrees** for
efficient non-contiguous revocation (**[Talk 9:51]**) — the "four-dimensional
triangles" the talk jokes about. Net effect for an *un*revoked device: at least
one MKB subset entry is decryptable with a key it can derive; for a revoked
device, none are.

## 3.3 MKB on-disc format — [E] v1 (resolved from [CCE §3.2.5])

The MKB is a sequence of **type-length-value records**. The v1 record set is now
**resolved [E]** from **[CCE §3.2.5]**: a properly formatted MKB has **exactly
one each** of these eight records (**[CCE §3.2.5]**, "shall have exactly one
Verify Media Key Record, one Type and Version Record, one Explicit
Subset-Difference Record, one Subset-Difference Index Record, one Media Key Data
Record, one Drive Revocation List Record, one Host Revocation List Record, and
one End of Media Key Block Record"):

| Record (v1) | Type ID | Purpose | Tag |
|---|---|---|---|
| **Type and Version** | — | MKB type + monotonically increasing version; **must be first record** | [CCE §3.2.5.1] |
| **Host Revocation List** | — | revoked host certs; signed by AACS_LApub | [CCE] |
| **Drive Revocation List** | — | revoked drive certs; signed by AACS_LApub | [CCE] |
| **Verify Media Key Record** | `0x81` | 20-byte record; `Verification Data (Dv)` validates a candidate media key (§3.4) | [CCE §3.2.5.4] |
| **Explicit Subset-Difference Record** | — | the SD-tree subset assignments `(uv, mu, mv)` | [CCE §3.2.5] |
| **Subset-Difference Index Record** | — | index into the SD record | [CCE §3.2.5] |
| **Media Key Data Record** | — | the per-subset encrypted media-key material (the KCD, spec 02 §2.4.2) | [CCE §3.2.5] |
| **End of Media Key Block** | — | terminator | [CCE §3.2.5] |

(Exact numeric type IDs for the unmarked rows are in **[CCE §3.2.5]** Tables
3-2…3-10; lift them from `res/common.txt` when implementing the parser rather
than guessing.) If any of these eight is missing, **[CCE]** says the device
"shall not process the MKB" — a useful validity check for `freeblue`'s parser.

**v2 deltas:** the talk reports "a couple very similar, just new MKB entries"
(**[Talk 49:56]**) without enumerating them. The architecture deck adds concrete
direction (**[Arch]**, "Device Key Spaces for AACS1 and AACS2.0" slide):

- **AACS 2.0 gets brand-new MKB *types*.** The deck poses "[Q] Which MKB Type is
  assigned to AACS2.0?" and answers: in AACS 1.1 *Type 4* was used to avoid
  legacy issues, but "in AACS2.0 there is no legacy issues… **two kinds of NEW
  types should be prepared**." So expect ≥2 new MKB type IDs distinct from the
  v1 set (the two likely track the *basic* vs *enhanced* disc models, spec 01
  §1.4 / spec 04).
- **The Host/Drive Revocation List Record in the MKB differs from AACS1**
  (**[Arch]**) — consistent with the new P-256 host-cert format (spec 04 §4.3.1).
- **Device key *spaces* are separate** for HD (v1) and UHD (v2): an AACS1 device
  is prohibited from decrypting AACS2.0 content, and a v2 device that also plays
  v1 discs carries **two device key sets + two host certs** (**[Arch]**). For
  `freeblue` this means the v2 device key set (spec 06) is a *distinct* space —
  do not expect v1 keys to index into a v2 MKB.

### 3.3.1 Observed v2 MKB record types — ✅ [Disc]

Parsing the real MKBv82 `MKB_RO.inf` (3.81 MB) from `res/MKB20_v82_…tgz` gives the
**actual v2 record sequence** (TLV: 1-byte type + 3-byte big-endian length).
First record is Type-and-Version with version field `0x00000052` = **82**,
matching the filename:

| order | type | length | maps to (v1 analogue / inference) |
|---|---|---|---|
| 1 | `0x10` | 12 | **Type and Version** (same as v1; version = 82) |
| 2 | `0x21` | 652 | new v2 record |
| 3 | `0x31` | 68 | new v2 record |
| 4 | `0x07` | 1,548 | Host Revocation List |
| 5 | `0x20` | 124 | new v2 record |
| 6 | `0x30` | 68 | new v2 record |
| 7 | `0xF8` | 4 | small (pre-signature marker?) |
| 8 | `0x7F` | 112 | new (signature-region) |
| 9 | `0x86` | 20 | **Verify Media Key Record** — v1 was `0x81`, **same length 20** |
| 10 | `0x04` | 906,356 | **Explicit Subset-Difference Record** |
| 11 | `0x05` | 2,900,308 | **Media Key Data Record** (largest — the KCD, spec 02 §2.4.2) |
| 12 | `0x28` | 68 | new v2 record |
| 13 | `0x02` | 44 | cert / signature record |
| 14 | `0x00` | 0 | **End of Media Key Block** |

So the deck's "two new MKB types" undersold it: alongside the familiar
`0x10/0x07/0x04/0x05/0x02/0x00`, v2 adds `0x20 0x21 0x28 0x30 0x31 0x7F 0x86 0xF8`.
The decryption-critical three are unchanged in *role*: `0x04` (subset-difference),
`0x05` (media-key data), `0x86` (verify-media-key, the v1 `0x81` renumbered, same
20-byte body). **Tagging:** the type IDs and lengths are hard **[Disc]** facts;
the *semantic* labels for the new `0x2x/0x3x/0x7F` records are **inference** to be
confirmed by actually deriving a media key that the `0x86` record validates
(§3.5) and/or against the v2 spec. Field-level layouts: lift from
`res/MKB_RO.inf` when writing the parser, don't guess.

Pinning the remaining record *semantics* and field widths is an RE target
(spec 07), validated by byte-matching the derived processing/media key (§3.5).

> **Parsing discipline:** record parsing is the easiest place to silently
> mis-read a length and corrupt everything downstream. Per parent-project rules,
> the MKB parser lands as failing-test-first against a captured real MKB blob
> (spec 09), not against a hand-imagined layout.

## 3.4 Deriving the media key — ✅ [Disc]-verified

Two entry points: from a **device key set** (full SD-tree walk) or, more simply,
from a **processing key** directly (what `keydb.cfg`'s `PK` records give). The
processing-key path is now **byte-verified on a real disc** (§3.4.2) and matches
**[libaacs]** `_validate_pk` / `_calc_mk_pks` exactly.

### 3.4.1 Record layout used (verified against real MKBs)

- **Explicit Subset-Difference Record (`0x04`)** body = a sequence of **5-byte
  entries**; entry = `[1-byte mask][4-byte UV]`. Iterate entries until the first
  byte has bits `0xC0` set (terminator). The **UV is the last 4 bytes** of each
  entry (offset `1 + a*5`), *not* the first 4 — a classic off-by-one trap.
- **Media Key Data Record (`0x05`)** body = parallel **16-byte cvalues**, one per
  subset `a` at offset `a*16`.
- **Verify Media Key Record (`0x81` in v1; `0x86` in v2, §3.3.1)**, 20-byte
  record → 16-byte **Verification Data `Dv`** at body offset 4.
- (`_simple_record` returns each record body at +4, length −4.)

### 3.4.2 The processing-key → media-key algorithm — [E]/[libaacs]/[Disc]

```
for each subset a (0 ≤ a < num_uvs):
    cvalue = cvalues[a*16 : a*16+16]                 # from 0x05
    uv     = subdiff[1 + a*5 : 1 + a*5 + 4]          # from 0x04 (skip mask byte)
    mk     = AES-128D(Kpc, cvalue)                   # plain decrypt — NOT AES-G
    mk[12..16] ^= uv                                 # XOR uv into mk's last 4 bytes
    if [AES-128D(mk, Dv)]msb64 == 0x0123456789ABCDEF:   # [CCE §3.2.5.4]
        return mk            # this is the Media Key Km
```

The verify constant is **`0x0123456789ABCDEF`** (**[CCE §3.2.5.4]**:
`[AES-128D(Km, Dv)]msb64 == 0123456789ABCDEF`; the record's `Dv` is
`AES-128E(Km, 0123456789ABCDEF‖XXXXXXXXXXXXXXXX)`). Not "DEADBEEF" — pinned from
the book.

> **✅ [Disc] — proven on a real disc.** Game of Thrones: Conquest & Rebellion
> (BD, MKBv63), using `keydb.cfg`'s MKBv63 processing key: the algorithm above
> derived the media key at **subset #23**, and it **equals the disc's keydb `M`
> field byte-for-byte** (and validates against the `0x81` Verify record). This is
> the `mkb_parse` + `media_key` KATs green on real data (spec 09 §9.10.1).

### 3.4.3 From device keys (the SD-tree walk)

When starting from device keys instead of a processing key: walk the SD tree from
each held device-key label down to the subset's node via **AES-G3** (spec 02
§2.3.3, the exact s0 constant + middle-output-is-Kpc, both [E]/[libaacs]), then
apply §3.4.2. Not yet run end-to-end here (the keydb's `DK` records carry the
`DEVICE_NODE`/`KEY_UV`/`KEY_U_MASK_SHIFT` needed, spec 06 §6.5), but every
primitive it uses is already verified. **[E]**, [Disc]-pending.

**v2 residual [?]:** the v2 Verify record is `0x86` not `0x81` (§3.3.1); confirm
the verify *constant* and the `0x04`/`0x05` layout are unchanged in v2 by running
§3.4.2 once with the v2 device/processing keys (the v2 device keys are the
user-sourced input, spec 06).

## 3.5 Validation hook (forward ref to spec 09)

The MKB→processing-key path is **independently testable without decrypting any
video**:

1. Capture a corpus disc's raw MKB bytes (spec 04 §4.4) → fixture.
2. Run `freeblue`'s MKB processor with the published device key set.
3. Oracle: the resulting **media key** must match the one MakeMKV uses for the
   same disc (recoverable from MakeMKV debug output / a captured key, spec 07
   §7.4), and/or must validate against the Verify Media Key Record.

This makes spec 03 the *first* end-to-end-verifiable milestone: a green
processing-key KAT proves AES-G, AES-G3, SD-tree walk, and MKB parsing all
correct, before any spec-05 content work.

## 3.6 Revocation status awareness

- The published v2 device keys **may already be revoked** by MKBs on newer
  discs (AACS LA revokes leaked keys, **[Talk 12:21]**). A disc whose MKB
  version post-dates the key leak may have no decryptable subset → step 5 fails
  for *every* subset. `freeblue` must report this as **"keys revoked by this
  disc's MKB,"** distinct from a parsing/derivation bug.
- This is a **data limitation, not a pipeline defect** — same class as rdd
  spec 02 §2.4's host-cert revocation note. The spec's job is to *diagnose* it
  clearly, not to defeat it (defeating it would need fresh key extraction, a
  non-goal, spec 00 §0.3).

## 3.7 Open questions

- **[?]** v2 MKB record-type table (§3.3): exact IDs and field widths. *Partly
  answered by [Arch]:* ≥2 new MKB types exist and the revocation-list record
  differs from v1 — but the concrete IDs/layouts still need RE + byte-match.
- **[?]** AES-G3 exact construction (spec 02 §2.3.3) — blocks §3.4 step 3.
- **[?]** The 253-key set's internal structure: which keys are SD-tree leaves vs.
  interior labels vs. processing-key-derivation keys (overlaps spec 06).
- **[?]** Whether the Volume ID / MKB also gate on a **content certificate**
  signature (P-256/SHA-256) that must verify before key use (spec 02 §2.8).
- **[?]** How MakeMKV behaves on a revoked-by-MKB disc — does it fail the same
  way, giving us a behavioral oracle for §3.6?
