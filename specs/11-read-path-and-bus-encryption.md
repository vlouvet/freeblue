# 11 — Read Path and Bus Encryption

> **Status:** ✅ Verified (read path) — the layer *between the optical drive and
> the decrypt core*. **Headline result (2026-06-04, §11.4.6): the LibreDrive read
> path returns RAW disc sectors — no bus encryption — so a real UHD/AACS 2.0 disc
> (Turbo, BEE) decrypts with `content_decrypt` alone (32/32, video PID `0x1011`).**
> Bus encryption (§11.2) afflicts only *plain/AACS-authenticated* reads, not the
> LibreDrive path we use. This **supersedes** the earlier "BEE is the blocker for
> UHD" framing below. Confidence tags per spec 00 §0.6.

## 11.1 The problem (recap) — and the key path distinction ✅ [Disc]

The decrypt core is proven (spec 09 §9.10.1), but it can only run if its input is
**AACS-content-encrypted** M2TS — the bytes as encrypted on the disc. How a read
yields those bytes depends on the **read path**:

| Read path | What it returns | Decrypt needed |
|---|---|---|
| **Plain UDF/`dd`**, non-BEE disc | AACS-content-encrypted | `content_decrypt` ✅ |
| **Plain UDF/`dd`**, **BEE** disc | content-encrypted **+ bus-encrypted** (drive adds a bus-key layer during normal AACS auth) | bus-strip **then** content — **blocked** without the bus key |
| **LibreDrive** (MakeMKV vendor SCSI), **any** disc incl. BEE/UHD | **RAW disc sectors** — content-encrypted only, **no bus layer** | `content_decrypt` ✅ |

**The correction that matters:** bus encryption is something the **drive** does on a
*normal authenticated* transfer for a BEE disc. **LibreDrive bypasses the drive's
AACS auth and reads raw**, so its `READ(10)` output is *not* bus-encrypted — even on
a BEE/UHD disc. This is `[Disc]`-verified (§11.4.6): GoT m2ts byte-matches MakeMKV;
Turbo UHD m2ts decrypts 32/32 with `content_decrypt` alone; Turbo's unencrypted JAR
files come back as plaintext. The earlier "LibreDrive `READ(10)` is bus-encrypted"
claim (§11.4.3) was measured on **UDF metadata / a plain read** and was wrong for
the LibreDrive path.

So the **practical** read path is simple: LibreDrive raw read → `content_decrypt`.
The bus-key machinery (§11.2, §11.4.3) is real AACS but matters only to the
standards-correct `AacsAuthReader` (which we don't use) — it is **not** on the
LibreDrive critical path.

## 11.2 The `UnitReader` abstraction

Keep the read path behind a trait so the verified decrypt core stays
read-agnostic; only the reader changes per disc/drive (spec 08 §8.5.1):

```rust
/// Yields raw AACS-content-encrypted (NOT bus-encrypted) 6144-B Aligned Units
/// for a clip. The decrypt core (freeblue-content) consumes these.
pub trait UnitReader {
    fn read_units(&mut self, clip: ClipId)
        -> Result<Box<dyn Iterator<Item = io::Result<[u8; 6144]>> + '_>, ReadError>;
}
```

Contract: a `UnitReader` returns **content-encrypted, non-bus-encrypted** units.
How it removes (or avoids) bus encryption is the backend's business. Backends must
**never silently emit bus-encrypted bytes** — on a BEE disc a backend that cannot
strip bus encryption must return `ReadError::BusEncrypted`, not garbage (so the
TS-sync smoke test in spec 05 §5.7 is a backstop, not the only guard).

## 11.3 Backends (increasing difficulty)

### 11.3.1 `PlainUdfReader` — ships first, non-BEE only

A plain UDF/file read of `BDMV/STREAM/*.m2ts` (or a folder dump). Correct **only**
for non-BEE discs and pre-extracted structure folders. **Must detect BEE and
refuse** (§11.5) rather than emit garbage. Trivial; covers GoT and any non-BEE
disc + the offline-fixture path.

### 11.3.2 `LibreDriveReader` — ✅ implemented, the route for BEE/UHD (no MakeMKV)

Replay the LibreDrive unlock to a compatible drive over `SG_IO`, then `READ(10)`
**raw on-disc (content-encrypted, non-bus)** content. This is what makes live
BEE/UHD ripping work, and it is now **implemented and `[Disc]`-verified without
MakeMKV** (§11.4.7): a drive-firmware/SCSI dependency, **not crypto**. Needs a
LibreDrive-capable drive (the LG WH16NS60 and Lite-On iHBS212 here report
LibreDrive v06.3) and that drive's unlock table (§11.4.7, currently the WH16NS60's).

**`LibreDriveReader` is THIN, not thick** (revised — §11.4.6): because LibreDrive
returns raw sectors with **no bus layer**, the reader's job is just *issue the
LibreDrive unlock + raw `READ(10)`s → emit content-encrypted Aligned Units* (the
verified core then `content_decrypt`s them). No bus-strip step. Care item: align
emitted units to the **clip's start LBA**, reassembling across 32 KB read
boundaries (spec 12 §12.5) — a 16-sector read is not a whole number of 6144-byte
units.

### 11.3.3 `AacsAuthReader` — the "correct" but hard route

Do AACS drive↔host mutual auth (AMAC) with an **unrevoked** host certificate
(spec 04 §4.3.1), derive the bus key, and un-bus-encrypt the transfer ourselves —
the same thing libaacs *would* do if its host cert weren't revoked. Needs a usable
host cert/key (spec 06 §6.6, the hard `[?]`). Doesn't depend on LibreDrive
firmware, but the host-cert gap makes it currently impractical.

## 11.4 LibreDrive mechanism (what's known) + the RE plan

**Known (public / observed):**
- LibreDrive is a MakeMKV capability for drives with compatible (often
  downgraded/patched) firmware; it lets MakeMKV bypass the drive's AACS
  enforcement and read raw sectors. Reported as `Using LibreDrive mode (v06.3
  id=…)` in the MakeMKV log.
- It is engaged via **vendor-specific SCSI commands**, not standard `READ`. The
  per-drive `id=…` suggests a drive-keyed unlock/handshake.
- After MakeMKV exits, the effect does not persist to ordinary OS reads (§11.1).

**[?] to reverse (the work):** the exact CDBs — the unlock/handshake sequence and
the raw-read opcode(s) + parameter layout that return non-bus content.

### 11.4.1 RE method — SCSI trace of a MakeMKV LibreDrive read

Capture every SCSI CDB MakeMKV sends while it reads a BEE disc via LibreDrive,
then isolate the non-standard (LibreDrive) commands:

1. Enable kernel SCSI tracing: `events/scsi/scsi_dispatch_cmd_start` (ftrace) —
   logs the full CDB of every command dispatched to the drive.
2. Run a **short content read** (`makemkvcon mkv`/`backup`, killed after a few
   seconds) of the BEE disc (Turbo, sr0) so the *content-read* path runs, not just
   structure probing.
3. Tabulate unique CDB opcodes; standard MMC opcodes (`0x28` READ(10), `0xA8`
   READ(12), `0xAD` READ DISC STRUCTURE, `0xA3/0xA4` SEND/REPORT KEY, `0xBE`
   READ CD) are the AACS/MMC baseline; **vendor opcodes (`0xC0–0xFF`) and unusual
   parameter patterns are the LibreDrive candidates.**
4. Correlate the read opcode + LBA pattern against the `m2ts` extents to identify
   the raw-content-read command, and the preceding handshake CDBs.

Output: a documented command table → implement `LibreDriveReader` test-first
against a real drive (the byte-match oracle is "decrypts to valid TS", spec 05).

### 11.4.2 First capture — ✅ [Disc] (LG WH16NS60, LibreDrive v06.3)

Traced `scsi_dispatch_cmd_start` while `makemkvcon mkv` read the BEE disc (Turbo,
sr0 = SCSI host1) — 928 commands to the drive. The mechanism is **not a vendor
opcode**; it is clever abuse of standard MMC commands:

| Opcode | Command | Count | Role |
|---|---|---|---|
| `0x3C` | **READ BUFFER** | **518** | **the LibreDrive primitive** — `3c 02 77 …` (mode 2 "data", buffer-id `0x77`) reads the drive's **internal memory**. Used for the handshake (the reported `id=4FBA32AEC678`) and many small targeted reads at incrementing offsets. **No `WRITE BUFFER` (`0x3B`) seen → read-only, i.e. key/state *extraction*, not firmware patching.** |
| `0x28` | READ(10) | 339 | content/sector reads, `28 08 .. lba=N txlen=16` (16-sector = 32 KB stages) |
| `0xAD` | READ DISC STRUCTURE | 10 | AACS structures (MKB etc.) |
| `0x46` | GET CONFIGURATION | 18 | feature probing / LibreDrive feature detection |
| `0x12`,`0x4A`,`0xBB`,`0x25`,`0x43`,`0x51` | INQUIRY / GET EVENT / SET CD SPEED / READ CAPACITY / READ TOC / READ DISC INFO | — | standard MMC setup |

Representative LibreDrive CDBs (protocol facts, not secrets — spec 11 §11.6):
```
READ BUFFER   3c 02 77 00 00 00 00 00 40 00   (mode=2, buf_id=0x77, off=0, len=0x40)  ← handshake/id
READ BUFFER   3c 02 77 12 01 00 00 00 04 00   (off=0x120100, len=4)                   ← targeted mem reads
READ(10)      28 08 00 00 01 40 00 00 10 00   (lba=320, txlen=16)                     ← content staging
READ DISC STRUCTURE  ad 01 00 00 00 00 00 00 00 20 00 00  (AACS format)
```

**Interpretation (the model to verify next):** LibreDrive uses **READ BUFFER
(`0x3C`, buffer-id `0x77`)** as a window into the drive's RAM/firmware to extract
a secret (the drive's AACS keys or bus-key state — read-only, no writes), then
reads content with normal **READ(10)**. Whether READ(10) then returns *raw* bytes,
or returns bus-encrypted bytes that MakeMKV un-bus-encrypts in software using the
extracted secret, is **the open question** — and it explains why a *separate*
plain `dd` after MakeMKV exits still gets bus-encrypted data (§11.1): the
extracted-secret/state lives in MakeMKV's process, not the drive.

**Resolved by SG_IO data capture (`[Disc]`, prior session):** READ(10) returns
**bus-encrypted** bytes — a captured content buffer failed all 32 candidate unit
keys at every alignment. So `LibreDriveReader` is **thick**: scrape a secret via
`0x3C`, READ(10), **bus-decrypt**, then hand AACS-content-encrypted units to the
verified core. (That captured READ(10) failed *because* there was no bus-decrypt
step in between — see §11.4.3.)

### 11.4.3 The bus-decryption algorithm — ✅ [E] (libaacs reference, citation-only)

The AACS bus-encryption read path is fully specified in the `libaacs` reference
(read as an oracle per CLAUDE.md Rule 2 — **protocol facts/constants only, no code
copied**). It is structurally identical to AACS content decryption, which
`freeblue` already byte-verifies (spec 05). The two layers, in order:

1. **Bus-decrypt** the unit (cite `aacs.c:_decrypt_unit_bus` / `aacs_decrypt_bus`):
   - Gate: only if `unit[0] & 0xC0` (the same "encrypted" flag content uses).
   - Granularity is the **2048-byte sector**, *not* the 6144 unit
     (`#define SECTOR_LEN 2048 /* bus encryption block size */`, `aacs.c:47`).
     A 6144 unit = **3 sectors**; bus-decrypt each independently.
   - Per sector: **leave bytes `[0..16)` as-is**, AES-128-**CBC**-decrypt bytes
     `[16..2048)` (2032 B) with key = **`read_data_key`**, IV = the **same AACS
     IV** `0x0BA0F8DDFEA61FB3D8DF9F566A050F78` freeblue already has as
     `CONTENT_IV` (cite `crypto.c:crypto_aacs_decrypt`).
2. **AACS content-decrypt** the (now non-bus) unit — freeblue's proven path
   (spec 05): per-unit seed in `[0..16)`, `block_key = AES-128E(Kcu,seed)⊕seed`,
   AES-128-CBC over `[16..6144)`.

The **`read_data_key`** is the only new secret:
`read_data_key = AES-128-D(bus_key, encrypted_read_data_key)` (cite
`mmc.c:_read_data_keys`), where `bus_key` is the ECDH AKE output
(`crypto.c:crypto_create_bus_key`: low 128 bits of the x-coord of
`host_priv × drive_point`) and `encrypted_read_data_key` is reported by the drive.
The legit path needs an **unrevoked host cert** for the AKE (the `AacsAuthReader`
blocker, §11.3.3). **LibreDrive sidesteps the AKE** by scraping the drive's RAM —
so what its `0x3C/0x77` reads extract must be the `read_data_key` itself (or the
`bus_key` + `encrypted_read_data_key` to derive it).

**This collapses the remaining RE to a search with a hard oracle.** The Turbo unit
key is known (keydb). For any candidate 16-byte `read_data_key` K and a captured
bus-encrypted unit U (with `U[0] & 0xC0`):
`ts_sync_score(aacs_decrypt(unit_key, bus_decrypt(K, U))) ≈ 32` **iff** K is
correct. So: capture the `0x3C` responses + one READ(10) unit, then brute-test
every 16-byte window in the `0x3C` dumps as K against the oracle. A hit *is* the
key and proves the whole thick-reader model end to end.

**Next RE step (`[?]` → mechanical):** capture `0x3C/0x77` response buffers + a
content READ(10) unit (LD_PRELOAD `SG_IO` shim on `makemkvcon`, BEE disc = Turbo
on sr0), then run the window-search oracle above. Then (optional, fully drive-only
ripping) replay MakeMKV's `0x3C` offset sequence ourselves to scrape K without
MakeMKV in the loop.

### 11.4.4 Bus decryption — 🚧 algorithm [E], live verification NOT YET achieved

The §11.4.3 algorithm (bus = AES-128-CBC per 2048-B sector, key `read_data_key`,
IV = `CONTENT_IV`) is a faithful read of the `libaacs` reference and is implemented
+ KAT-tested (`freeblue-content::bus_decrypt_unit`, synthetic round-trip). What is
**not** yet done is confirming it against a **real captured bus-encrypted unit** —
and a first capture attempt produced a cautionary false positive worth recording.

**The `ts_sync_score ≥ 31` oracle is too weak to identify keys by itself.** An
`SG_IO` shim under `makemkvcon backup disc:0` (Turbo, LG WH16NS60) captured 209
`0x3C/0x77` reads + 24 `READ(10)`s. Brute-searching every 16-byte window of the
`0x3C` data as a candidate `read_data_key` found **two different `(read_data_key,
unit_key)` pairs that each "decrypt" 54/120 units to 31/32 TS-sync** — but to
**byte-different** plaintexts (PIDs `0x0DBF` vs `0x026E`). Both are spurious:

- The captured `READ(10)` buffers are **near-zero entropy** (mean ≈ 0.3 bits/byte)
  at **low LBAs (32–528)** — i.e. UDF/BDMV **metadata**, not the high-entropy
  (~8.0) encrypted m2ts. In the kill window, `backup` never reached title content.
- CBC-decrypting low-entropy/zero data with many keys yields **periodic `0x47`**;
  the "31 packets" were identical (constant ATS, one PID) — not real video.

**Lesson (load-bearing):** verifying a key needs a **strong** oracle — real
high-entropy content + structural TS checks (monotonic ATS, incrementing
continuity counters, plausible BDAV PIDs like `0x1011`), or a byte-match against a
known-good decrypt — never bare sync-byte cadence on whatever sectors happened to
be read.

**Next step:** re-capture with the shim **entropy-gated** to keep only
high-entropy `READ(10)`s (or let `makemkvcon mkv` of the main title run long enough
to read real content), then re-run the search with the strong oracle and the
keydb unit key pinned by the disc's **Volume ID** (not guessed from 5 candidates).
Until then the live LibreDrive path stays `[?]`; only the *algorithm* is `[E]`.

### 11.4.5 Capture path + content decrypt — ✅ [Disc] VERIFIED byte-vs-MakeMKV (GoT)

The whole **read+decrypt apparatus** is proven on a real disc by a byte match
against MakeMKV's own output. An `SG_IO` shim under `makemkvcon backup --decrypt
disc:1` (GoT, non-BEE, on the iHBS212) captured the `READ(10)` content **and**
MakeMKV wrote its **decrypted** `BDMV/STREAM/*.m2ts` — a ground-truth oracle.
Taking captured (encrypted) `READ(10)` units, aligning to Aligned-Unit boundaries,
and running `freeblue-content::decrypt_aligned_unit` with the **keydb unit key**:

- Recovered units are **32/32 TS-sync** and **byte-identical to MakeMKV's
  plaintext** — `6112/6144` bytes exact. The only differences are **byte 0 of each
  192-byte M2TS packet** (offset 0, 192, 384, …): the **TP_extra_header
  copy-permission/encryption indicator**. The disc has the top bits set (`0xD2`);
  MakeMKV clears them post-decrypt (`0x12 = 0xD2 & 0x3F`). The 6128-byte payload +
  the other 3 header bytes are **identical**. (freeblue may optionally clear those
  2 bits to match players' expectations — a 1-byte-per-packet cosmetic step, not
  decryption.)

This `[Disc]`-verifies, against an independent oracle (not a self-judged TS score):
the `SG_IO` capture shim, the entropy gate, **Aligned-Unit alignment**, the keydb
unit-key parse/lookup, and `decrypt_aligned_unit` — end to end on real bus traffic.

**Key correction to §11.1/§11.4.4 layering:** a `READ(10)` over LibreDrive on a
**non-BEE** disc is **content-encrypted only — NOT bus-encrypted**. `content_decrypt`
alone yields 32/32 (and the byte match above); no `read_data_key` is involved.
§11.4.6 extends this to **BEE/UHD**.

### 11.4.6 BEE/UHD content via LibreDrive is also raw — ✅ [Disc] (Turbo UHD)

After the Turbo UHD disc (AACS 2.0, **BEE**) was cleaned so `makemkvcon` could read
it, an `SG_IO` capture of its LibreDrive reads settles the bus-encryption question
for the BEE case too. The disc's **Volume ID** appeared in the capture
(`30FFCAF2…`), pinning the keydb unit key unambiguously (`1B33F91C…`). Then:

- **Real UHD m2ts content decrypts with `content_decrypt` ALONE — 32/32 TS-sync,
  BDAV video PID `0x1011`, monotonic arrival timestamps, continuity counters
  30/30** (multiple Aligned Units at LBAs spread from 7.6 M to 20.9 M). Raw
  (undecrypted) units score 0–1/32. **This is the first real AACS 2.0 / UHD content
  Aligned Unit decrypted** (closes spec 05 §5.8 / spec 09's "last unproven step").
- **No bus layer.** Turbo's **unencrypted** JAR/BD-J files come back as
  **plaintext** over LibreDrive (captured bytes == MakeMKV's decrypted output,
  `C⊕P` all-zero) — i.e. LibreDrive returns **raw disc sectors**: encrypted where
  the disc is AACS-encrypted (m2ts), plaintext where it isn't (JARs).

**Conclusion (supersedes §11.4.3/§11.4.4 for the LibreDrive path):** bus encryption
is applied by the drive only on a *normal AACS-authenticated* transfer; **LibreDrive
bypasses that and returns raw**, so `LibreDriveReader` is **thin** for *all* discs,
BEE included — capture → align (to clip LBA, spec 12 §12.5) → `content_decrypt`.
`bus_decrypt_unit` remains correct-by-reference but is only relevant to
`AacsAuthReader` (spec 11 §11.3.3), which the project does not use. The methodology
trap that produced the earlier wrong "BEE READ(10) is bus-encrypted" reading is in
§11.4.4 / spec 12 §12.6.

### 11.4.7 The unlock is a static read-only replay — ✅ [Disc], MakeMKV removed

The decisive RE result: **the LibreDrive "unlock" is a fixed sequence of read-only
`READ BUFFER` (`0x3C`, mode 2, buffer-id `0x77`) commands** — freeblue replays it
itself, no MakeMKV. Established by:

1. **No writes.** A bidirectional `SG_IO` shim (capturing TO_DEV *and* FROM_DEV)
   over a MakeMKV session recorded **zero** `WRITE BUFFER`/`SEND KEY` commands — the
   whole unlock is reads.
2. **Static.** The `0x3C` CDB sequence is **byte-identical across two independent
   sessions** (521/521 commands); only a few drive-state *response* bytes differ
   (a counter), and MakeMKV doesn't compute on them — so there is **no
   challenge-response**. The sequence: a 64-byte handshake read (returns ASCII
   `MMkv…LbDr`) + 518 small drive-RAM scrape reads at fixed offsets (`0x120100`,
   `0x140000…0x14ff00`), in two passes.
3. **Replay works.** On a **cold** drive (a fresh eject/reload, where content reads
   are bus-encrypted and decrypt 2/32), replaying the 521 commands via `SG_IO`
   flips the drive to raw mode: the next `READ(10)` decrypts **32/32**.

**Implemented + verified end-to-end with no MakeMKV.** `freeblue-read` now ships a
minimal `SG_IO` layer (`scsi.rs`), the captured unlock table
(`libredrive_unlock.rs`, the WH16NS60 sequence — SCSI CDBs are protocol facts, spec
§11.6), and `libredrive.rs` (replay + clip-aligned `READ(10)` streaming). The
`raw_read` example, run as root on the **cold** TURBO UHD disc, read 8 consecutive
Aligned Units that decrypt **8/8 at 32/32** (video PID `0x1011` + audio) — freeblue
opening, unlocking, reading, and decrypting a real AACS 2.0 disc entirely on its
own.

**Open (`[?]`):** the unlock table is the **WH16NS60 / LibreDrive v06.3** sequence;
other drives/firmware likely need their own captured table (spec 12 §12.2). And the
clip **extent** (`start_lba`, `num_units`) is currently supplied by the caller —
resolving it from UDF/BDMV is the remaining glue (spec 12 §12.5).

## 11.5 BEE detection (which path to pick)

**Scope (post-§11.4.6):** BEE detection is **not** needed to choose the
*LibreDrive* path — LibreDrive returns raw for BEE and non-BEE alike, so the same
thin capture→`content_decrypt` works either way. It still matters for one thing:
making **`PlainUdfReader` refuse a BEE disc** (a plain read of a BEE disc *is*
bus-encrypted, §11.1) instead of emitting garbage. So treat the below as the
`PlainUdfReader` safety guard, not a LibreDrive router.

`freeblue-core` can know a disc's BEE status **before** reading content:
- **From the keydb** (cheap, available now): entries are tagged `…/BEE/…`
  (spec 06 §6.5). If the disc is in the keydb, its BEE status is known.
- **From the disc** (`[?]`): the BEE flag should live in the Unit Key File / the
  CPS-unit CCI (`CPSUnitNNNNN.cci`) and/or content-cert attributes — lift the
  exact bit from `libaacs` (`mmc.c`) + the AACS spec.
- **Empirically** (backstop): a `PlainUdfReader` unit that fails the TS-sync smoke
  test (spec 05 §5.7) on a disc with otherwise-correct keys ⇒ bus-encrypted.

## 11.6 Legal posture

Reverse-engineering the LibreDrive/drive command set is **interoperability RE**
(spec 10 §10.3) — documenting a drive-protocol so Free software can read media the
user owns, the same category as `libaacs`/`libbluray`. SCSI CDBs and protocol
steps are facts, not copyrightable expression (spec 07 §7.6). No MakeMKV code is
read or copied; we observe the *commands on the bus*, then write our own. The work
involves a real optical drive and discs the user owns; no keys or media are
committed (spec 10 §10.4).

## 11.7 Open questions

- **[?]** The LibreDrive unlock handshake + raw-read CDBs (§11.4) — the core RE.
- **[?]** Whether the LG WH16NS60 / iHBS212 need a specific (downgraded) firmware,
  or LibreDrive v06.3 works as-is (it engaged in MakeMKV here, so likely as-is).
- **[?]** Exact on-disc BEE flag location (§11.5) for drive-independent detection.
- **[?]** Whether a `AacsAuthReader` host cert is ever obtainable (spec 06 §6.6);
  if not, `LibreDriveReader` is the only viable BEE/UHD route.
