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
| 12.1 | ~~BEE/UHD bus-layer byte-match~~ → **no bus layer on LibreDrive path; UHD content decrypted** | 🟢 resolved | 11 §11.4.6 |
| 12.2 | `LibreDriveReader` does no SCSI of its own (depends on MakeMKV) | 🔴 open | 11 §11.3.2 |
| 12.3 | ~~`makemkvcon` hangs on the Turbo UHD disc~~ → fixed by cleaning the disc | 🟢 resolved | — |
| 12.4 | TP_extra_header copy-bit not cleared in output | 🟡 deferred | 05 §5.8 |
| 12.5 | Aligned-Unit alignment is per-clip-LBA, not absolute | 🐛 gotcha | 05 §5.1 |
| 12.6 | `ts_sync_score` is a weak verification oracle | 🟢 lesson | 11 §11.4.4 |
| 12.7 | Device-key-set file + full SD-tree walk not wired | 🟡 deferred | 03, 06 |
| 12.8 | Multi-CPS-unit key selection | 🟡 deferred | 04 §4.5 |
| 12.9 | Volume-ID acquisition for non-keydb discs | 🟡 deferred | 04 §4.3 |
| 12.10 | `freeblue-cli` is a stub | 🟡 deferred | 08 |
| 12.11 | Capture-harness tooling caveats | 📄 note | 11 §11.4 |

---

## 12.1 🟢 BEE/UHD "bus layer" — RESOLVED: there is none on the LibreDrive path `[Disc]`

**Original concern:** UHD discs are BEE, so we assumed live UHD ripping needed a
bus-decrypt step (`bus_decrypt_unit`) before `content_decrypt`, and that step was
unverified on real content.

**Resolution (2026-06-04, spec 11 §11.4.6):** the assumption was wrong for the path
we use. After cleaning the Turbo UHD disc (§12.3), a LibreDrive capture showed its
real m2ts content decrypts with **`content_decrypt` alone — 32/32 TS-sync, BDAV
video PID `0x1011`, monotonic ATS, continuity 30/30** (unit key pinned by the
captured Volume ID `30FFCAF2…`). **LibreDrive returns raw disc sectors; bus
encryption is applied only on a *normal AACS-authenticated* read, which LibreDrive
bypasses.** This is the **first real AACS 2.0 / UHD Aligned Unit decrypted**.

So there is **no bus-layer byte-match owed** for the LibreDrive path. The open
sub-questions about scraping/deriving a `read_data_key` are moot here.
`bus_decrypt_unit` stays in the tree (correct-by-reference, KAT'd) but is exercised
only by the unused `AacsAuthReader` (spec 11 §11.3.3); if that path is never built,
consider it dead code to prune.

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

## 12.3 🟢 `makemkvcon` hung on the Turbo UHD disc — RESOLVED (disc was dirty)

`makemkvcon` had been **hanging at "Reading Disc information"** on the Turbo UHD
disc — looping on low-LBA structure sectors, never reaching content; behaviour was
run-to-run variable. **Root cause: a dirty disc.** After the user cleaned and
re-inserted it, `makemkvcon` read straight through to "Decrypting" and the capture
yielded 64 high-entropy content reads (which closed §12.1). Lesson: a flaky
"hangs reading disc info" on optical media is often physical (clean/reseat first)
before suspecting tooling.

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
- Capture `0xA4 REPORT KEY` / `0xAD READ DISC STRUCTURE` too, not just `0x3C` (cheap
  insurance, though the LibreDrive path turned out not to need bus-key material).
- A "hangs at Reading Disc information" stall is often a **dirty disc** — clean and
  reseat before blaming tooling (§12.3).
- Never commit captures, decrypted output, or the keydb (Rule 4); clean `/tmp`.

---

## 12.12 Priority for closing

With §12.1 (UHD content decrypted) and §12.3 (disc hang) resolved, the decrypt
half is **done** (non-BEE *and* UHD, byte/structurally verified). Remaining:

1. **§12.2** — RE the LibreDrive unlock SCSI sequence so `freeblue` reads discs
   without MakeMKV (the standalone-ripper milestone, and the only thing between
   "decrypts captured units" and "rips a disc itself"). Now known to be a **thin**
   reader (no bus-strip) — capture → align → `content_decrypt`.
2. **§12.5 / §12.10** — robust clip-LBA Aligned-Unit alignment in the reader, and
   the `freeblue-cli` binary.
3. **§12.4** — the TP_extra_header copy-bit output decision (cosmetic).
4. **§12.7–§12.9** — non-keydb / device-key / multi-unit paths (needed only outside
   the ~20k keydb-covered titles).
5. **§12.1 cleanup** — decide whether to prune `bus_decrypt_unit`/`AacsAuthReader`
   if the AACS-auth path is never pursued.
