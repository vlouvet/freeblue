# 14 — Virtual Decrypted Disc (userspace decrypt-on-read mount)

Status: 📋 **Design** (no code yet). Confidence tags per spec 00 §0.6:
`[E]` established · `[Disc]` verified on a real disc · `[?]` open.

> **One-line goal.** Wrap freeblue's proven decrypt core (spec 05, spec 11) in a
> **random-access, decrypt-on-read** surface so that *any* off-the-shelf tool
> — `ffmpeg`, a media player, `dd`, `rdd` — can read a UHD disc as if it were
> already decrypted, without that tool knowing anything about AACS.

This extends the spec 00 §0.4 contract from *"stream a clip's plaintext bytes"*
to *"expose the whole disc as a readable, decrypted thing"* — purely in
userspace, **no kernel driver**.

---

## 14.0 At-a-glance

| Piece | What | Risk | Phase |
|---|---|---|---|
| `decrypt_range` engine | random-access Aligned-Unit decrypt (offset+len → plaintext) over an existing `UnitReader` | low — pure logic on the verified core | 1 (MVP) |
| FUSE overlay | mount that decrypts `*.m2ts` on read, passes everything else through | low — userspace, read-only | 2 |
| NBD decrypted image | whole-disc decrypted block device (nbdkit/buse), mountable as UDF | medium — needs disc-wide extent map | 3 |
| Disc extent map | LBA/file → CPS unit → key, disc-wide | medium — builds on spec 04 §4.5 | 3 |

**MVP "sign of life" (Phase 1):** `decrypt_range` serves an arbitrary byte range
of a clip **byte-identical** to the existing sequential `decrypt_units`
(spec 09 oracle), proving random access is correct. No mount yet.

---

## 14.1 Motivation and scope

Today freeblue decrypts **sequentially, per clip**: `freeblue-core::decrypt_units`
takes a `UnitReader`'s forward iterator of 6144-byte Aligned Units and yields
plaintext units in order (spec 05 §5.4); `freeblue-cli decrypt-disc` does
unlock → raw `READ(10)` an extent → decrypt (spec 12 §12.10). That is a *pull a
whole clip start-to-finish* model.

A presentation layer needs the opposite shape — **random access**: a player
seeks, an `ffmpeg` probe reads the last MB then the first, a UDF driver reads a
directory block here and a file extent there. The transparent decrypt-on-read
pattern is the long-established way to serve protected optical media to unmodified
software — `libdvdcss` has done exactly this for CSS DVDs since 1999 `[E]` (open a
device, return decrypted sectors on read). freeblue already holds the AACS 2.0
crypto; this spec is the *plumbing* that turns "I can decrypt a clip" into
"mount the disc and use it."

**In scope:** a random-access decrypt engine; a FUSE filesystem surface; an NBD
block-device surface; the disc-wide encrypted-extent → key map; a `freeblue mount`
CLI.

**Out of scope:** anything past "plaintext bytes presented for reading" — no
re-mux/transcode (rdd/HandBrake), no write support, no new key acquisition
(spec 00 §0.3), no bus-layer work (retired, spec 12 §12.1).

## 14.2 Clean-room provenance (load-bearing — read before coding)

This design is built **only** from:

- freeblue's own verified decrypt core and read path (specs 04, 05, 11) and its
  existing public API (`UnitReader`, `decrypt_units`, `ALIGNED_UNIT_LEN`);
- the **publicly-obvious, pre-existing** "decrypt-on-read device shim" pattern,
  whose canonical FLOSS exemplar is `libdvdcss` (LGPL) for CSS `[E]`;
- the **public** nbdkit / NBD protocol and libfuse APIs;
- public MMC-5/6 and AACS books already in `res/` for any command/structure
  detail.

No proprietary decryptor's internals inform this spec. The architecture is
independently derivable and decades old; the AACS-specific behavior all traces to
freeblue's existing `[Disc]`-verified specs. (Per CLAUDE.md Rule 4 / spec 10:
keep any competitor-RE notes firewalled out of this work — they are neither
needed nor permitted as a source here.)

## 14.3 Architecture

```
                         ┌─────────────────────────────────────────┐
  unmodified consumer    │  ffmpeg / player / dd / rdd / mount -t udf │
  (knows nothing of AACS)└───────────────┬───────────────────────────┘
                                          │ ordinary reads (random offset+len)
              ┌───────────────────────────┴───────────────────────────┐
              │            Surface (one of):                            │
              │   A) FUSE filesystem  — decrypt *.m2ts on read          │  spec 14.5
              │   B) NBD block device — decrypted whole-disc UDF image  │  spec 14.6
              └───────────────────────────┬───────────────────────────┘
                                          │ read(decrypted_offset, len)
                            ┌──────────────┴──────────────┐
                            │  decrypt_range engine        │  spec 14.4  (NEW: freeblue-mount)
                            │  offset→Aligned Units→decrypt │
                            └──────────────┬──────────────┘
                       resolve key │        │ read raw units (random access)
              ┌────────────────────┘        └────────────────────┐
   freeblue-core::resolve_unit_key            freeblue-read::UnitReader
   (keydb U / Kvu unwrap, spec 04 §4.5)       (LibreDriveReader / PlainUdfReader, spec 11)
```

New crate **`freeblue-mount`**: the `decrypt_range` engine plus the two surfaces
behind cargo features (`fuse`, `nbd`). It depends on `freeblue-core`,
`freeblue-content`, `freeblue-read`, `freeblue-disc`. The verified crates are
untouched; only `freeblue-read` gains a small random-access capability (§14.4.2).

## 14.4 The random-access decrypt engine `[E]` design

### 14.4.1 Aligned-Unit math

The content cipher operates on **6144-byte Aligned Units** = 3 × 2048-byte
sectors (`ALIGNED_UNIT_LEN`, spec 05 §5.1). An Aligned Unit is the atomic unit of
decryption: you cannot decrypt a sub-range without the whole unit (CBC chains
within it; the 16-byte seed is at unit head, spec 05 §5.2).

To serve a decrypted read of `[off, off+len)` within a clip whose plaintext ==
ciphertext length:

```
first_unit = off / 6144
last_unit  = (off + len - 1) / 6144
for u in first_unit..=last_unit:
    raw   = reader.read_unit_at(u)         # 6144 ciphertext bytes
    plain = decrypt_aligned_unit(key, raw) # spec 05, verified
    copy the overlapping slice of `plain` into the output
```

Decrypted length == on-disc length (content decryption is length-preserving,
spec 05), so offsets map 1:1 between the ciphertext extent and the presented
plaintext. **This is the whole trick** — everything else is wiring.

### 14.4.2 Random-access read primitive (small `freeblue-read` addition)

`UnitReader` today is a forward iterator (`read_units → UnitIter`). Add an
**optional** seekable capability without disturbing it:

```rust
/// Random-access companion to `UnitReader`. Backends that can address a
/// specific unit (file-offset seek, or SG_IO READ(10) by LBA) implement this.
pub trait UnitReaderAt {
    /// Read Aligned Unit `index` (0-based within the clip's extent).
    fn read_unit_at(&mut self, clip: &ClipId, index: u64) -> Result<Unit, ReadError>;
}
```

- `PlainUdfReader` (file/mounted UDF): `seek(index * 6144)` + read 6144 `[E]`.
- `LibreDriveReader` (SG_IO): `READ(10)` at `extent_start_lba + index*3`, 3
  sectors `[Disc]` — the drive read is *already* LBA-addressed (spec 11 §11.4.7),
  so this is a natural fit, not new I/O.

Forward iteration stays the default for the existing sequential `decrypt-disc`
path; `UnitReaderAt` is what the mount surfaces use.

### 14.4.3 `DecryptingClip` and caching

`freeblue-mount` exposes:

```rust
pub struct DecryptingClip<R: UnitReaderAt> { reader: R, clip: ClipId, key: Block, len_units: u64, /* + small LRU unit cache */ }
impl<R: UnitReaderAt> DecryptingClip<R> {
    /// Decrypt `buf.len()` plaintext bytes starting at clip-relative `offset`.
    pub fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ...>;
}
```

A tiny LRU cache of recently-decrypted units makes the common
"sequential-ish reads with occasional seeks" pattern cheap (a 4 KB OS read that
straddles a unit boundary must not redecrypt twice). Cache holds **plaintext**
units; it is **in-memory only, zeroized on drop** (spec 10, Rule 4 — never spill
plaintext or keys to disk).

## 14.5 Surface A — FUSE overlay (the low-risk MVP mount)

The current pipeline already **unlocks the drive and OS-mounts the disc as UDF**
(spec 12 §12.13); the mounted `*.m2ts` files are *content-encrypted* (unlock only
strips the bus layer, not AACS content encryption). So the simplest mount is an
**overlay** over that read-only UDF mount:

- Enumerate the source tree (the OS UDF mount, or `freeblue-disc`'s reader).
- Pass every file through **unchanged** except `BDMV/STREAM/*.m2ts`.
- For each `*.m2ts`, back it with a `DecryptingClip` (key resolved per its CPS
  unit, spec 04 §4.5) so `read(offset,len)` returns plaintext on the fly.
- Read-only (`-o ro`), single-user, no writeback.

Result: `mount`-point looks like a normal decrypted BDMV; `ffmpeg -i
/mnt/freeblue/BDMV/STREAM/0000.m2ts …`, players, and rdd's existing demux all
work with no AACS awareness. This is the **headline user win** at the **lowest**
engineering risk (pure userspace, reuses the existing mount + verified decrypt).

Library: `fuser` (pure-Rust libfuse binding) behind the `fuse` feature.

## 14.6 Surface B — NBD decrypted disc image (higher value, more work)

Expose the **entire disc** as a decrypted block device, so a portable decrypted
`.iso`/UDF image is mountable anywhere, matching the block-level model most
directly:

- Implement an **NBD server** (an `nbdkit` plugin, or `buse`-style in-process)
  whose backing store is the unlocked drive (`LibreDriveReader` over SG_IO) or a
  raw image.
- On `pread(sector_range)`: if the range lies in an **encrypted CPS-unit extent**
  (§14.7), decrypt the covering Aligned Units and return plaintext; otherwise
  return the raw sectors **unchanged** (UDF metadata, `.mpls`/`.clpi`, padding
  are not content-encrypted, spec 04).
- The consumer does `nbd-client → /dev/nbdX → mount -t udf` and sees a fully
  decrypted disc.

This needs the disc-wide map (§14.7) and careful read-only semantics, hence
Phase 3.

## 14.7 The disc encrypted-extent map `[?]`

Both surfaces need: *given a location, is it encrypted, and with which key?*

- **FUSE (per-file)**: trivial — the file *is* the extent; one CPS unit per
  `.m2ts` for single-CPS-unit discs (the common case, spec 04 §4.5). Map
  `.m2ts → CPS index → unit key` via `Unit_Key_RO.inf` + `resolve_unit_key`.
- **NBD (whole-disc LBA)**: needs `LBA range → CPS unit` disc-wide. Source: the
  UDF file→extent table (via `libudfread`/`freeblue-disc`) cross-referenced with
  the CPS-unit assignment. Multi-CPS-unit selection is the open item already
  tracked in spec 12 §12.8 / spec 04 §4.5 `[?]`; the NBD surface inherits it.

Until §12.8 is closed, NBD targets **single-CPS-unit discs** and `log()`s a clear
refusal for multi-unit discs rather than guessing a key (Rule 1: no silent
wrong output).

## 14.8 CLI integration

Add to `freeblue-cli` (spec 12 §12.10 already has `unlock`/`decrypt`/`decrypt-disc`):

```
freeblue mount <source> <mountpoint> [--fuse|--nbd] [--unit-key <hex> | --keydb <cfg>]
```

- `<source>`: a device (`/dev/sr0`, triggers unlock + LibreDriveReader), a
  mounted UDF path, or a disc image/folder (PlainUdfReader).
- Resolves keys exactly as `decrypt-disc` does (keydb `U`, or `Kvu` unwrap).
- `--fuse` (default) mounts the overlay (§14.5); `--nbd` starts the server (§14.6).
- Clean unmount on SIGINT; zeroize keys/caches on exit.

## 14.9 Safety and security

- **Read-only, always.** No surface ever writes to the disc or accepts writes
  from the consumer (NBD exports read-only; FUSE denies write).
- **No plaintext/keys to disk.** The unit cache and key material are in-memory and
  zeroized (Rule 4, spec 10 §10.4). The mount presents plaintext only through the
  live FD/socket; nothing persists.
- **No keys in repo/tests.** Real-disc mount tests are `#[ignore]`d and load from
  `$FREEBLUE_FIXTURES` (Rule 4, spec 09 §9.6).
- **Local-only.** NBD binds to `127.0.0.1`/a Unix socket by default — never a
  network interface (a decrypted-disc NBD export on the LAN is a redistribution
  hazard).

## 14.10 Test plan (TDD — Rule 1, failing test first)

1. **Engine KAT (no I/O, no fixtures):** a `MockUnitReaderAt` returns synthetic
   "ciphertext" units that `decrypt_aligned_unit` maps to known plaintext (reuse
   the spec 09 content vector). Assert `read_at` for ranges that (a) sit inside
   one unit, (b) straddle a unit boundary, (c) cover the whole clip, all equal the
   ground-truth slice. **This is the MVP gate.**
2. **Equivalence:** `read_at` over the full length, concatenated, ==
   `decrypt_units` sequential output for the same input (ties the new path to the
   already-`[Disc]`-verified oracle, spec 09 §9.10.1).
3. **Cache correctness:** randomized read offsets/lengths produce identical bytes
   with the LRU on vs off (cache is transparent).
4. **FUSE (fixture-gated `#[ignore]`):** mount a fixture disc, `sha256` a
   `.m2ts` read through the mount == `decrypt-disc` output for that clip.
5. **NBD (fixture-gated):** `nbd-client` + `mount -t udf`, then byte-compare a
   read file vs the FUSE/`decrypt-disc` result.
6. **Pass-through (NBD):** an un-encrypted region (e.g. a `.mpls`) reads byte-
   identical to the raw disc (decryption must not touch it).

## 14.11 Phasing

- **Phase 1 — engine (MVP / sign of life).** `UnitReaderAt` + `DecryptingClip` +
  tests 1–3. No mount; proves random-access decrypt is byte-correct. *Smallest
  shippable, highest confidence.*
- **Phase 2 — FUSE overlay.** `freeblue mount --fuse` over the unlocked UDF mount;
  tests 4. The first "any tool reads the disc decrypted" milestone.
- **Phase 3 — NBD + disc map.** Whole-disc image, extent map, single-CPS-unit;
  tests 5–6. Higher value, gated on spec 12 §12.8 for multi-unit.

## 14.12 Open questions `[?]`

- **Multi-CPS-unit LBA map** for NBD — inherits spec 12 §12.8 / spec 04 §4.5.
- **Length/EOF semantics** when a clip's last Aligned Unit is partial (the
  sequential reader stops on a trailing partial unit — `freeblue-read` lib note);
  define how `read_at` reports the final boundary.
- **Hot-unmount / disc-change** behavior for the FUSE mount (eject mid-read).
- **`nbdkit` plugin vs in-process `buse`** — pick after a Phase-3 spike; nbdkit
  gives `nbdkit … | nbdfuse`/`qemu-nbd` interop for free, buse avoids the
  external dep.

## 14.13 Cross-references

- spec 00 §0.4 — the contract this extends (clip bytes → mountable disc).
- spec 04 §4.5 — CPS unit / `Unit_Key_RO.inf` → unit-key resolution.
- spec 05 — the verified Aligned-Unit content decrypt this engine calls.
- spec 11 §11.4.6–7 — LibreDrive unlock + raw `READ(10)`; the read backend.
- spec 12 §12.8 (multi-CPS-unit), §12.10 (CLI), §12.13 (unlock+mount) — prerequisites/relations.
- spec 10, CLAUDE.md Rule 4 — no keys/plaintext persisted; clean-room provenance.
