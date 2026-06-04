# freeblue

A clean-room, **all-original Rust** library that decrypts **AACS 2.0** (Ultra HD
/ 4K Blu-ray) — the gap the FLOSS stack (`libaacs`/`libbluray`) leaves open. It
turns an encrypted UHD/BD volume plus user-supplied key material into **plaintext
M2TS**, so a remuxer like [`rippidydoodah`](../rippidydoodah/) can produce a
playable `.mkv`.

> **Status:** the **decrypt core + key hierarchy are implemented and byte-verified
> against MakeMKV on real discs**, including end-to-end through a **live SCSI
> capture** (decrypted output is byte-identical to MakeMKV). The library is real
> code (7 crates, 35 passing tests), not scaffolding. The one piece **not** yet
> live-verified is the **bus-encryption (BEE) layer** that UHD discs add — its
> algorithm is implemented and KAT-tested but awaits a real BEE-content capture.
> See [`specs/README.md`](specs/README.md) and the known-issues register in
> [`specs/12-known-issues-and-deferred-work.md`](specs/12-known-issues-and-deferred-work.md).

## The pipeline (every step proven byte-for-byte against a real disc)

```
processing key ──[MKB 0x04/0x05 + verify]──► Media Key   ✅ derived == keydb M        (GoT, BD)
Media Key      ──[Kvu = AES-G(Km, IDv)]────► VUK         ✅ AES-G(M,I)==V              (GoT + UHD)
VUK            ──[Kcu = AES-128D(Kvu, ·)]──► Unit Key     ✅ in Unit_Key_RO.inf         (UHD)
Unit Key+seed  ──[AES-128E⊕seed, AES-CBC]──► plaintext    ✅ byte-identical to MakeMKV  (GoT, live SCSI capture)

[BEE/UHD only] drive bus-encrypts the transfer; strip it first:
bus-enc unit   ──[AES-128-CBC, read_data_key]──► content  🚧 algorithm impl + KAT'd; not live-verified
```

AACS 2.0 is, cryptographically, AACS v1 with SHA-256/P-256 and the player keys
formerly hidden in SGX. That thesis is proven at the byte level: real GoT disc
content captured live off the SCSI bus decrypts to output **byte-identical to
MakeMKV** (6112/6144 bytes — the only delta is the per-packet TP_extra_header copy
bit MakeMKV clears). The remaining gap is the **bus-encryption layer on BEE/UHD
discs** (spec 11), implemented but not yet checked against a real BEE capture. See
[`specs/`](specs/) for the cited derivation of every step.

## Layout

```
specs/          The specification series (00–12) — the source of truth.
crates/
  freeblue-crypto    AES-128, AES-G, AES-G3                  (✅ implemented + KATs)
  freeblue-mkb       MKB parse + media-key derivation        (✅ implemented + KATs)
  freeblue-content   Aligned-Unit decrypt + bus_decrypt_unit (✅ implemented + KATs)
  freeblue-keys      KEYDB.cfg parser                        (✅ implemented + tests)
  freeblue-disc      /AACS/ structures + raw m2ts read       (✅ implemented + tests)
  freeblue-read      UnitReader trait + PlainUdfReader       (✅ impl; LibreDrive/AacsAuth stubs)
  freeblue-core      orchestration → plaintext M2TS          (✅ implemented + tests)
  freeblue-cli       `freeblue` binary                       (🚧 stub)
res/            Reference material (git-ignored) — see res/README.md.
fixtures/       Local KAT fixtures w/ real keys (git-ignored) — see fixtures/README.md.
overview.md     One-page project intro.
```

## Build & test

Requires Rust 1.75 (pinned in `rust-toolchain.toml`; `cargo` is rustup-managed at
`~/.cargo/bin` — ensure it's on `PATH`):

```sh
cargo build
cargo test            # unit KATs (deterministic, no secrets) — 35 pass, 4 ignored
cargo test -- --ignored   # KATs needing $FREEBLUE_FIXTURES (real disc data)
```

The implemented crates carry deterministic KATs (FIPS-197 AES, AES-G/AES-G3
vectors, the content block-key vector, the bus-decrypt round-trip) that encode the
`[Disc]`-verified algorithms and pass on `cargo test`. The `#[ignore]`d KATs are
the real-disc byte-matches; they load encrypted units + keys from
`$FREEBLUE_FIXTURES` and never ship in the repo (Rule 4).

## Working agreement & legal

Read [`CLAUDE.md`](CLAUDE.md) before contributing: **TDD-always**, **spec-first**,
**no keys/media in the repo, ever**. `freeblue` performs decryption for
interoperability with discs you own; it ships **no keys** and is built clean-room
from public AACS specs + standards, never from MakeMKV/CyberLink source. Full
posture: [`specs/10-legal-and-licensing.md`](specs/10-legal-and-licensing.md).
