# 13 — Multi-Drive Support (Generalizing the Read Path Beyond One Drive)

> **Status:** 📋 Design — a proposal for the maintainer. Today the live read path
> works on exactly **one** optical drive (the LG WH16NS60, spec 11 §11.4.2, spec
> 12 §12.14). This spec outlines *how to extend support to other drives* without
> that single-drive coupling. Nothing here is implemented; every design claim is
> `[?]` until built and `[Disc]`-verified, and every reuse claim cites the spec
> section / code that already earned its tag. Confidence tags per spec 00 §0.6.
> Clean-room rules (spec 10) are binding on everything below.

## 13.0 At-a-glance

| # | Item | Axis | State |
|---|------|------|-------|
| 13.3 | Data-driven LibreDrive unlock **registry** (drive-keyed) | A — breadth | 🔵 proposed |
| 13.3.1 | Drive **fingerprint** (INQUIRY + handshake id) | A | 🔵 proposed |
| 13.3.3 | Contributor **capture harness** (scale the RE method) | A | 🔵 proposed |
| 13.3.5 | **Minimize** the unlock sequence to the portable core | A | 🔵 proposed |
| 13.4 | `AacsAuthReader` — drive-agnostic **standard** auth path | B — depth | 🔵 proposed |
| 13.5 | Backend **selection** chain | both | 🔵 proposed |

The two axes are independent and composable. **Axis A** widens the existing
LibreDrive path to more drives (cheaper, no host cert, but per-drive data).
**Axis B** removes drive-specificity entirely via standard AACS auth (more
crypto, needs a host cert, works on any compliant drive). Recommended order:
A first (near-term breadth), B second (durable answer).

---

## 13.1 The problem — the single-drive coupling `[Disc]`

The `LibreDriveReader` (spec 11 §11.3.2, §11.4.7) drives a drive into raw-read
mode by replaying a captured sequence of `READ BUFFER (0x3C, buf-id 0x77)`
commands that read the drive's **internal firmware memory** at fixed offsets
(spec 11 §11.4.2). Those offsets index *that drive's* RAM layout, so
`LIBREDRIVE_UNLOCK_WH16NS60` is meaningless to a different chipset/firmware
(spec 12 §12.14). The coupling is **firmware memory layout**, nothing else — the
decrypt core (spec 05) and the read transport (`scsi.rs`) are already
drive-agnostic.

**Hard safety boundary (carried from spec 12 §12.14, restated because it gates
this whole spec):** the reader is read-only (`READ BUFFER` / `READ(10)` only,
never `WRITE BUFFER`). Probing any drive **cannot brick it** — a non-capable
drive just fails the `MMkv` handshake → `NotLibreDrive`. `freeblue` **never
flashes firmware**; firmware flashing is the only bricking risk and is out of
scope here, permanently. LibreDrive capability requires a specific (MediaTek)
chipset (LG WH16NS60/BU40N, ASUS BW-16D1HT, Pioneer BDR-XD07/XS07 on old
firmware, certain Buffalo externals); a wrong-chipset drive (e.g. the Lite-On
iHBS212 on hand) **cannot** be made capable and must not be coerced.

---

## 13.2 Two axes (overview)

```
                         ┌───────────────────────────────┐
   any drive  ──────────►│  Backend selection (§13.5)     │
                         └───────────────┬───────────────┘
              ┌──────────────────────────┼──────────────────────────┐
              ▼                           ▼                          ▼
     --reader=libredrive         (AXIS A, fast)            (AXIS B, universal)
     explicit override     LibreDrive registry §13.3      AacsAuthReader §13.4
                            (drive-keyed unlock table)     (standard AACS auth)
              └──────────────────────────┼──────────────────────────┘
                                         ▼
                          UnitReader trait (spec 11 §11.2)  ← unchanged seam
                                         ▼
                          freeblue-core::decrypt_clip  ← verified, unchanged
```

Both axes are new `UnitReader` implementations behind the **existing** trait
(spec 11 §11.2). The verified decrypt core never changes — this is purely about
*getting content-encrypted, non-bus Aligned Units* from more hardware.

---

## 13.3 Axis A — generalize the LibreDrive path (breadth)

Replace the single hardcoded sequence with a **data-driven, drive-identified**
unlock. Adding a drive should be adding *data*, not code.

### 13.3.1 Drive fingerprint `[?]`

Identify the drive before choosing an unlock procedure:

- **`INQUIRY (0x12)`** — vendor / product / firmware-revision strings.
- **LibreDrive handshake** — `READ BUFFER (0x3C, mode 2, buf-id 0x77)` returns
  the `id=…` reported in MakeMKV's log (the WH16NS60 returned `4FBA32AEC678`,
  spec 11 §11.4.2). Presence of a valid handshake = LibreDrive-capable.
- **`GET CONFIGURATION (0x46)`** — feature/profile probe (optional discriminator).

All three CDBs are already issuable via the generic `Scsi::from_dev(cdb, len)`
sender in `crates/freeblue-read/src/scsi.rs` (✅ exists — the SG_IO `ioctl`
machinery, `read_buffer(0x3C)`, and `read10(0x28)` are built). A `DriveId`
fingerprint type and an `inquiry()` method are the only additions.

### 13.3.2 Unlock-table registry + `DriveUnlock` trait `[?]`

- Lift `LIBREDRIVE_UNLOCK_WH16NS60` out of code into a **registry** of entries:
  `{ matcher: DriveId predicate, procedure: [ReadBufferOp …] }`. The procedure
  is pure data (mode, buffer-id, offset, length per op).
- Introduce `trait DriveUnlock { fn matches(&self, id: &DriveId) -> bool;
  fn unlock(&self, dev: &Scsi) -> Result<bool, ReadError>; }`. The WH16NS60
  becomes the first registry entry; the interpreter that walks a `procedure` is
  shared.
- Selection: fingerprint → first matching entry → run it → confirm raw mode (the
  `MMkv` handshake) → hand off to the existing `RawUnitIter`.
- **No match but capable** → a distinct, actionable error (e.g.
  `LibreDriveCapableButNoTable { id }`) so the user knows to run the capture
  harness (§13.3.3), not that decryption failed.

### 13.3.3 Sourcing tables without owning every drive — capture harness `[?]`

This is the crux of breadth. Provide a **"characterize my drive"** mode that
automates the exact RE method that produced the WH16NS60 table (spec 11
§11.4.1):

1. ftrace `scsi_dispatch_cmd_start` while a short LibreDrive read runs.
2. Tabulate unique CDBs; isolate the `READ BUFFER (0x3C/0x77)` handshake + reads
   that precede raw `READ(10)`s.
3. Emit a **candidate registry entry** (fingerprint + procedure) for review and
   community submission.

This scales the *method*, not the hardware: contributors with other supported
chipsets generate their own tables. The harness is read-only (§13.1) and emits
**protocol facts only** (offsets/opcodes), never keys or media (Rule 4).

### 13.3.4 Sourcing tables — public LibreDrive facts (clean-room caveat) `[?]`

Firmware fingerprints and memory offsets are **facts**, not copyrightable
expression, and may inform a registry entry. But stay clean-room (spec 10
§10.2): **cite the source, transcribe behaviorally, copy no code**, and do not
bulk-import another project's drive database wholesale without checking its
license. Prefer §13.3.3 captures as the primary, provably-original source.

### 13.3.5 Minimize the unlock sequence `[?]`

The observed WH16NS60 capture is ~521 `READ BUFFER` commands (spec 12 §12.14).
Most are likely incidental; the portable core is the handshake + the few reads
that actually flip raw mode. Minimizing per drive yields a cleaner, more robust,
more portable registry entry and a faster unlock. (Open `[?]`: which subset is
load-bearing — determine empirically per drive, never by guessing.)

---

## 13.4 Axis B — `AacsAuthReader`, the drive-agnostic standard path (depth)

Implement the backend already stubbed in `crates/freeblue-read/src/lib.rs:176`
and specified at spec 11 §11.3.3: do **AACS drive↔host mutual authentication**
(AMAC) with an unrevoked host certificate, derive the **bus key**, and
un-bus-encrypt the transfer ourselves. **No per-drive table — works on any
AACS-compliant drive.**

**What it needs:**
- **Drive↔host auth (AKE)** — P-256 ECDSA handshake via `SEND KEY (0xA3)` /
  `REPORT KEY (0xA4)` with a user-supplied **host certificate/key** (spec 04
  §4.3.1, spec 06 §6.6 — the hard `[?]`: obtaining an unrevoked cert).
- **Bus decryption** — AES-128-CBC per 2048-B sector under `read_data_key`
  (algorithm `[E]`, spec 11 §11.4.3). **The primitive already exists:**
  `freeblue-content::bus_decrypt_unit` is implemented with a round-trip KAT
  (spec 11 §11.4.4). This reader feeds it; it does not re-derive the cipher.
- **Bonus:** standard auth is what gates the **Volume ID**, so this backend also
  resolves Volume-ID acquisition for non-keydb discs (spec 12 §12.9) for free.

**Trade-off:** more protocol/crypto to build (AKE + bus layer) and a host cert is
required — but it is **firmware-independent**, the structural answer to "don't
depend on one drive." The host-cert gap (spec 06 §6.6) is the standing blocker
that makes this currently impractical, so it is the longer-horizon item.

---

## 13.5 Backend selection `[?]`

A small selector in front of the `UnitReader` seam, first hit wins:

1. **Explicit override** — `--reader=libredrive|aacs-auth|plain` (and/or a
   non-BEE `PlainUdfReader`, spec 11 §11.3.1).
2. **LibreDrive registry** (§13.3) — if fingerprint matches an entry. Fast, no
   host cert. Preferred for BEE/UHD on supported drives.
3. **`AacsAuthReader`** (§13.4) — universal fallback when a host cert is
   available and the drive isn't in the registry.
4. **Fail loudly** with the most specific reason (`NotLibreDrive` vs.
   `LibreDriveCapableButNoTable` vs. missing host cert vs. bus-encrypted with no
   capable backend), never emit garbage (spec 11 §11.5).

---

## 13.6 What already exists (reuse map)

Per spec 11 / 12 and the current crates, the seam and most primitives are built:

| Need | Status | Where |
|------|--------|-------|
| `UnitReader` trait (the seam both axes plug into) | ✅ | spec 11 §11.2 |
| SG_IO transport + generic `from_dev(cdb)` sender | ✅ | `freeblue-read/src/scsi.rs` |
| `READ BUFFER (0x3C)` / `READ(10) (0x28)` issuers | ✅ | `scsi.rs` |
| WH16NS60 unlock (first registry entry) | ✅ | spec 11 §11.4.7 |
| Bus-decrypt cipher for Axis B | ✅ `[E]`+KAT | `freeblue-content::bus_decrypt_unit`, spec 11 §11.4.4 |
| Decrypt core (unchanged by either axis) | ✅ `[Disc]` | `freeblue-core::decrypt_clip`, spec 05 |
| `INQUIRY (0x12)` issuer / `DriveId` | ❌ | new (§13.3.1) |
| Unlock registry + `DriveUnlock` trait | ❌ | new (§13.3.2) |
| Capture harness | ❌ | new (§13.3.3) |
| AKE / host-cert auth | ❌ `[?]` | new (§13.4), spec 06 §6.6 |

**Net new engineering:** Axis A = `INQUIRY` + a data-driven registry + a capture
tool (the transport is done). Axis B = the AKE/host-cert handshake (the bus
cipher is done). Neither touches the verified decrypt core.

---

## 13.7 Safety, clean-room, and legal `[E]`

- **Read-only, no flashing** (§13.1) — non-negotiable; probing cannot brick.
- **Clean-room** (spec 10 §10.2) — RE only MakeMKV's *behavior* (SCSI traces),
  never its source; drive offsets are cited facts; copy no code.
- **No secrets in artifacts** (Rule 4) — registry entries and capture output are
  protocol facts (opcodes/offsets) only; host certs/keys are user-supplied at
  runtime and live in git-ignored `fixtures/` / `$FREEBLUE_FIXTURES`, never the
  repo.
- **Honest capability reporting** — distinct errors over silent garbage.

---

## 13.8 Validation plan `[?]`

Per Rule 1 / spec 09, each backend lands test-first:

- **Axis A:** registry interpreter unit-tested against a recorded WH16NS60
  procedure (no hardware); end-to-end `[Disc]` byte-match on each newly captured
  drive (oracle: decrypts to valid TS / matches MakeMKV, spec 09 §9.3). Log
  which drives are `[Disc]`-verified vs. captured-but-unverified.
- **Axis B:** AKE handshake KAT against published P-256 vectors; bus-decrypt
  already has its KAT (spec 11 §11.4.4); end-to-end `[Disc]` byte-match on a BEE
  disc via a non-LibreDrive compliant drive once a host cert exists.

---

## 13.9 Open questions (`[?]` register)

- Which subset of the unlock sequence is load-bearing per drive (§13.3.5)?
- Does MakeMKV *generate* the sequence from a drive-keyed algorithm rather than a
  static table? If so, RE that generator instead of per-drive capture (spec 12
  §12.14).
- Obtaining an unrevoked host certificate for Axis B (spec 06 §6.6) — the
  standing blocker.
- v2 AKE deltas vs. v1 (P-256 vs. legacy curve) for `AacsAuthReader`.

---

## 13.10 Relationship to other specs

- **Supersedes the "generalize" half of spec 12 §12.14** (the other half — the
  do-not-brick caveat — is restated here as §13.1 and remains binding).
- **Resolves on completion:** spec 12 §12.9 (Volume-ID for non-keydb discs) via
  Axis B §13.4.
- **Extends** spec 11 §11.3 (the backend taxonomy) and §11.2 (the `UnitReader`
  seam).
- **Bounded by** spec 10 (clean-room/legal) and CLAUDE.md Rules 1–4.
