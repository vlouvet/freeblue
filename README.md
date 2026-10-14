# freeblue

A clean-room, all-original Rust library and command-line tool that decrypts
**AACS 2.0** (Ultra HD / 4K Blu-ray) content into **plaintext M2TS**. It covers
the step the FLOSS Blu-ray stack (`libaacs` / `libbluray`) does not: `libaacs`
implements AACS v1 only.

freeblue takes an encrypted disc (or a copy of its files) plus key material you
supply, and produces the decrypted transport stream. Turning that stream into a
playable `.mkv` is the job of a remuxer such as `rdd`
([see below](#relationship-to-rdd)).

Background and design rationale: [`overview.md`](overview.md). The
specification series in [`specs/`](specs/README.md) is the source of truth.

## What it is, and what it is not

It is:

- a Rust workspace of small crates that implement the AACS key ladder and
  Aligned-Unit content decryption, each written from public AACS
  specifications, NIST/RFC standards and published research;
- a `freeblue` CLI that can read a protected disc directly from a
  LibreDrive-capable drive on Linux and write plaintext M2TS;
- a written, cited specification of AACS 2.0 decryption (`specs/`).

It is not:

- a key source. It ships no keys of any kind; you supply them at runtime.
- a ripper, remuxer or player. Output is plaintext M2TS, nothing more.
- a tool for extracting keys from players, attacking SGX, or defeating AACS 2.1
  traitor tracing, BD+, BD-J or Cinavia (spec 00 §0.3).
- a firmware tool. It never writes to a drive or flashes firmware.

## Status

Verified on real discs (details and evidence in the cited specs):

| Step | Result | Where |
|------|--------|-------|
| Processing key + MKB → Media Key | derived key matches the key database, BD (MKBv63) | spec 03 §3.4.2, spec 09 §9.10.1 |
| Media Key + Volume ID → Volume Unique Key | matches the key database, BD and UHD (MKBv82) | spec 09 §9.10.1 |
| Volume Unique Key → CPS Unit Key | wrapped key matches `Unit_Key_RO.inf`, UHD | spec 04 §4.5, spec 09 §9.10.1 |
| Content decrypt, AACS v1 (non-BEE BD) | output equals MakeMKV's decrypted m2ts in 6112 of every 6144 bytes; the rest is one header byte per packet (see [Limitations](#limitations)) | spec 11 §11.4.5 |
| Content decrypt, AACS 2.0 (UHD) | valid BDAV transport stream: TS sync, video PID `0x1011`, monotonic arrival timestamps, continuity counters | spec 11 §11.4.6 |
| Standalone read, no MakeMKV | cold drive → `freeblue unlock` → mount → `freeblue decrypt` of a UHD main title: 1024 units, 100% TS sync | spec 12 §12.10, §12.13 |

What works today:

- **Standalone disc reading.** On a supported drive, freeblue replays a static,
  read-only LibreDrive unlock sequence (`READ BUFFER` commands over Linux
  `SG_IO`). After that the drive returns raw, content-encrypted sectors with no
  bus-encryption layer, so the disc can be mounted normally and its `m2ts` files
  decrypted. MakeMKV is not involved at any point (spec 11 §11.4.6–§11.4.7).
- **CLI:** `freeblue unlock`, `freeblue decrypt`, `freeblue decrypt-disc`.
- **Library:** `KEYDB.cfg` parsing, MKB media-key derivation from a processing
  key, Volume Unique Key and Unit Key derivation, streaming Aligned-Unit
  decryption, and reading a Volume ID produced by another tool (MakeMKV's
  `discatt.dat`, or the libaacs `vid` cache; spec 06 §6.8). The CLI does not
  use the last two yet.

## Limitations

These are open items, tracked in
[spec 12](specs/12-known-issues-and-deferred-work.md):

- **One drive model.** Only one unlock table has been captured, for the LG
  WH16NS60 (spec 12 §12.14). Other LibreDrive-capable drives are untested and
  likely need their own table. A drive that fails the LibreDrive handshake is
  refused. Without the unlock, a UHD disc returns bus-encrypted data that does
  not decrypt.
- **The unit key is passed by hand.** The CLI needs `--unit-key`. Automatic
  resolution from the disc's Volume ID is not implemented (spec 12 §12.15).
  freeblue cannot read the Volume ID itself: the drive releases it only after
  AACS drive–host authentication, and the tested drive rejects the only publicly
  available host certificate as revoked, with or without the LibreDrive unlock.
  A Volume ID produced by another tool can be read through the library.
- **Single CPS unit only.** Every disc tested has one CPS unit. Mapping clips
  to keys on multi-unit discs is unimplemented (spec 12 §12.8).
- **No device-key path.** There is no parser for a device-key-set file, and the
  subset-difference walk from device keys to a processing key has not been run
  end to end (spec 12 §12.7). Decryption needs a unit key, a Volume Unique
  Key, or a processing key plus the disc's Volume ID, all from key material you
  provide; the CLI accepts only a unit key.
- **Copy-permission bits.** freeblue passes byte 0 of each 192-byte packet (the
  TP_extra_header copy-permission indicator) through as it is on the disc;
  MakeMKV clears its top two bits. Payload is identical. Whether to mask them is
  undecided (spec 12 §12.4).
- **Linux only for drive access.** `unlock` and `decrypt-disc` use `SG_IO`.
  `decrypt` (file input) has no OS-specific code.
- **No title selection.** You pick the `m2ts` (or the LBA extent) to decrypt.
  There is no playlist parsing and no `verify` subcommand yet.

## Build

Requirements:

- Rust 1.75.0, pinned in `rust-toolchain.toml` (rustup installs it on the
  first `cargo` call, with `rustfmt` and `clippy`).
- No C libraries. The only platform dependency is the `libc` crate, used on
  Linux for `SG_IO`.

```sh
cargo build --release          # binary: target/release/freeblue
cargo test                     # synthetic known-answer tests, no secrets needed
```

Privileges: `unlock` and `decrypt-disc` send SCSI commands through `SG_IO` on
the drive's device node (opened read-only). They have been tested as root
(`sudo`). Whether a non-root user with read access to the device is enough
depends on the kernel's SG_IO command filtering and has not been checked.
Mounting the disc needs root as usual. `decrypt` on a file needs only read
access to that file.

## Usage

Running `freeblue` with no arguments, `--help`, or an unknown subcommand
prints this usage text and exits with status 2:

```text
freeblue unlock <device>
freeblue decrypt <m2ts> --unit-key <hex32> -o <out.m2ts>
freeblue decrypt-disc <device> --start-lba N --num-units M --unit-key <hex32> -o <out>
```

`--unit-key` is the 16-byte CPS unit key for the disc, as 32 hex digits
(an optional `0x` prefix is accepted). You supply it; freeblue does not
include or fetch keys.

Decrypt a title from a mounted disc:

```sh
# 1. Switch the drive to raw-read mode (read-only LibreDrive unlock).
sudo freeblue unlock /dev/sr0

# 2. Mount the disc read-only and decrypt one clip.
sudo mount -t udf -o ro /dev/sr0 /mnt/disc
freeblue decrypt /mnt/disc/BDMV/STREAM/00002.m2ts \
    --unit-key "$UNIT_KEY" -o title.m2ts
```

Or read an extent straight from the drive without mounting. `decrypt-disc`
runs the unlock itself, then reads `M` Aligned Units (3 sectors each) starting
at LBA `N`. `N` must be on an Aligned-Unit boundary, such as the first sector
of an `m2ts` clip:

```sh
sudo freeblue decrypt-disc /dev/sr0 --start-lba N --num-units M \
    --unit-key "$UNIT_KEY" -o title.m2ts
```

`decrypt` accepts any content-encrypted `m2ts` file, for example one copied
from the disc while the drive was unlocked.

When it finishes, freeblue prints the number of units written and a TS-sync
score for the first unit (`32/32` means every packet header lined up). A low
score means the wrong key or still-bus-encrypted input. A high score alone is
not proof of a correct decrypt (spec 12 §12.6).

## Crates

```
crates/
  freeblue-crypto    AES-128, AES-G, AES-G3
  freeblue-mkb       MKB parsing, processing key → media key
  freeblue-content   Aligned-Unit content decryption; bus_decrypt_unit (unused by the read path)
  freeblue-keys      KEYDB.cfg parser; disc ID; external Volume ID (discatt.dat, libaacs cache)
  freeblue-disc      /AACS/ structures from a folder copy; encrypted unit keys
  freeblue-read      UnitReader trait; PlainUdfReader (files); LibreDriveReader (SG_IO, Linux)
  freeblue-core      unit-key resolution and streaming decrypt_clip → plaintext M2TS
  freeblue-cli       the `freeblue` binary
```

The API and module design are in [spec 08](specs/08-reference-implementation.md).

## Tests

`cargo test` runs deterministic known-answer tests built from public vectors
(FIPS-197 AES, AES-G/AES-G3, content block-key, a bus-decrypt round trip) and
synthetic data. They need no keys or media. CI runs `cargo fmt --check`,
`cargo clippy -D warnings` and `cargo test` on every push to `main`.

Tests that need real disc data are `#[ignore]`d and read files from the
directory named by `$FREEBLUE_FIXTURES`; see
[`fixtures/README.md`](fixtures/README.md). Run them with
`cargo test -- --ignored`. Some of them are still placeholders: the real-disc
results recorded in the specs were measured outside the test suite.

## Relationship to rdd

`rdd` (rippidydoodah) is a sibling project that remuxes decrypted Blu-ray
streams into Matroska. On its own it treats UHD / AACS 2.0 as out of scope.
rdd integrates freeblue as an optional decryption backend behind a `freeblue`
Cargo feature, off by default; with it enabled, rdd reads raw `m2ts` clips,
decrypts them through freeblue, and feeds the plaintext M2TS to its existing
demux and remux pipeline. rdd depends on this repository's `main` branch as a
git dependency.

## Specification

[`specs/README.md`](specs/README.md) indexes the series, gives a reading order,
and explains the confidence tags (`[E]` established, `[Disc]` verified on a
real disc, `[?]` open). Every behavioural change updates the relevant spec in
the same commit.

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first. The rules in short: test first, spec
first, and **no keys or media in the repository, ever**. freeblue is built
clean-room from public AACS specifications, NIST/RFC standards and published
37c3 research. `libaacs` may be read as a behavioural reference; no code is
copied from it or from any proprietary product.

## License

The code is GPL-3.0-or-later; the full text is in [`LICENSE`](LICENSE). A
separate license for the specification text (CC BY-SA 4.0) has been proposed
but not confirmed. Rationale:
[`specs/10-legal-and-licensing.md`](specs/10-legal-and-licensing.md) §10.5.

## Legal & disclaimer

`freeblue` is a Free Software **interoperability** library that decrypts AACS 2.0
(UHD/4K Blu-ray) so discs you own can be played and remuxed on the platforms of
your choice — the gap the FLOSS Blu-ray stack leaves open. It is research and
interoperability software, not a piracy tool.

- **No keys, tables, or media are included.** freeblue ships **no** device keys,
  processing keys, media keys, `KEYDB.cfg`, host certificates, Volume IDs, unit
  keys, or any copyrighted content — encrypted or decrypted. All key material is
  supplied by you at runtime and is never committed (CLAUDE.md Rule 4).
- **Clean-room originality.** Cryptographic constants and protocol facts come from
  public AACS specs, NIST/RFC standards, and published 37c3 research, each cited
  inline. Constants/protocol facts are not copyrightable; no third-party code is
  copied (spec 08 §8.6, spec 10 §10.2).
- **No new key extraction.** freeblue does not attack SGX or extract keys from
  any player; that work is published independent research which the specs cite
  (spec 10 §10.3).
- **Decryption / circumvention may be regulated where you live.** Bypassing copy
  protection on optical media is restricted or unlawful in some jurisdictions
  (e.g. the DMCA in the United States, the EUCD in parts of the EU), even for a
  disc you own. **You are solely responsible for ensuring your use complies with
  the laws that apply to you.**
- **No warranty.** Provided "as is", without warranty of any kind, to the extent
  permitted by the GPL-3.0 (see §15–17 of the [`LICENSE`](LICENSE)).

This is not legal advice. If you are unsure whether your intended use is lawful
where you are, consult a qualified lawyer in your jurisdiction. Full posture:
[`specs/10-legal-and-licensing.md`](specs/10-legal-and-licensing.md).
