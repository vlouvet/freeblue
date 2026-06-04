# 08 — Reference Implementation

> **Status:** ✅ Built out (2026-06-04) — standalone Rust workspace, **36 passing
> tests**. All eight crates are implemented, including `freeblue-read`'s
> `LibreDriveReader` (SG_IO LibreDrive unlock, no MakeMKV) and `freeblue-cli`
> (`unlock`/`decrypt`/`decrypt-disc`). **End-to-end standalone rip verified**: a
> cold TURBO UHD disc → `freeblue unlock` → mount → `freeblue decrypt` = 100%
> TS-sync plaintext, MakeMKV nowhere (spec 11 §11.4.6–7). v1 output is
> byte-identical to MakeMKV (spec 11 §11.4.5). Decisions inherit `rippidydoodah`'s
> stack so the two projects compose. Open polish in spec 12 (auto unit-key,
> per-drive unlock tables, `verify`).

## 8.1 Goals for the implementation

1. **Faithful to the spec.** Code implements specs 02–05; spec and code change
   together (parent Rule 2). The spec's `[?]`/`[E]` tags map to test status.
2. **A library first, CLI second.** A reusable `freeblue` core that
   `rippidydoodah` can call as a decryption backend (spec 00 §0.5), plus a thin
   CLI for standalone use and for driving the byte-match harness (spec 09).
3. **Clean-room.** Built from this spec + FLOSS `libaacs` + standards, never from
   MakeMKV/CyberLink source (spec 07 §7.6, spec 10).
4. **Drop-in with the FLOSS stack.** Interoperate with `KEYDB.cfg` (spec 06 §6.5)
   and emit M2TS that `rdd`'s demux ingests unchanged (spec 05 §5.4).

## 8.2 Language and crates

**Rust**, matching `rippidydoodah` (rdd spec 01) so the projects share toolchain,
style, and the eventual library boundary. Candidate dependencies (pin versions in
`Cargo.toml` and in this spec; parent anti-hallucination rule #5):

| Need | Crate(s) | Notes |
|---|---|---|
| AES-128 E/D, AES-NI | `aes`, `cipher` | basis for AES-G/AES-G3 and CBC content decrypt (spec 05) |
| CBC mode | `cbc` | content units (spec 05 §5.4) |
| SHA-256 | `sha2` | cert/MKB hashing (spec 02 §2.3.4) |
| P-256 ECDSA | `p256`, `ecdsa` | drive↔host auth, cert verify (spec 02 §2.3.5) |
| Key zeroization | `zeroize` | all `[u8;16]` key material (spec 02 §2.7) |
| Disc/UDF/BDMV read | `libbluray`/`libudfread` FFI, or rdd's disc layer | reuse, don't reinvent (spec 04 §4.2) |
| Hex / config | `hex`, `serde` | KEYDB + device-key-set parse (spec 06 §6.5) |

> **AES-G is not a library function** — it is `AES-128D(k,d) ⊕ d` (spec 02
> §2.3.2). Implement it once, KAT it first (spec 02 §2.7), and build AES-G3, the
> SD-tree walk (spec 03), and content decrypt (spec 05) on top.

## 8.3 Module layout (proposed)

```
freeblue/                     (standalone workspace; scaffolded 2026-06-04)
  crates/
    freeblue-crypto/   ✅ primitives: aes_128e/d, aes_g, aes_g3 → spec 02 §2.3.
                          Implemented + KATs pass. The KAT-first foundation.
    freeblue-mkb/      ✅ MKB TLV parse + processing-key→media-key → spec 03 §3.4.
                          Implemented + KATs pass. Depends on -crypto.
    freeblue-content/  ✅ aligned-unit block-key + AES-CBC → spec 05 §5.3, PLUS
                          bus_decrypt_unit (BEE layer, spec 11 §11.4.3). KATs pass;
                          decrypt_aligned_unit byte-matches MakeMKV (spec 11 §11.4.5).
    freeblue-keys/     ✅ KEYDB.cfg parser (disc D/M/I/V/U + DK/PK/HC), zeroized
                          → spec 06 §6.5. Implemented; tests pass incl. against the
                          real 182k-entry keydb. (device-key-set file: TODO.)
    freeblue-disc/     ✅ on-disc structures (MKB/UnitKeyFile/certs) + raw m2ts via
                          Image/Folder source → spec 04. Implemented + tests.
    freeblue-read/     ✅/🚧 the READ-PATH layer (§8.5.1): UnitReader trait +
                          PlainUdfReader (non-BEE, works). LibreDriveReader /
                          AacsAuthReader are stubs — the live BEE/UHD last mile
                          (spec 11, spec 12 §12.2).
    freeblue-core/     ✅ orchestration: resolve_unit_key (keydb-or-unwrap) +
                          decrypt_units + decrypt_clip over a freeblue-read::
                          UnitReader → §0.4. Implemented + tests. (BEE read routing
                          pending the read backend.)
    freeblue-cli/      ✅ `freeblue unlock` (LibreDrive raw mode), `decrypt`
                          (PlainUdfReader→plaintext), `decrypt-disc` (SG_IO extent).
                          Standalone rip verified. `verify` (§9.3) + auto unit-key TODO.
```

Mapping is 1:1 with the spec series on purpose: a failing test in `-mkb` points
at spec 03; a byte mismatch in `-content` points at spec 05 §5.3. The
decryption-core crates (`-crypto`, `-mkb`, `-content`, `-keys`, `-disc`, `-core`)
are **implemented and `[Disc]`-verified** (spec 09 §9.10.1, spec 11 §11.4.5). The
remaining work is the **BEE/UHD read backend** in `-read` (`LibreDriveReader`) and
the `-cli` binary — see spec 12 for the open register.

## 8.4 Public API sketch (forward of CLI)

```rust
// freeblue-core — the §0.4 contract as code (illustrative; pin signatures by test)
pub struct Disc { /* opened via freeblue-disc */ }
pub struct KeySet { /* device keys / processing key / host cert, zeroized */ }

impl Disc {
    pub fn open(source: DiscSource) -> Result<Disc>;          // image | folder | drive (spec 04 §4.6)
    pub fn volume_id(&self) -> Result<[u8; 16]>;              // may require drive auth (spec 04 §4.3)
    pub fn mkb(&self) -> Result<RawMkb>;                      // spec 04 §4.4
    pub fn unit_key_file(&self) -> Result<RawUnitKeyFile>;    // spec 04 §4.5
}

/// device keys + MKB → media key (spec 03); + Volume ID → Kvu (spec 02 §2.4.3)
pub fn derive_keys(disc: &Disc, keys: &KeySet) -> Result<TitleKeys>;

/// stream of plaintext 6144-B aligned units (spec 05) for a chosen clip/title
pub fn decrypt_clip(disc: &Disc, keys: &TitleKeys, clip: ClipId)
    -> Result<impl Iterator<Item = Result<AlignedUnit>>>;
```

Signatures are **illustrative**; per parent Rule 1, the real ones land
test-first. The `Result` error type must distinguish **"keys revoked by MKB"**
(spec 03 §3.6), **"Volume ID unavailable"** (spec 04 §4.3), and **"bus-encrypted
read (BEE)"** (spec 04 §4.3.2) from genuine bugs.

✅ **Implemented decomposition of `decrypt_clip`** (`freeblue-core`,
read-agnostic so it doesn't couple to the in-flux `freeblue-read` API):
`resolve_unit_key(keydb_unit_key, kvu, unit_key_file, cps_index)` picks the CPS
Unit Key — the keydb's unwrapped `U` when present, else `AES-128D(Kvu, enc)` over
the Unit Key File (spec 04 §4.5) — and `decrypt_units(unit_key, raw_units)` lazily
maps `freeblue-content::decrypt_aligned_unit` over the raw 6144-B units.
`decrypt_clip(reader, clip, unit_key)` ties them to a
`freeblue-read::UnitReader` (§8.5.1): it reads the clip's raw units and decrypts
each, lazily, surfacing a **BEE** disc as the opening `ReadError` (route those to
a LibreDrive/AACS reader) and per-unit failures as `ClipError` (read I/O vs.
cipher). `CoreError` separates **NoVolumeUniqueKey** and the disc-parse error from
a cipher bug. What remains for an end-to-end rip: the real **GoT byte-match**
(`$FREEBLUE_FIXTURES`), keydb→`resolve_unit_key` glue, and clip enumeration.

### 8.5.1 The read-path layer (`freeblue-read`) — required for BEE/UHD

Turbo proved (spec 04 §4.3.2) that a plain UDF read is **insufficient** for
bus-encryption (BEE) discs — and the UHD corpus disc is BEE. So `freeblue` needs
a read abstraction *separate from* the decrypt core, with pluggable backends:

```rust
/// Yields raw AACS-content-encrypted (NOT bus-encrypted) 6144-B units.
pub trait UnitReader {
    fn read_units(&mut self, clip: ClipId) -> Result<Box<dyn Iterator<Item = Vec<u8>>>>;
}
```

Backends, in increasing difficulty:
1. **`PlainUdfReader`** — a plain UDF/file read. Correct only for **non-BEE**
   discs (e.g. GoT) and for pre-decrypted-structure folder dumps. Easy; ships
   first; must *detect BEE and refuse* rather than emit garbage.
2. **`LibreDriveReader`** — issue the LibreDrive vendor SCSI reads (what MakeMKV
   uses) to a compatible/flashed drive, returning on-disc (non-bus) content. The
   pragmatic route for live BEE/UHD discs. A drive-firmware dependency, **not
   crypto** — reverse the command set from the LibreDrive-enabled read path.
3. **`AacsAuthReader`** — perform AACS drive↔host auth (AMAC) with an unrevoked
   host cert (spec 04 §4.3.1), negotiate the bus key, and un-bus-encrypt the
   transfer ourselves. Needs a usable host cert/key (spec 06 §6.6, hard `[?]`).

Keeping this behind `UnitReader` means the **decrypt core stays read-agnostic and
already-verified**; only the reader changes per disc. `freeblue-core` selects a
backend from a BEE flag it reads off the disc (Unit Key File / CCI, spec 04
§4.3.2 `[?]`).

## 8.5 Concurrency / performance

Aligned units are independent (spec 05 §5.1) → a worker pool decrypts spans in
parallel, AES-NI-backed, with a bounded channel back to an ordered writer so
output stays in disc order. Goal: stay I/O-bound on the drive (spec 05 §5.6,
rdd spec 07). **No perf numbers asserted until measured** (spec 09 §9.5; parent
rule #6).

## 8.6 Relationship to libaacs

**Decision (settled):** `freeblue` is a **standalone, all-original Rust library**
— *not* a patch to `libaacs`. `libaacs` is used here strictly as a **clean-room
reference oracle** (to confirm the v1 algorithms byte-for-byte, spec 03 §3.4) and
nothing is copied from it. Rationale: this codebase is independently authored and
not gated on upstream review/acceptance, and a from-scratch Rust implementation
is easier to test-drive (spec 09) and to keep provably original (spec 10 §10.2).

`freeblue` still **interoperates** with the existing FLOSS ecosystem by reading
the standard `KEYDB.cfg` format (spec 06 §6.5) and emitting M2TS that `rdd`'s
demux ingests unchanged (spec 05 §5.4). The `libaacs` maintainers are welcome to
read this library and its spec as a reference for adding AACS 2.0 to their C code
— that is a *downstream benefit*, not a dependency or a goal of this project.

## 8.7 Self-checks built into the binary

- Per-unit **TS `0x47`-cadence smoke test** (spec 05 §5.7) behind a debug flag —
  turns silent corruption loud.
- **AES-G / AES-G3 / SD-tree KATs** run in `cargo test` (spec 09 §9.2).
- A `freeblue verify <disc>` subcommand that runs the spec-09 byte-match against
  a reference rip path, for regression and for closing `[?]`s (spec 07 §7.4).

## 8.8 Open questions

- **Settled:** standalone all-original Rust library, **pure-Rust** crypto (no
  `libaacs` FFI), `libaacs` as reference-only (§8.6). `rdd` consumes `freeblue`
  as an external crate dependency (Phase 3, README roadmap).
- **[?]** First in-repo TDD task: the **AES-G3 SD-tree walk** (device key →
  processing key, spec 03 §3.4.3) — every primitive is verified; only the full
  descent is unrun. The `PK` path already reaches Km, so this is for completeness
  + non-keydb discs.
- **[?]** `rdd` consumption shape: external crate vs. git submodule vs. vendored
  (Phase 3; spec 00 §0.5).
- **[?]** The `freeblue-read` backend for BEE/UHD (§8.5.1) — the live-disc last
  mile. `LibreDriveReader` (reverse the vendor SCSI read commands) is the likely
  first viable route; `AacsAuthReader` needs an unrevoked host cert. This is the
  highest-value *new* work item, distinct from the (verified) decrypt core.
