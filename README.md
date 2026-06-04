# freeblue

A clean-room, **all-original Rust** library that decrypts **AACS 2.0** (Ultra HD
/ 4K Blu-ray) — the gap the FLOSS stack (`libaacs`/`libbluray`) leaves open. It
turns an encrypted UHD/BD volume plus user-supplied key material into **plaintext
M2TS**, so a remuxer like [`rippidydoodah`](../rippidydoodah/) can produce a
playable `.mkv`.

> **Status:** the decryption core is **byte-verified on real discs** but the
> library is **early scaffolding** — most crates are stubs to be filled
> test-first. See [`specs/README.md`](specs/README.md) for the full picture and
> the integration roadmap.

## The pipeline (every step proven byte-for-byte against a real disc)

```
processing key ──[MKB 0x04/0x05 + verify]──► Media Key   ✅ derived == keydb M   (GoT, BD)
Media Key      ──[Kvu = AES-G(Km, IDv)]────► VUK         ✅                       (GoT + UHD)
VUK            ──[Kcu = AES-128D(Kvu, ·)]──► Unit Key     ✅ in Unit_Key_RO.inf    (UHD)
Unit Key+seed  ──[AES-128E⊕seed, AES-CBC]──► plaintext    ✅ 32/32 TS-sync         (GoT)
```

AACS 2.0 is, cryptographically, AACS v1 with SHA-256/P-256 and the player keys
formerly hidden in SGX. That thesis is now proven at the byte level; the only
unproven step is decrypting one real *v2 content* Aligned Unit. See
[`specs/`](specs/) for the cited derivation of every step.

## Layout

```
specs/          The specification series (00–10) — the source of truth.
crates/
  freeblue-crypto    AES-128, AES-G, AES-G3            (✅ implemented + KATs)
  freeblue-mkb       MKB parse + media-key derivation  (✅ implemented + KATs)
  freeblue-content   Aligned-Unit block-key + AES-CBC  (✅ implemented + KATs)
  freeblue-keys      KEYDB.cfg parser                  (stub — TDD)
  freeblue-disc      /AACS/ structures + raw m2ts read (stub — TDD)
  freeblue-core      orchestration → plaintext M2TS    (partial)
  freeblue-cli       `freeblue` binary                 (stub)
res/            Reference material (git-ignored) — see res/README.md.
fixtures/       Local KAT fixtures w/ real keys (git-ignored) — see fixtures/README.md.
overview.md     One-page project intro.
```

## Build & test

Requires Rust 1.75 (pinned in `rust-toolchain.toml`). On the build host:

```sh
cargo build
cargo test            # unit KATs (deterministic, no secrets)
cargo test -- --ignored   # KATs needing $FREEBLUE_FIXTURES (real disc data)
```

`cargo` is not installed on every host here; the implemented crates
(`freeblue-crypto`, `-mkb`, `-content`) carry deterministic KATs that must pass
on first `cargo test` on the build machine.

## Working agreement & legal

Read [`CLAUDE.md`](CLAUDE.md) before contributing: **TDD-always**, **spec-first**,
**no keys/media in the repo, ever**. `freeblue` performs decryption for
interoperability with discs you own; it ships **no keys** and is built clean-room
from public AACS specs + standards, never from MakeMKV/CyberLink source. Full
posture: [`specs/10-legal-and-licensing.md`](specs/10-legal-and-licensing.md).
