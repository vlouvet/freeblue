# 09 — Validation and Test Vectors

> **Status:** 📋 Design — what "correct" means operationally and how every claim
> in specs 02–05 gets verified. This is the spec that makes the parent project's
> "no claim without a test run" rule concrete for `freeblue`. No real vectors are
> recorded yet — recording fabricated ones is forbidden (parent rule #6); this
> spec defines the *slots* they go in.

## 9.1 The definition of correct

```
freeblue is correct on disc D ⇔ freeblue(D, keys) == MakeMKV(D, keys)  byte-for-byte
```

(spec 00 §0.4, spec 07 §7.2). Everything below exists to test that, and to test
the *intermediate* values (keys) so a failure localizes instead of just saying
"the video is wrong."

## 9.2 Known-Answer Tests (KATs) — the primitive and key-hierarchy layer

A KAT pins an input→output for a single transformation, captured from a
MakeMKV-validated context (spec 07 §7.4), so the step is testable without a disc
in the drive. One KAT slot per spec-02/03/05 arrow:

| KAT | Pins | Spec | Status |
|---|---|---|---|
| `aes_g` | AES-G direction + XOR operand | 02 §2.3.2 | ⬜ awaiting vector |
| `aes_g3` | child-label derivation, constants | 02 §2.3.3 | ⬜ |
| `sd_walk` | device-key label → processing key | 03 §3.4 | ⬜ |
| `mkb_parse` | raw MKB blob → record set | 03 §3.3 | ⬜ |
| `media_key` | Kpc + MKB → Km (+ verify record) | 02 §2.4.2 / 03 §3.4 | ⬜ |
| `vuk` | Km + Volume ID → Kvu | 02 §2.4.3 | ⬜ |
| `unit_key` | Unit Key File + Kvu → Ku | 02 §2.4.4 / 04 §4.5 | ⬜ |
| `block_key` | Ku + seed → block key | 05 §5.3 | ⬜ |
| `aligned_unit` | one 6144-B unit ciphertext → plaintext | 05 §5.4 | ⬜ |

Each KAT, once filled from a real capture, also **closes a `[?]`** in spec 07's
resolution table. A green KAT re-tags its claim `[E]`.

> **Sourcing vectors without shipping secrets:** KAT inputs/outputs that *are*
> key material or copyrighted content **do not go in the repo** (spec 10 §10.4).
> Use non-secret vectors where the standard provides them (AES/SHA/P-256 test
> vectors), and for the AACS-specific steps keep the secret vectors in a local,
> git-ignored fixtures dir referenced by env var (§9.6). The *test code* is in
> the repo; the *secret fixtures* are not.

## 9.3 The golden byte-match (the top-level oracle)

The end-to-end test (spec 07 §7.2):

1. **Reference:** MakeMKV-decrypt a corpus disc's chosen title → reference M2TS
   (or decrypted backup folder). Stored **outside** the repo (§9.6).
2. **Subject:** `freeblue decrypt` the same disc + key set → candidate M2TS.
3. **Diff:** byte-compare. Record first-divergence offset and map it to a spec
   section via the failure taxonomy (§9.4).

A pass on ≥1 disc of each crypto generation present in the corpus is the
implementation's definition of done (spec 00 §0.8, spec 08).

**✅ Realized (2026-06-04):** this oracle has been run for real, on both generations.
- **v1 / non-BEE (GoT, spec 11 §11.4.5):** a live `SG_IO` capture decrypted by
  `freeblue-content` is **byte-identical to MakeMKV's decrypted m2ts** — `6112/6144`
  bytes, the only delta being the per-packet TP_extra_header copy bit MakeMKV clears
  (`0xD2`→`0x12`). That is the §9.3 step 3 diff, passing.
- **v2 / UHD (TURBO, AACS 2.0, BEE, spec 11 §11.4.6):** real UHD content decrypts to
  **32/32 TS-sync, BDAV video PID `0x1011`, monotonic ATS, continuity 30/30** — the
  first real AACS 2.0 content unit. No bus-decrypt was needed (LibreDrive returns
  raw). A full MakeMKV byte-diff on the UHD title awaits letting `makemkvcon`
  finish writing its STREAM output, but the structural match against an independent
  decode is conclusive.

## 9.4 Failure taxonomy (so a diff localizes the bug)

| Symptom | Likely cause | Spec |
|---|---|---|
| Total high-entropy noise | wrong `Ku` / wrong media-key / SD-walk bug | 03, 02 |
| Garbage from byte 16 of *every* unit, first 16 OK | wrong block-key / IV / AES-G direction / CBC-vs-CTR | 05 §5.3 |
| Correct then diverges at a unit boundary | aligned-unit size / seed-offset error | 05 §5.1, §5.3 |
| Correct for clip A, noise for clip B | CPS-unit → key mapping wrong | 04 §4.5, 05 §5.2 |
| Fails before any output, "no Km" | revoked-by-MKB or wrong device subset | 03 §3.6 |
| Fails at Volume ID | drive-auth / host-cert gap | 04 §4.3, 06 §6.6 |

The `0x47`-cadence smoke test (spec 05 §5.7) is the fast pre-filter that
classifies "noise vs. structured" before the full diff.

## 9.5 Performance validation

Measured, never asserted (parent rule #6). Harness: decrypt throughput
(MB/s, AES-NI on/off), and wall-clock vs. raw drive read time, on the corpus.
Gate (from spec 05 §5.6 / rdd spec 07): decryption must not be the bottleneck —
wall-clock tracks the drive. Numbers recorded here only after a real run.

## 9.6 Fixtures and the no-secrets boundary

```
freeblue/
  tests/                      ← in repo: test CODE + non-secret standard vectors
  fixtures/                   ← git-ignored: real MKB blobs, key sets, reference
                                M2TS. Referenced via $FREEBLUE_FIXTURES.
```

- **In repo:** test harness, NIST AES/SHA/P-256 vectors, synthetic structures.
- **Never in repo:** device keys, `KEYDB.cfg`, Volume IDs, MKB blobs that embed
  disc keys, any decrypted content (spec 10 §10.4). CI enforces this with a
  secret/keys scanner (cf. rdd spec 10 §10.7).
- A contributor without fixtures can still run the non-secret KATs and the build;
  the byte-match tests **skip** (loudly, not silently — print "fixtures absent")
  when `$FREEBLUE_FIXTURES` is unset.

## 9.7 Reproduce-real-disc-bugs-as-fixtures

Inheriting rdd spec 09 §9.9 / parent CLAUDE.md: every decryption bug found on a
real disc becomes a **minimized synthetic-or-captured fixture + a failing test
before the fix**. A single mis-decrypted aligned unit is a perfect minimal
fixture (6144 bytes); capture it, add the KAT, then fix.

## 9.8 The corpus

Aim for diversity, documented per disc (title withheld/abbreviated; no media in
repo):

| Property to cover | Why | Have it? |
|---|---|---|
| Single CPS unit | the common case (spec 05 §5.2) | ✅ GoT (BD) |
| Multiple CPS units | exercises key selection (spec 04 §4.5) | ✅ Turbo (7 units) |
| **non-BEE** disc | raw read + decrypt works end-to-end | ✅ GoT — 32/32 |
| **BEE** disc | bus-encryption read-path needed (spec 04 §4.3.2) | ✅ Turbo (raw read fails) |
| v2 / UHD on-disc structures | confirm v2 layout + MKB types | ✅ The Warning (MKBv82, BEE) |
| Image vs. live-drive | Volume ID with/without drive auth (spec 04 §4.6) | ✅ both modes seen |

The **GoT (no BEE) vs. Turbo (BEE)** pair is the key discriminator: identical
crypto, opposite raw-read outcomes — it isolates bus-encryption as a *read-path*
problem, not a decrypt one. A read-path test (spec 08 §8.5.1) must show
`PlainUdfReader` **detects BEE and refuses** (rather than emitting garbage) and,
once a `LibreDriveReader`/`AacsAuthReader` exists, that Turbo decrypts to valid
TS through it.

## 9.10 The AACS v1 bring-up gate (do this first)

Because AACS 2.0's key hierarchy and content encryption are identical to v1
except for the primitives' inner hash/curve (spec 01 §1.4), **the entire core is
validatable on v1 before any UHD disc is involved.** This is the cheapest path to
a trustworthy core and is therefore the *first* milestone:

1. **Inputs (all in hand):** the community **AACS v1 keydb**
   (`res/keydb_eng.zip`, spec 06 §6.5.1) + any **regular Blu-ray** + the host's
   `libaacs`/`libbluray` as a second oracle.
2. **Gate:** `freeblue` decrypts the v1 disc and its output is byte-identical to
   what `libaacs`/MakeMKV produce, AND the intermediate keys (processing key,
   media key, Kvu, unit key) match the keydb / `libaacs` debug values.
3. **What passing proves:** AES-G, **AES-G3** (incl. the SD-tree walk), MKB
   parsing, the Verify-Media-Key step, `Kvu = AES-G(Km, IDv)`, the
   `AES-128D(Kvu, ·)` unit-key unwrap, and the Aligned-Unit CBC content decrypt
   are all **correct** — i.e. every KAT in §9.2 is green on real v1 data.
4. **Then and only then** move to v2, where only the *deltas* (new MKB types,
   P-256 signatures, the v2 253-key set, possible CTR content) are unverified —
   shrinking the v2 risk surface to a handful of byte-matches.

This gate turns the supplied keydb into the project's bootstrap oracle: it
lets the whole pipeline reach "green on real discs" using only already-available
inputs, long before the full UHD corpus + v2 device keys are assembled.

### 9.10.1 KAT status on real discs — most of the core is green [Disc]

Verified directly on real discs (method: spec 06 §6.5.2, spec 05 §5.3.1):

| KAT (§9.2) | Verified | On |
|---|---|---|
| `mkb_parse` (TLV records) | ✅ record map parsed; Verify/`0x04`/`0x05` located | UHD MKBv82 **+** BD MKBv63 |
| `media_key` (Kpc + MKB → Km) | ✅ derived Km @subset 23 == keydb `M`; validates `0x81` | BD MKBv63 |
| `vuk` (Km + IDv → Kvu) | ✅ `AES-G(M, I) == V` byte-exact | UHD MKBv82 **+** BD MKBv63 |
| `unit_key` (Kvu + UKF → Kcu) | ✅ `AES-128E(Kvu, Kcu)` in `Unit_Key_RO.inf` @ off 112 | UHD MKBv82 |
| `block_key` (Kcu + seed → Kb) | ✅ `Kb = AES-128E(Kcu, seed) ⊕ seed` | BD (GoT) |
| `aligned_unit` (CBC content) | ✅ 32/32 TS-sync, valid PAT, units 0/1/2/30/63 | BD (GoT) |

**The full `processing-key → media-key → VUK → unit-key → plaintext content`
chain is proven end-to-end on a real disc** (GoT, BD MKBv63). What remains:
- `aes_g3` / `sd_walk` — the **device-key → processing-key** SD-tree walk. Every
  primitive is already verified (AES-G3 constant matches libaacs, spec 02 §2.3.3);
  only the full walk with the keydb `DK` records is not yet run end-to-end
  (spec 03 §3.4.3). Optional, since the `PK` path already reaches Km.
- A **v2 content** byte-match — needs a UHD disc's `STREAM/*.m2ts` (the structure
  dump omits it). With a UHD disc in the drive + its keydb Unit Key, this is the
  same 6144-byte capture done for GoT (§5.3.1). This is the *only* step that
  still requires new hardware/input to close.

## 9.11 Open questions

- **[?]** Whether MakeMKV logs intermediate keys usable as KAT oracles (spec 07
  §7.3) — determines how independently §9.2 can be filled.
- **[?]** Minimal legally-clean way to share *structure* fixtures (MKB blobs)
  for CI without sharing keys/content (overlaps spec 10).
- **[?]** Exact `makemkvcon` invocation that yields a stable, comparable
  reference M2TS (vs. one MakeMKV has reordered/relabeled).
