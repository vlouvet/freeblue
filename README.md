# freeblue

A clean-room, **all-original Rust** library that decrypts **AACS 2.0** (Ultra HD
/ 4K Blu-ray) — the gap the FLOSS stack (`libaacs`/`libbluray`) leaves open. It
turns an encrypted UHD/BD volume plus user-supplied key material into **plaintext
M2TS**, so a remuxer like [`rippidydoodah`](../rippidydoodah/) can produce a
playable `.mkv`.

> **Status:** **freeblue reads and decrypts a real UHD/AACS 2.0 disc on its own —
> no MakeMKV.** It replays the (static, read-only) LibreDrive unlock over `SG_IO`,
> reads raw sectors, and decrypts them: verified on the **cold** TURBO UHD disc,
> 8/8 Aligned Units at 32/32 TS-sync (video PID `0x1011`). v1 output is
> **byte-identical to MakeMKV**. Real code, 7 crates, 36 passing tests. Remaining
> work is glue: resolving a clip's on-disc extent from UDF/BDMV (today the caller
> passes it) and a CLI. See [`specs/README.md`](specs/README.md) and the
> known-issues register in
> [`specs/12-known-issues-and-deferred-work.md`](specs/12-known-issues-and-deferred-work.md).

## The pipeline (every step proven on a real disc)

```
processing key ──[MKB 0x04/0x05 + verify]──► Media Key   ✅ derived == keydb M        (GoT, BD)
Media Key      ──[Kvu = AES-G(Km, IDv)]────► VUK         ✅ AES-G(M,I)==V              (GoT + UHD)
VUK            ──[Kcu = AES-128D(Kvu, ·)]──► Unit Key     ✅ in Unit_Key_RO.inf         (UHD)
Unit Key+seed  ──[AES-128E⊕seed, AES-CBC]──► plaintext    ✅ byte-identical to MakeMKV  (GoT, live SCSI capture)
                                                          ✅ 32/32, video PID 0x1011    (TURBO UHD/AACS 2.0, live)
```

AACS 2.0 is, cryptographically, AACS v1 with SHA-256/P-256 and the player keys
formerly hidden in SGX. That thesis is now proven on **real v2/UHD content**: a
TURBO UHD Aligned Unit captured live off the bus decrypts to valid MPEG-TS (BDAV
video PID `0x1011`); GoT v1 content decrypts byte-identical to MakeMKV. **No bus
decryption is needed** — MakeMKV's **LibreDrive read returns raw disc sectors (no
bus-encryption layer), even on a BEE/UHD disc**; bus encryption only afflicts plain
OS reads (spec 11 §11.4.6). See [`specs/`](specs/) for the cited derivation.

## Layout

```
specs/          The specification series (00–12) — the source of truth.
crates/
  freeblue-crypto    AES-128, AES-G, AES-G3                  (✅ implemented + KATs)
  freeblue-mkb       MKB parse + media-key derivation        (✅ implemented + KATs)
  freeblue-content   Aligned-Unit decrypt + bus_decrypt_unit (✅ implemented + KATs)
  freeblue-keys      KEYDB.cfg parser                        (✅ implemented + tests)
  freeblue-disc      /AACS/ structures + raw m2ts read       (✅ implemented + tests)
  freeblue-read      UnitReader: PlainUdfReader + LibreDrive  (✅ LibreDrive unlock via SG_IO — no MakeMKV)
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
cargo test            # unit KATs (deterministic, no secrets) — 36 pass, 4 ignored
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
