# 04 — On-Disc Structures

> **Status:** ✅ Verified (layout + BEE) / 📋 Design (v2 auth) — where AACS 2.0 data physically lives on a UHD Blu-ray
> volume, how to read it, and the drive↔host authentication that gates the
> Volume ID. This is the "left of the +" half of spec 00 §0.4. Heavily **[?]**:
> UHD-specific on-disc layout is the least-documented part and a primary RE
> target (spec 07). The v1 layout (**[E]**) is the starting hypothesis.

## 4.1 The UHD volume, top down — ✅ [Disc]-confirmed

A UHD Blu-ray is a UDF volume. The `/AACS/` layout is now **confirmed from a real
AACS 2.0 disc** — the MKBv82 dump `res/MKB20_v82_…tgz` ("The Warning Live From
Auditorio Nacional", disc-id `a4a2…4d1c`, dumped by MakeMKV 1.18.3 + ASUS
BW-16D1HT/LibreDrive). Sizes are this disc's actual bytes:

```
/AACS/
   MKB_RO.inf            3,813,376 B  ← the Media Key Block (spec 03)        [Disc]
   Unit_Key_RO.inf          65,536 B  ← encrypted CPS Unit Keys (§4.5)       [Disc]
   Content000.cer              360 B  ← content certificate (P-256/SHA-256)  [Disc]
   Content001.cer              232 B                                          [Disc]
   Content002.cer              336 B                                          [Disc]
   ContentRevocation.lst 1,048,576 B  ← host/drive revocation list           [Disc]
   DH_Pairing_Server.cer       144 B  ← NEW in v2: online-pairing server cert [Disc]
/BDMV/
   PLAYLIST/*.mpls            ← playlists (title structure; rdd spec 03)      [E]
   CLIPINF/*.clpi             ← clip info                                     [E]
   STREAM/*.m2ts              ← the encrypted A/V (what we decrypt)           [E]
```

The dump also carried tool-side files (not on the disc proper): `kv.dat` (MakeMKV
key/value cache), `discatt.dat` (disc attributes), `fs_00x.bin` (UDF sectors),
`version.txt`, `log.txt`.

> **`DH_Pairing_Server.cer` is the v2 tell.** Its presence confirms the
> *enhanced / online-pairing* model (spec 01 §1.4.1, the deck's Title Key Server)
> — a Diffie-Hellman pairing-server certificate has no v1 analogue. (The talk's
> `CLDShowX2` is a *player-side* seal blob, **not** on the disc — **[Talk 23:34]**
> — correctly absent here.) Caveat: this is **one** disc; a second UHD title
> should confirm the layout generalizes (multi-disc corpus, spec 09 §9.8).

## 4.2 Reading the disc (drive access)

- `freeblue` reads from a **decrypted-at-rest target** wherever possible: a
  raw disc image, a folder dump, or a block device. The on-disc *structures*
  (MKB, Unit Key File, certs) are **not** themselves drive-auth-gated on most
  discs and can be read directly **[?-confirm]**.
- The **Volume ID** *is* gated (§4.3) and requires talking to a real drive with
  AACS drive↔host auth — the one step that may need MMC commands to a physical
  UHD-capable drive.
- Reuse, don't reinvent: drive access, UDF parsing, and BDMV navigation are
  already solved by `libbluray`/`libudfread` and by `rippidydoodah`'s disc layer
  (rdd spec 02). `freeblue` should consume those for structure reading and focus
  its novel code on AACS 2.0 crypto. See spec 08.

## 4.3 Drive↔host authentication and the Volume ID — [E] mechanism, [?] v2

The Volume ID lives in a **special, normally-unreadable area** the drive only
releases **after a mutual drive↔host authentication handshake** (**[Talk 5:03,
4:58]**). v1 mechanism **[E]**:

1. Host and drive exchange certificates (AACS drive cert / host cert).
2. Mutual challenge-response signature check (v1: custom-curve ECDSA; **v2:
   P-256 [R]**, spec 02 §2.3.5).
3. On success the drive returns the **16-byte Volume ID** (and permits reads of
   any protected sectors).

**[?] for v2:** the host certificate / private key `freeblue` must present, and
whether the published key set (spec 06) includes a usable **host certificate**
for this handshake or only the *content* device keys. This is the analogue of
rdd spec 02 §2.4's "host-cert revocation blocks bus-encrypted discs" limit —
resolve early, because **without a Volume ID there is no `Kvu`** (spec 02 §2.4.3)
and the pipeline cannot complete on a real drive. (An image that already
captured the Volume ID sidesteps this — see §4.6.)

### 4.3.1 Host Certificate format — [Arch]

The architecture deck's "Host Certificate" slide gives the field layout for both
generations (**[Arch]**, `res/arch-deck-extracted.txt`). The v1→v2 diff is the
clearest concrete evidence of the curve change (40-byte → 64-byte = ~160-bit →
P-256, two coordinates each):

```
  AACS1 Host Certificate            AACS2.0 Host Certificate (proposal)
  ─────────────────────────         ────────────────────────────────────────
  Certificate Type                  Certificate Type
  Reserved || DKS || BEC            Reserved || DKS || BEC
  Length                            Length
  Host ID (6-byte)                  Host ID (6-byte)
  Reserved                          device number "d" of paired
  Host Public Key (40-byte)           Device Key Set (4-byte)      ← NEW in v2
  Signature Data (40-byte)          Reserved
                                    Host Public Key (64-byte)      ← P-256 (was 40)
                                    Signature Data (64-byte)       ← P-256 (was 40)
```

Notable v2 additions (**[Arch]**), all **[Arch]-reported / proposal-stage**:
- **`device number "d"` (a 4-byte field; "d" itself is 31-bit)** pairs the host
  cert to a specific Device Key Set — the deck's "Pairing of Host Private Key and
  Device Key Set" mechanism for *synchronized revocation* (revoke either the host
  private key or the device keys and both are revoked).
- **`Host Type` (1-bit)**: `0` = Type A device key (Enhanced revocation), `1` =
  Type C device key (Proactive renewal). These device-key *types* likely map to
  the basic vs. enhanced disc models (spec 01 §1.4).

For `freeblue` this means a usable v2 **host certificate** is `Host ID +
device-number + 64-byte P-256 public key + 64-byte signature`, and the matching
**host private key** is what spec 06 §6.6 flags as the possibly-missing input for
§4.3. Whether the published key material includes a valid such cert/key pair is
**[?]** and gates live-drive use. Field widths are **[Arch]**-level (a 2014 draft
marked "proposal"/"TOSHIBA TO REVISE"); confirm against a real cert + `libaacs`.

### 4.3.2 Bus Encryption (BEE) — ✅ [Disc] — the raw-read blocker

**The most important practical finding for live-disc ripping.** Many discs set
**Bus Encryption Enabled (BEE)**: the drive, after AACS drive↔host auth, encrypts
the content sectors it returns to the host under a per-session **bus key**, so the
data crossing the drive→host bus is protected *on top of* AACS content encryption.
`libaacs` cannot do the auth (its host cert is revoked) → it cannot get the bus
key → it **fails on BEE discs** (rdd spec 02 §2.4). MakeMKV/LibreDrive bypass it
with raw drive access.

**Verified [Disc]** on real discs (the community keydb tags BEE discs `…/BEE/…`):

| Disc | BEE? | Plain UDF read + standard content decrypt (spec 05) |
|---|---|---|
| GoT: Conquest & Rebellion (BD, MKBv63) | no | ✅ 32/32 TS-sync — works |
| Turbo (BD, MKBv36) | **yes** | ❌ random (~1/32) — fails with *every* unit key |
| The Warning (UHD, MKBv82) | **yes** | content untested (structure-only dump) |

For Turbo, the **key hierarchy still verifies** (`AES-G(Km, IDv) == Kvu` byte-
exact) and all 7 CPS-unit keys were tried — none decrypt the raw stream. So BEE
adds a layer the keys+content-cipher pipeline does not remove.

**Critical implication for the UHD goal:** the UHD disc here is **also BEE**, so
this is *not* a v1 curiosity — it is on the critical path for ripping UHD. A plain
OS/UDF read is **insufficient** for any BEE disc. `freeblue` (the decrypt half)
must be paired with a read path that returns *non-bus-encrypted* AACS content,
via one of:
1. **LibreDrive-style raw reads** (what MakeMKV does) — a flashed/compatible drive
   read in a mode that omits bus encryption. The pragmatic route; a drive-firmware
   dependency, not crypto.
2. **AACS drive↔host auth with a valid (unrevoked) host cert** (§4.3.1) →
   negotiate the bus key → un-bus-encrypt the transfer yourself. Needs a usable
   host cert/key (spec 06 §6.6, the hard `[?]`).

Either way, **bus-key handling is a read-path concern outside the pure-crypto
core** (specs 02–05 remain correct and `[Disc]`-verified). This is tracked as a
first-class scope item (spec 00 §0.4) and roadmap phase (README), not a defect in
the decryption math. **[?]:** the exact bus-key derivation / where BEE is flagged
in the Unit Key File / CCI (lift from `libaacs` `mmc.c` + the AACS spec).

## 4.4 The MKB on disc

- Located in `/AACS/` (§4.1); read as a raw blob and handed to spec 03's parser.
- Capture the raw bytes as a **test fixture** the moment a corpus disc is
  available (spec 03 §3.5, spec 09) — it is the input to the first verifiable
  milestone (processing-key KAT).
- **[?]** exact filename and whether multiple MKBs exist (e.g. a small "drive"
  MKB vs. the content MKB, as in some BD layouts).

## 4.5 The Unit Key File

Holds the per-CPS-unit **title/unit keys** (`Kcu`), encrypted under the **Volume
Unique Key** `Kvu` (spec 02 §2.4.3–2.4.4). v1 shape **[E]**, v2 framing **[?]**:

```
UnitKeyFile:
   header: number of CPS units, version, flags                 [?]
   for each CPS unit:
       encrypted CPS Unit Key (16 bytes)  =  AES-128E(Kvu, Kcu) [E, BD §3.9]
       CPS-unit → application/title mapping                     [?]
```

**Decryption resolved [E] and ✅ [Disc]-verified** — **[BD §3.9]**: *"Encrypted
CPS Unit Key field contains the 16 bytes of the encrypted CPS Unit Key (Kcu)…
encrypted as `AES-128E(Kvu, Kcu)`."* So for a **basic** on-disc disc the unwrap
is plain AES-128 ECB:

```
Kcu = AES-128D(Kvu, EncryptedCPSUnitKey)        # [BD §3.9], verified on real v2
```

> **✅ [Disc] — verified on real AACS 2.0.** On the MKBv82 UHD disc (spec 06
> §6.5.2), `AES-128E(Kvu, Kcu)` for the keydb's Kvu/Unit-Key is present **verbatim
> in this disc's real `Unit_Key_RO.inf` at offset 112** — i.e. the first encrypted
> unit key sits after a 64-byte header record (type `0x00`, len `0x40`) and a
> 48-byte sub-header. The wrap is confirmed plain `AES-128E(Kvu, ·)` — no AES-G,
> no nonce — for a basic UHD disc.

No AES-G, no nonce, no AES_H on this path. (The nonce/AES_H variant
`AES-128E(Kvu, Kt ⊕ Nonce ⊕ AES_H(Volume ID ‖ title_id))` is the *downloaded /
Virtual-File-System* title-key case — spec 02 §2.3.7, the *enhanced* online-key
disc, not the basic target.) The mapping from a **playlist/clip** (rdd spec 03)
to its **CPS unit index** to its `Kcu` is the join that lets spec 05 pick the
right key per stream — its encoding remains a priority **[?]** (overlaps spec 02
§2.8, spec 05 §5.2).

## 4.6 Working from an image vs. a live drive

Two intake modes, with different `[?]` exposure:

| Mode | MKB / Unit Key File | Volume ID | Notes |
|---|---|---|---|
| **Live UHD drive** | read directly | via drive↔host auth (§4.3) | needs a host cert; full pipeline |
| **Pre-made image / folder** | read directly | **only if captured** in the image | many tools don't store the gated Volume ID; pipeline may stall at `Kvu` |

The corpus (spec 09) should include at least one **live-drive** capture so the
§4.3 auth path is exercised, and one **image** capture (with Volume ID recorded
out-of-band) so the crypto path can be developed without a drive present.

## 4.7 Content certificate and revocation lists

- v2 content certs are **P-256 / SHA-256** signed (**[Talk 50:01–50:07]**).
  Whether `freeblue` *must* validate them before decrypting: **resolved — no.**
  GoT and Turbo decrypted from keys + ciphertext alone, with **no content-cert
  validation step** (§5.3.1). A ripper, unlike a compliant player, does not need
  to verify the cert chain to recover plaintext, so `freeblue` skips it.
- The disc's **revocation lists** can revoke host/drive certs; note the v1
  "sneaky" behavior where inserting a disc with a *newer* revocation list makes
  the drive persist it to NVRAM (**[Talk 9:58]**) — a reason to be careful with
  unknown discs on a real drive. **[?]** whether v2 keeps this NVRAM-write
  behavior; flag as an operational caution in spec 08/10.

## 4.8 The optional on-disc Security Module — [Arch]

The deck's "Playback from Disc" slide lists a disc's assets as **"MKB and
records"** plus an **optional "Security Module"**, and the playback process
includes "Is there a Security Module on the disc (if yes, load Security Module)"
(**[Arch]**). This is a disc-delivered code module (the v2 analogue of BD+'s
disc-delivered VM code, spec 00 §0.3). For the corpus discs the talk studied,
no such extra module blocked decryption (the talk reaches plaintext with keys
alone), so `freeblue`'s baseline assumes **no Security Module**. If a corpus disc
carries one, that disc is flagged and set aside — handling disc-delivered
security code is **out of scope** (spec 00 §0.3), same posture as BD+. **[?]**
how often UHD discs ship one in practice.

## 4.9 Open questions

- ~~Exact `/AACS/` filenames on UHD~~ → ✅ **[Disc]**-confirmed (§4.1). Record
  framing within `Unit_Key_RO.inf` for multi-CPS-unit discs is still partly open.
- **[?]** Whether the published key set includes a working **host certificate +
  private key** for §4.3 (format now known from **[Arch]**, §4.3.1; presence of a
  usable instance still open — blocks live-drive use otherwise).
- **[?]** Unit-Key-File unwrap cipher (ECB vs AES-G) and CPS-unit mapping (§4.5).
- **[?]** Whether content-cert / revocation validation is mandatory before
  decryption matches MakeMKV (§4.7).
- **[?]** Whether UHD adds a second/"enhanced" MKB or bus-encryption wrinkle
  (cf. rdd spec 02 §2.4). *Partly informed by [Arch]:* basic (keys on disc) vs.
  enhanced (keys online) disc models exist (spec 01 §1.4); whether that changes
  the on-disc MKB layout vs. just key delivery is open.
- **[?]** How common the optional on-disc Security Module (§4.8) is on retail UHD.
