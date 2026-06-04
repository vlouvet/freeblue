# 12 — Known Issues, Deferred Work, and Bugs Found

> **Status:** 🚧 Living register — the single place to look for "what's wrong,
> what's missing, and what bit us." Each entry carries a confidence tag (spec 00
> §0.6: `[E]` established · `[Disc]` verified on a real disc · `[?]` open) and a
> **state**: 🔴 open blocker · 🟡 deferred · 🟢 resolved (kept as a lesson) ·
> 🐛 bug/gotcha. Cross-references point at the spec that owns the detail. When an
> item is closed, move it to 🟢 with the evidence, don't delete it.

## 12.0 At-a-glance

| # | Item | State | Owner spec |
|---|------|-------|-----------|
| 12.1 | BEE/UHD bus-layer byte-match not done | 🔴 open | 11 §11.4 |
| 12.2 | `LibreDriveReader` does no SCSI of its own (depends on MakeMKV) | 🔴 open | 11 §11.3.2 |
| 12.3 | `makemkvcon` hangs on the Turbo UHD disc | 🔴 blocker | — |
| 12.4 | TP_extra_header copy-bit not cleared in output | 🟡 deferred | 05 §5.8 |
| 12.5 | Aligned-Unit alignment is per-clip-LBA, not absolute | 🐛 gotcha | 05 §5.1 |
| 12.6 | `ts_sync_score` is a weak verification oracle | 🟢 lesson | 11 §11.4.4 |
| 12.7 | Device-key-set file + full SD-tree walk not wired | 🟡 deferred | 03, 06 |
| 12.8 | Multi-CPS-unit key selection | 🟡 deferred | 04 §4.5 |
| 12.9 | Volume-ID acquisition for non-keydb discs | 🟡 deferred | 04 §4.3 |
| 12.10 | `freeblue-cli` is a stub | 🟡 deferred | 08 |
| 12.11 | Capture-harness tooling caveats | 📄 note | 11 §11.4 |

---

## 12.1 🔴 BEE/UHD bus-layer byte-match not done `[?]`

`freeblue-content::bus_decrypt_unit` (per-2048-byte-sector AES-128-CBC over
`[16..2048)`, key `read_data_key`, IV `CONTENT_IV`) is a faithful read of the
`libaacs` reference and is **KAT-tested** (synthetic round-trip + key-sensitivity),
so it is tagged `[E]`. It has **never been byte-checked against a real
bus-encrypted Aligned Unit** — i.e. no `[Disc]` evidence. UHD discs are BEE
(spec 04 §4.3.2), so this is the gating gap for live UHD ripping.

**Why not done:** verifying it needs *one* real bus-encrypted unit **and its
session `read_data_key`, captured together** (the bus key is session-bound). The
only BEE disc on hand (Turbo UHD) can't be read end-to-end by MakeMKV (§12.3), so
no BEE content has transited the bus to test against.

**Open sub-questions:**
- Is `read_data_key` recoverable from captured **bus** traffic (scraped from drive
  RAM via `READ BUFFER 0x3C/0x77`), or is it **computed inside MakeMKV's process**
  from the AKE and never on the bus? If the latter, `LibreDriveReader` must
  replicate the derivation (or scrape MakeMKV's RAM), not just window-search the
  capture. Evidence is inconclusive (the one window-search "hit" was a §12.6
  false positive on metadata).
- Is `read_data_key` **ephemeral** (per-session AKE nonces) or **stable per disc**?
  Unresolved — the apparent cross-session stability seen earlier was on spurious
  (metadata) data and must not be trusted.

**Next step:** get a real BEE-content capture (resolve §12.3, or use any UHD/BEE
disc MakeMKV *can* open), then run the **strong** oracle (§12.6) — bus-decrypt →
content-decrypt → byte-match against MakeMKV's decrypted output — with the unit key
pinned by the disc's Volume ID, not guessed.

## 12.2 🔴 `LibreDriveReader` does no SCSI of its own `[?]`

The decrypt path is proven (spec 11 §11.4.5), but **`freeblue` cannot yet read a
protected disc by itself.** `freeblue-read::LibreDriveReader` is a stub; today the
only way to get content-encrypted units off a real drive is to let **MakeMKV**
perform the LibreDrive SCSI session and capture the `READ(10)` traffic (the
`SG_IO` shim in spec 11 §11.4). To stand alone, freeblue needs the **LibreDrive
unlock/handshake CDB sequence** reverse-engineered (the `READ BUFFER 0x3C/0x77`
protocol, ~200+ commands observed) so it can put the drive in LibreDrive mode and
issue the reads itself. This is a drive-firmware/SCSI dependency, **not crypto**
(spec 11 §11.3.2, §11.4). `AacsAuthReader` (the standards-correct route) needs an
unrevoked P-256 host cert (spec 06 §6.6) and stays impractical.

**Usable today:** `PlainUdfReader` (non-BEE folder dumps / mounted UDF) feeds the
proven core. So freeblue decrypts any **non-BEE** content given the units; the open
work is *acquiring* BEE/UHD units without MakeMKV.

## 12.3 🔴 `makemkvcon` hangs reading the Turbo UHD disc

On the Turbo UHD disc (BEE, `/dev/sr0`, LG WH16NS60), `makemkvcon ... backup`/`info`
**hangs at "Reading Disc information"** — it loops re-reading low-LBA structure
sectors (LBA 0/512/1945/3520, single 2048-byte reads) and never advances to the
high-LBA `STREAM/*.m2ts` content. Multiple runs (110 s … 16 min) never produced a
single high-entropy content `READ(10)`. Behavior is run-to-run variable (an early
run got ~24 structure reads; later runs got none), suggesting **drive/disc
flakiness**, not a `freeblue` issue. An `eject -t` cycle made it worse (the cold
BEE disc then refused plain reads — expected: the drive gates content reads behind
AACS auth, which only MakeMKV's open performs).

**This blocks §12.1** (no BEE content to test against). **Things to try:** clean /
reseat the disc; the other LibreDrive drive (Lite-On iHBS212, `/dev/sr1`); a
**different UHD/BEE disc** MakeMKV can open; or a much longer single run from a cold
boot. Capture recipe and disc/drive map: spec 11 §11.4 + the project memory.

## 12.4 🟡 TP_extra_header copy/encryption bit not cleared in output `[Disc]`

freeblue's decrypted output is byte-identical to MakeMKV **except byte 0 of each
192-byte M2TS packet** (offsets 0, 192, 384, …): the TP_extra_header
copy-permission / encryption-mode indicator. The disc sets the top bits (`0xD2`);
MakeMKV clears them after decryption (`0x12 = 0xD2 & 0x3F`). freeblue currently
passes the disc bytes through verbatim. The **payload and the rest of each header
are identical** (spec 05 §5.8), so this is cosmetic — but some players read the
"encrypted" flag. **Decision deferred:** whether `freeblue` should mask those 2
bits per packet on output (a 1-byte-per-packet step) or leave the stream byte-exact
to the disc. If we mask, add a KAT and a `--raw` opt-out.

## 12.5 🐛 Aligned-Unit alignment is per-clip-LBA, not absolute `[Disc]`

A 6144-byte Aligned Unit aligns to the **start LBA of its `m2ts` clip**, *not* to
absolute disc `LBA % 3 == 0`, and a drive `READ(10)` of 16 sectors (32 KB) is
**not** a whole number of units (32768 / 6144 = 5.33). Naively slicing each 32 KB
read into 6144-byte units therefore **mis-phases most units** and yields garbage
that *looks* like a decryption failure. This bit hard during the GoT verification:
only units that happened to land on a clip-unit boundary decrypted (3 of ~290)
until alignment was fixed to the clip's first LBA.

**Implication for `LibreDriveReader`/any capture consumer:** track the clip's first
LBA and align units to it; reassemble across read boundaries; do **not** assume
read-buffer offset 0 is a unit boundary. The decrypt core is fine — this is a
read-layer bookkeeping rule (spec 05 §5.1, spec 11 §11.2).

## 12.6 🟢 `ts_sync_score` alone is a weak verification oracle (lesson)

**What happened:** an early "31/32 TS-sync ⇒ verified" result was a **false
positive**. Window-searching captured data found *two different* `(read_data_key,
unit_key)` pairs that each "decrypted" 54/120 units to 31/32 TS-sync — but to
**byte-different** plaintexts. The captured `READ(10)`s were near-zero-entropy UDF
metadata (the run never reached content), and **CBC-decrypting low-entropy/zero
data with many keys yields a periodic `0x47`** at the 192-byte cadence (the "valid"
packets were identical: constant arrival timestamp, one PID — not real video).

**Rule (load-bearing):** never accept a key/decrypt on sync-byte cadence alone.
Verify with a **strong oracle**: real high-entropy content **and** structural TS
checks (monotonic arrival timestamps, incrementing per-PID continuity counters,
plausible BDAV PIDs), **or** a byte-match against a known-good decrypt (the §9.3
golden diff). The retraction is recorded in spec 11 §11.4.4; this entry keeps the
lesson visible so it isn't repeated.

## 12.7 🟡 Device-key-set file + full SD-tree walk not wired `[?]`

Two related deferrals on the "no keydb entry" path:
- `freeblue-keys` parses `KEYDB.cfg` but **not** an AACS **device-key-set file**
  (the published v2 device keys). Needed only when a disc isn't in the keydb.
- The **AES-G3 subset-difference tree walk** (device keys → processing key via the
  MKB `DK` records, spec 03) is **not run end-to-end** — verification used the
  keydb's processing/media keys directly. The primitives exist (`aes_g3`); the full
  SD walk is unexercised (spec 09 notes the `DK`-record walk is "not yet run end to
  end").

For the ~20k keydb-listed UHD titles, neither is needed (spec 06 §6.5.1).

## 12.8 🟡 Multi-CPS-unit key selection `[?]`

All verification used **single-CPS-unit** discs (one Unit Key). Mapping a clip /
playlist to the correct CPS unit key on **multi-unit** discs is unimplemented
(spec 04 §4.5, spec 05 §5.2). Additive when a multi-unit disc is in hand.

## 12.9 🟡 Volume-ID acquisition for non-keydb discs `[?]`

For keydb-listed discs the Volume ID is the `I` field (no drive auth). For a disc
**not** in the keydb, reading the Volume ID needs AACS drive↔host auth (host cert,
spec 06 §6.6) or a LibreDrive-style scrape — same dependency as §12.2. Not
implemented.

## 12.10 🟡 `freeblue-cli` is a stub `[E]`

The library is built out (`-crypto/-mkb/-content/-keys/-disc/-read/-core`), but
`freeblue-cli` (`freeblue decrypt` / `freeblue verify`) is not written. Needed for
standalone use and to drive the §9.3 byte-match harness from the command line.

## 12.11 📄 Capture-harness tooling caveats

For anyone re-running the spec 11 capture:
- **snap `makemkvcon` can't be `LD_PRELOAD`-shimmed** (snap confinement strips the
  env). Use the **arm container's** `makemkvcon` (glibc-matched shim, device
  passthrough). Exec with `-e HOME=/home/arm` so it finds the registered key.
- The `SG_IO` shim must **entropy-gate** large `READ(10)`s — otherwise the capture
  is dominated by low-entropy metadata and you'll trip §12.6.
- Capture `0xA4 REPORT KEY` / `0xAD READ DISC STRUCTURE` too, not just `0x3C` —
  bus-key material for BEE may transit there, not only the `0x3C` handshake.
- Never commit captures, decrypted output, or the keydb (Rule 4); clean `/tmp`.

---

## 12.12 Priority for closing

1. **§12.3 → §12.1** — get any BEE/UHD content MakeMKV can read, then byte-match
   `bus_decrypt_unit`. This is the one gap between "non-BEE proven" and "UHD proven."
2. **§12.2** — RE the LibreDrive SCSI sequence so freeblue reads BEE discs without
   MakeMKV (the standalone-ripper milestone).
3. **§12.4 / §12.10** — output-bit decision + CLI (small, ship-blocking polish).
4. **§12.7–§12.9** — the non-keydb / device-key / multi-unit paths (needed only
   outside the ~20k keydb-covered titles).
