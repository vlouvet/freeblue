# freeblue

A clean-room, **all-original Rust** library that decrypts **AACS 2.0** (Ultra HD
/ 4K Blu-ray) — the gap the FLOSS stack (`libaacs`/`libbluray`) leaves open. It
turns an encrypted UHD/BD volume plus user-supplied key material into **plaintext
M2TS**, so a remuxer like
[`rippidydoodah`](https://github.com/vlouvet/rippidydoodah) can produce a
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
  freeblue-cli       `freeblue` unlock / decrypt / decrypt-disc (✅ standalone rip — no MakeMKV)
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

## Decrypt a disc (no MakeMKV)

On a LibreDrive-capable drive (e.g. LG WH16NS60), `freeblue` reads and decrypts a
protected UHD/BD disc itself. SG_IO + mount need root.

```sh
# 1. Put the drive in raw-read mode (replays the read-only LibreDrive unlock).
sudo freeblue unlock /dev/sr0

# 2. Mount the now-raw disc and decrypt an m2ts to plaintext M2TS.
sudo mount -t udf -o ro /dev/sr0 /mnt/disc
sudo freeblue decrypt /mnt/disc/BDMV/STREAM/00002.m2ts \
     --unit-key <32-hex-CPS-unit-key> -o title.m2ts      # → 32/32 TS-sync

# …or read an extent straight off the drive (no mount):
sudo freeblue decrypt-disc /dev/sr0 --start-lba N --num-units M \
     --unit-key <hex> -o title.m2ts
```

The CPS unit key is the keydb `U` field for the disc (auto-resolution from the
disc's Volume ID is on the roadmap — spec 12 §12.15). `freeblue` only ever issues
`READ BUFFER`/`READ(10)` — it never flashes firmware, so it cannot brick a drive;
a non-LibreDrive drive simply reports `not LibreDrive-capable`.

## Working agreement

Read [`CLAUDE.md`](CLAUDE.md) before contributing: **TDD-always**, **spec-first**,
**no keys/media in the repo, ever**. `freeblue` is built **clean-room** from
public AACS specifications and NIST/RFC standards plus public 37c3 research —
never from MakeMKV/CyberLink/`libaacs` source (which is consulted, if at all, only
as a read-only behavioral oracle; no code is copied).

## License

GPL-3.0-or-later — full text in [`LICENSE`](LICENSE). Licensing rationale and the
spec/docs vs. code split: [`specs/10-legal-and-licensing.md`](specs/10-legal-and-licensing.md).

## Legal & disclaimer

`freeblue` is a Free Software **interoperability** library that decrypts AACS 2.0
(UHD/4K Blu-ray) so discs you own can be played and remuxed on the platforms of
your choice — the gap the FLOSS Blu-ray stack leaves open. It is research and
interoperability software, not a piracy tool.

- **No keys, tables, or media are included.** freeblue ships **no** device keys,
  processing keys, media keys, `KEYDB.cfg`, host certificates, Volume IDs, unit
  keys, or any copyrighted content — encrypted or decrypted. All key material is
  supplied by you at runtime and lives only in git-ignored `fixtures/` (Rule 4).
- **Clean-room originality.** Cryptographic constants and protocol facts come from
  public AACS specs, NIST/RFC standards, and published 37c3 research, each cited
  inline. Constants/protocol facts are not copyrightable; no third-party code is
  copied (spec 08 §8.6, spec 10 §10.2).
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
