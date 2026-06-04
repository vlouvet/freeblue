# 11 — Read Path and Bus Encryption

> **Status:** 📋 Design — the layer *between the optical drive and the decrypt
> core*. Specs 02–05 assume their input is already AACS-content-encrypted bytes;
> this spec covers how to actually obtain those bytes from a real drive, which is
> non-trivial for **bus-encryption (BEE)** discs — the verified blocker for live
> UHD ripping (spec 04 §4.3.2). Confidence tags per spec 00 §0.6.

## 11.1 The problem (recap, ✅ [Disc])

The decrypt core is proven (spec 09 §9.10.1), but it can only run if its input is
**AACS-content-encrypted** M2TS — the bytes as encrypted on the disc. For
**non-BEE** discs a plain UDF/file read yields exactly that (GoT: 32/32, spec
§5.3.1). For **BEE** discs the drive applies a second encryption layer (the **bus
key**, negotiated in AACS drive↔host auth) to the data it returns, so a plain read
yields **bus-encrypted** bytes the content cipher can't remove.

Two findings pin this, both **[Disc]**-verified:
1. **Turbo (BEE) fails a plain read** — raw 1/32 TS-sync, freeblue decrypt 1-2/32,
   with the key hierarchy still verifying (`AES-G(Km,IDv)==Kvu`). (spec 04 §4.3.2)
2. **A LibreDrive-capable drive does not help a plain read** — in the LG WH16NS60,
   MakeMKV reported "Using LibreDrive mode" yet a separate `mount`+`dd` still
   returned bus-encrypted data. LibreDrive is a *vendor-SCSI command path MakeMKV
   speaks*, not a drive state ordinary `READ(10/12)` inherit. (spec 04 §4.3.2)

**The UHD corpus disc is also BEE** (spec 04 §4.3.2) — so this is on the critical
path for the project's actual goal, not a v1 footnote.

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

### 11.3.2 `LibreDriveReader` — the pragmatic route for BEE/UHD

Speak MakeMKV's LibreDrive vendor-SCSI command path to a compatible drive,
returning on-disc (non-bus) content. This is what makes live BEE/UHD ripping work
and is the **primary new work item**. A drive-firmware/SCSI-protocol dependency,
**not crypto**. Requires reverse-engineering the command set (§11.4) and a
compatible/unlocked drive (the LG WH16NS60 and Lite-On iHBS212 here both report
LibreDrive v06.3).

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

**Next RE step (`[?]`):** the CDB trace gives commands but **not data**. Capture
the READ BUFFER *responses* and the READ(10) *data* (via `SG_IO` buffer capture /
`strace -e` with buffer dump, or a USB/SATA analyzer) and check: does READ(10)
data == on-disc AACS-content-encrypted bytes (→ `LibreDriveReader` = "0x3C unlock
+ plain READ(10)") or == bus-encrypted (→ we must replicate the bus-key removal
using the `0x3C`-extracted secret). That decides how thin/thick `LibreDriveReader`
can be.

## 11.5 BEE detection (which path to pick)

`freeblue-core` must know whether a disc needs the bus-encrypted path **before**
reading content, to choose a backend and to make `PlainUdfReader` refuse safely:
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
