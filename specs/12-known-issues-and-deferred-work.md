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
| 12.2 | ~~`LibreDriveReader` depends on MakeMKV~~ → **implemented in Rust/SG_IO; MakeMKV removed** | 🟢 resolved | 11 §11.4.7 |
| 12.3 | ~~`makemkvcon` hangs on the Turbo UHD disc~~ → fixed by cleaning the disc | 🟢 resolved | — |
| 12.4 | TP_extra_header copy-bit not cleared in output | 🟡 deferred | 05 §5.8 |
| 12.5 | ~~Aligned-Unit alignment per-clip-LBA~~ → **reader reads from clip start in whole units** | 🟢 resolved | 11 §11.4.7 |
| 12.14 | Unlock table is **drive-specific** (WH16NS60 only); other drives need their own | 🟡 deferred | 11 §11.4.7 |
| 12.6 | `ts_sync_score` is a weak verification oracle | 🟢 lesson | 11 §11.4.4 |
| 12.7 | Device-key-set file + full SD-tree walk not wired | 🟡 deferred | 03, 06 |
| 12.8 | Multi-CPS-unit key selection | 🟡 deferred | 04 §4.5 |
| 12.9 | Volume-ID acquisition for non-keydb discs | 🟡 deferred | 04 §4.3 |
| 12.10 | ~~`freeblue-cli` is a stub~~ → **`unlock`/`decrypt`/`decrypt-disc` implemented** | 🟢 resolved | 08 |
| 12.11 | Capture-harness tooling caveats | 📄 note | 11 §11.4 |
| 12.13 | ~~Clip extent resolution~~ → **unlock + OS mount + PlainUdfReader (m2ts @off0)** | 🟢 resolved | 11 §11.4.6 |
| 12.15 | Auto unit-key from disc Volume ID + keydb (today `--unit-key` is passed) | 🟡 deferred | 06 §6.5 |

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

## 12.2 🟢 `LibreDriveReader` — RESOLVED: implemented in Rust, MakeMKV removed `[Disc]`

**`freeblue` now reads a protected disc by itself.** The LibreDrive unlock turned
out to be a **static, read-only** sequence of `READ BUFFER` (`0x3C/0x77`) commands
(spec 11 §11.4.7) — no writes, no challenge-response. `freeblue-read` ships a
minimal `SG_IO` layer (`scsi.rs`), the captured unlock table (`libredrive_unlock.rs`),
and `libredrive.rs` (replay + raw `READ(10)` streaming); `LibreDriveReader` wires
them. **Verified end-to-end on the cold TURBO UHD disc with no MakeMKV:** the
`raw_read` example read 8 Aligned Units that decrypt 8/8 at 32/32 (video PID
`0x1011`). `AacsAuthReader` (the standards-correct route) remains unbuilt and is now
unnecessary. Residual items split out as §12.13 (extent resolution) and §12.14
(per-drive unlock tables).

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

## 12.5 🟢 Aligned-Unit alignment — RESOLVED in the reader `[Disc]`

The trap: a 6144-byte Aligned Unit aligns to the **start LBA of its `m2ts` clip**,
*not* to absolute disc `LBA % 3 == 0`, and a 16-sector (32 KB) `READ(10)` is **not**
a whole number of units (32768 / 6144 = 5.33). Naively slicing 32 KB reads into
units mis-phases most of them (it cost us during GoT analysis: 3 of ~290 decrypted).

**Fixed in `freeblue-read::libredrive::RawUnitIter`:** it reads starting at the
clip's first LBA (which *is* a unit boundary) and advances a **whole number of
units per `READ(10)`** (`BATCH_UNITS × 3` sectors), so units never straddle a read
boundary. Verified: 8 consecutive units from a clip-aligned start decrypted 8/8 at
32/32 (spec 11 §11.4.7). Correctness now depends only on the **extent** being right
(§12.13).

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

## 12.10 🟢 `freeblue-cli` — RESOLVED (unlock / decrypt / decrypt-disc) `[Disc]`

`freeblue-cli` now ships three subcommands wiring the verified pipeline:
- `freeblue unlock <device>` — replay the LibreDrive unlock (raw-read mode).
- `freeblue decrypt <m2ts> --unit-key <hex> -o <out>` — `PlainUdfReader` +
  `decrypt_clip` → plaintext M2TS (for a mounted unlocked disc, or a folder dump).
- `freeblue decrypt-disc <device> --start-lba N --num-units M --unit-key <hex> -o <out>`
  — unlock + raw `READ(10)` the extent + decrypt, all over SG_IO.

**Verified standalone (no MakeMKV):** cold `unlock` → mount → `decrypt` of the TURBO
UHD main title's head produced 1024 units at **32768/32768 TS-sync (100%)**, full
BDAV PID set (video `0x1011` + 6 audio). Remaining CLI nicety: auto unit-key
(§12.15) and a `verify` subcommand for the §9.3 byte-diff.

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

## 12.13 🟢 Clip extent resolution — RESOLVED via mount-after-unlock `[Disc]`

No bespoke UDF parsing needed. **After `freeblue unlock`, ordinary OS reads return
raw (content-encrypted) content** (the unlock is drive-global), so the disc mounts
normally and `PlainUdfReader` reads `BDMV/STREAM/*.m2ts` straight off the
filesystem — and an `m2ts` file **starts on an Aligned-Unit boundary** (offset 0),
so units are read aligned with no LBA math (closes §12.5 for this path too).
Verified: mounted-file decrypt of the TURBO main title = 32768/32768 TS-sync.
Title/clip *selection* (which `m2ts`) is just "pick the largest" or read the main
`.mpls`; the `decrypt-disc` SG_IO path still takes an explicit extent for the
no-mount case. (UDF parsing inside `freeblue-disc` remains an option for a
mount-free library, but isn't required.)

## 12.14 🟡 Unlock table is drive-specific (WH16NS60 only) `[?]`

`LIBREDRIVE_UNLOCK_WH16NS60` is the exact sequence observed on the LG WH16NS60
(LibreDrive v06.3). Other LibreDrive-capable drives/firmware likely need their own
captured table (the offsets index that drive's RAM layout). Generalizing means a
per-drive table registry (keyed by INQUIRY id) or RE'ing how MakeMKV *generates*
the sequence. Also worth doing: **minimize** the 521-command sequence to the subset
that flips raw mode (the handshake + likely a few reads), for a cleaner, more
portable unlock.

**Drive caveat (do not brick):** LibreDrive requires a specific (MediaTek) chipset —
LG WH16NS60/BU40N, ASUS BW-16D1HT, Pioneer BDR-XD07/XS07 (old firmware only),
Buffalo externals. The **Lite-On iHBS212 here is NOT LibreDrive-capable** (wrong
chipset, non-UHD platform) and cannot be made so by flashing — do not attempt it.
The reader is read-only (`READ BUFFER`/`READ(10)`), so probing any drive can't brick
it (it just fails the `MMkv` handshake → `NotLibreDrive`); firmware *flashing* is the
only bricking risk, and `freeblue` never flashes.

## 12.15 🟡 Auto unit-key from disc Volume ID + keydb `[?]`

`freeblue decrypt` currently takes `--unit-key <hex>` (the keydb `U` field, looked
up by hand). Turnkey resolution = read the disc's **Volume ID** (SG_IO `READ DISC
STRUCTURE 0xAD`, AACS format — available after unlock) → match it against the keydb
`I` fields → use that entry's `U`/`V`. `freeblue-keys` already parses the keydb;
the missing pieces are the `0xAD` Volume-ID read and a by-Volume-ID lookup index.

---

## 12.12 Priority for closing

**freeblue rips a real UHD disc end to end on its own — MakeMKV removed** (§12.1,
§12.2, §12.5, §12.10, §12.13 all resolved; verified: cold `unlock` → mount →
`decrypt` = 100% TS-sync). Remaining is convenience + breadth:

1. **§12.15** — auto unit-key (read Volume ID via `0xAD`, look up the keydb), so the
   user needn't pass `--unit-key`. The last step to a true one-liner.
2. **§12.14** — per-drive unlock tables (other LibreDrive drives; minimize the
   sequence). **Not** the iHBS212 (not capable).
3. **§12.4 / `verify`** — output copy-bit decision; a `verify` subcommand for §9.3.
4. **§12.7–§12.9** — non-keydb / device-key / multi-unit paths (only outside the
   ~20k keydb-covered titles).
5. **§12.1 cleanup** — prune `bus_decrypt_unit`/`AacsAuthReader` if AACS-auth is
   never pursued.
6. **rdd integration** (spec README Phase 3) — freeblue as rdd's UHD backend.
