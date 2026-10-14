# freeblue — overview

Background and design rationale. For what works today, how to build it and how
to use it, see the [README](README.md).

## Why it exists

The FLOSS optical-disc stack stops at Ultra HD Blu-ray. `libdvdcss` handles
DVD, and `libaacs` / `libbdplus` handle ordinary Blu-ray, but `libaacs` has no
AACS 2.0 support. Until 2023 the AACS 2.0 player keys existed only inside
Intel SGX enclaves, and the AACS 2.0 specification itself is under NDA.

freeblue set out to write a FLOSS, clean-room specification of AACS 2.0
decryption, precise enough to implement from, and then to implement it in
original Rust. It is a sibling of the `rdd` (rippidydoodah) remuxer, which
remuxes already-decrypted Blu-ray streams and could not handle UHD discs on its
own.

## The thesis

AACS 2.0 is, cryptographically, AACS v1 with SHA-1 replaced by SHA-256 and the
v1 elliptic curve replaced by NIST P-256, with the player's keys moved into an
SGX enclave ([Talk 49:43]). The SGX wrapper, not the cipher, was the barrier.
The SGX.fail researchers broke it and published their results in the 37c3
talk "AACSess" and the SGX.Fail paper. freeblue does not touch SGX or extract
keys. It documents and implements the v1-shaped math that consumes key
material:

```
processing key ─[MKB]──────────────────► Media Key
Media Key ──────[AES-G with Volume ID]─► Volume Unique Key
Volume Unique Key ─[AES-128 decrypt]───► CPS Unit Key
CPS Unit Key ───[per-unit AES-128-CBC]─► plaintext M2TS (6144-byte Aligned Units)
```

## How correctness is established

Every claim in the specs carries a confidence tag: `[E]` established from a
public source, `[R]` reported in the talk, `[Disc]` reproduced byte for byte on
a real disc, `[?]` open (spec 00 §0.6). A tag is only raised with the citation
or byte match that earns it.

The original plan was to pin unknowns by comparing against MakeMKV's output and,
if necessary, static analysis. In practice the decryption core was settled by
the public AACS books, the `libaacs` source as a read-only reference, and
byte matches on real discs; static analysis was not needed for it (spec 07
§7.7). MakeMKV's decrypted output remained useful as an independent oracle for
the v1 content byte match (spec 11 §11.4.5).

The read path was the last piece. On a LibreDrive-capable drive, a static
read-only command sequence switches the drive to returning raw sectors, without
the bus-encryption layer, and freeblue replays that sequence itself (spec 11
§11.4.6–§11.4.7).

## Where to read next

- [`specs/README.md`](specs/README.md): index and reading order.
- Core path: [02 key hierarchy](specs/02-key-hierarchy-and-crypto-primitives.md)
  → [03 MKB and revocation](specs/03-media-key-block-and-revocation.md)
  → [04 on-disc structures](specs/04-on-disc-structures.md)
  → [05 content decryption](specs/05-content-decryption-pipeline.md).
- Read path: [11 read path and bus encryption](specs/11-read-path-and-bus-encryption.md).
- Open items: [12 known issues and deferred work](specs/12-known-issues-and-deferred-work.md).
- Legal posture: [10 legal and licensing](specs/10-legal-and-licensing.md).
- Reference material the specs cite: [`res/README.md`](res/README.md).
