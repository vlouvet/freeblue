# 06 — Key Sources and SGX Provisioning

> **Status:** 📄 Reference — where the AACS 2.0 device keys come from, documented
> so the project understands its single most important *input* and its
> provenance. **This is background, not a freeblue work item:** spec 00 §0.3
> makes new key extraction a hard non-goal. `freeblue`'s trust boundary begins
> *after* the keys exist. Almost entirely **[R]** from `res/talk-transcript.txt`.

## 6.1 The input freeblue consumes

`freeblue` needs an **AACS 2.0 device key set** (the **253-key** set,
**[Talk 47:18]**) and possibly a **host certificate** for drive auth (spec 04
§4.3). These are **user-supplied** at runtime via a key database (§6.5). The repo
ships **none of them** (spec 10 §10.4). This spec explains *how such a set comes
to exist* — so contributors understand provenance, revocation exposure (spec 03
§3.6), and why the keys are a durable public input — without `freeblue` itself
doing any of it.

## 6.2 Why the keys were hard to get (SGX, in brief)

AACS 2.0 moved the device keys into an **Intel SGX enclave** (spec 01 §1.5,
**[Talk 14:16]**). The keys are:

- **Never in the binary.** Provisioned online, after install (§6.3).
- **Bound to one CPU.** SGX *sealing* derives its key from a per-CPU **Root
  Sealing Key** fused at manufacture and unknown even to Intel (**[Talk 27:08]**),
  so a sealed key blob is useless on any other CPU (**[Talk 27:50]**).
- **Gated by remote attestation.** Only a genuine, verified enclave on genuine
  Intel silicon can receive the keys (§6.3).

So you cannot just copy a key file off a licensed machine.

## 6.3 How keys are provisioned (the legitimate flow) — [R]

The CyberLink PowerDVD flow (**[Talk 23:15–40:03]**):

1. PowerDVD detects a UHD title, checks for SGX, loads the **CLTA** trusted
   agent.
2. If no sealed key blob (`CLDShowX2`) exists, it loads **CKD** (CyberLink Key
   Downloader enclave), which performs **SGX remote attestation** to CyberLink's
   server.
3. Attestation proves "a genuine, unrevoked enclave on genuine Intel hardware is
   asking." On success the server sends the **AACS 2.0 device keys + the PCL
   key**, encrypted to an attestation-derived session key.
4. The enclave **seals** them to disk (`CLDShowX2`), bound to that CPU. The
   real decryption enclave **CLTE** later unseals and uses them.

The whole point: the keys only ever materialize *inside* an enclave, in
shareable form *never*.

## 6.4 How that was defeated — [R], for provenance only

The SGX.fail chain (**[Talk 40:03–48:24]**), summarized for understanding, **not
to be reproduced or implemented by freeblue**:

1. **Foreshadow** side channel against the Intel **Quoting Enclave** leaked the
   **EPID attestation private key** — only ~128 bits of seal key needed
   (**[Talk 43:20, 44:06]**).
2. With the EPID key, the researchers built a **rogue quoting enclave** that
   signs anything, and then **emulated the entire attestation flow in software**
   — "if you can run Python, congratulations, you're now an authentic Intel SGX
   enclave" (**[Talk 47:54]**). Notable secondary findings that made this
   tractable: a debug build (`CLTASW`) with the full algorithm and **hard-coded
   AES keys/IV** was shipped by mistake (**[Talk 25:25, 46:34]**).
3. They then **downloaded the keys directly** — a full **253-key device set +
   PCL key** (**[Talk 47:18]**) — and later demonstrated a valid v2 **media key**
   and **processing key** (**[Talk 52:58]**), the latter able to decrypt *any*
   UHD disc up to its MKB revision.

### 6.4.1 Why this matters to freeblue (and why we still don't do it)

- **It makes the input public and reproducible.** A cheap vulnerable Intel CPU
  (e.g. i3-7100) extracts keys faster than discs can be remastered
  (**[Talk 55:23]**) — the §1.3.1 asymmetry — so device key sets exist in the
  wild and stay viable.
- **But freeblue still treats acquisition as out of scope** (spec 00 §0.3): it
  is already done, already published, and reproducing an SGX exploit is neither
  necessary for, nor part of, specifying decryption. We consume keys; we do not
  mint them.

## 6.5 Key database format (the freeblue input contract)

`freeblue` reads keys from a user-provided file, mirroring the established FLOSS
convention so existing key sets and `libaacs` tooling interoperate. The talk
notes the extracted keys were used by **patching the VLC libaacs plugin and
dropping the keys into the standard `KEYDB.cfg` format** (**[Talk 51:57]**) — so
that format is the target.

- **Primary format: `KEYDB.cfg`** (libaacs convention). **Resolved [E]** from a
  real current 182k-entry instance (`res/keydb_eng.zip` → `keydb.cfg`, header
  dated 2026-06-03; **note: this is an AACS *v1* database — see §6.5.1**). The
  grammar is two record families:

  **Per-disc entries** — keyed by a 40-hex (20-byte) AACS disc ID, with
  `|`-delimited fields:
  ```
  0x<40-hex disc-id> = (Title) | D | <date> | M | 0x<20-byte> | V | 0x<...> | U | 0x<...> | I | 0x<...>
       D = disc date      M = MKB/media-key hash (20 B)    V = Volume ID
       U = Unit/CPS-Unit Key(s)                            I = disc info/id
  ```
  **Global key records** (a handful, the actual decryption secrets):
  ```
  | DK | DEVICE_KEY 0x<16B> | DEVICE_NODE 0x<2B> | KEY_UV 0x<4B> | KEY_U_MASK_SHIFT 0x<1B>
  | PK | 0x<16B> ; MKBv63            (processing key, tagged by MKB-version range)
  | HC | HOST_PRIV_KEY 0x<priv> | HOST_CERT 0x<cert>
  ```
  The `DK` shape (DEVICE_KEY + DEVICE_NODE + KEY_UV + KEY_U_MASK_SHIFT) is the
  SD-tree node addressing of spec 03 §3.2 made concrete: the node position and
  the `u`/`v` mask that place a device key in the tree. `PK` keys are
  version-scoped because each spans an MKB-version range (spec 03 §3.1). This
  is the exact format the v2 keys get "dropped into" (**[Talk 51:57]**).
- **Device key set file** — the v2 253-key set. It will reuse the `DK`/`PK`
  record shapes above (the talk dropped v2 keys into this same `KEYDB.cfg`
  format), with v2-sized fields (P-256 host keys are 64-byte vs v1's 20). **[?]**
  finalize in spec 08 once a v2 set is in hand.

### 6.5.1 What `keydb_eng.zip` does and does not give us (per-disc keys ≠ device keys)

The supplied `res/keydb_eng.zip` has **two layers** with different reach:

- **Global key records (`DK`/`PK`/`HC`) — AACS v1 only.** Device keys are
  16-byte, processing keys cover only **MKB v63/64/66** (v1 Blu-ray), the host
  private key is **20-byte (160-bit ECC = v1)** (`libaacs` reads `host_priv_key`
  as 20 B, `host_cert` as 92 B). A v2 host key would be 64-byte P-256 (spec 04
  §4.3.1). So the keydb **cannot derive keys for an unknown disc** of either
  generation past those MKB versions, and has **no v2 device/processing keys**.
- **Per-disc entries — cover UHD too.** Correcting an earlier reading: the keydb
  holds **20,228 UHD / AACS 2.0 entries** (MKB versions up to **82**: 4,394 at
  v82, 3,844 at v81, 3,356 at v77, …). Each per-disc entry gives that disc's
  **Volume Unique Key and Unit Key directly**, so any UHD disc *already in the
  keydb* can be decrypted **without the v2 device keys at all** — the device-key
  → MKB → media-key path is only needed for discs *not* in the keydb.

**Verified keydb field semantics** (`<disc-id> = (title) | D | <date> | M | … |
I | … | V | … | U | n-… ; MKBvNN`), pinned by byte-test on a real disc (§6.5.2):
`D`=date · **`M`=Media Key (Km)** · **`I`=Volume ID (IDv)** · **`V`=Volume Unique
Key (Kvu)** · **`U`=Unit/CPS Unit Key(s)** (`1-0x…` = unit #1). The note tail
records `MKBvNN`, the FindVUK version, the main playlist, and volume size.

### 6.5.2 Real AACS 2.0 disc — the core hierarchy is byte-verified [Disc]

Using the supplied UHD dump `res/MKB20_v82_THE_WARNING…tgz` (disc structures) +
its keydb entry (disc-id `a4a2…4d1c`, MKB **v82**), the v1 key formulas were
confirmed to hold **byte-exactly on real AACS 2.0** (test in `/tmp`, no key
values committed per spec 10):

1. **`Kvu = AES-G(Km, IDv)`** — computing `AES-G(M, I)` from the keydb fields
   reproduced `V` exactly. → spec 02 §2.4.3 confirmed **[Disc]**.
2. **`EncryptedCPSUnitKey = AES-128E(Kvu, Kcu)`** — `AES-128E(V, U)` is present
   verbatim in `Unit_Key_RO.inf` at **offset 112** (the first encrypted unit key,
   after a 64-byte header + 48-byte sub-header). → spec 04 §4.5 confirmed
   **[Disc]**; decrypt with `Kcu = AES-128D(Kvu, ·)`.

This is the project's central thesis proven at the byte level: **the AACS v1
key-derivation math is unchanged in v2.** Only the device-key → media-key step
(needs the v2 device keys / MKB processing) and the content-cipher mode remain to
be verified the same way (spec 09 §9.10).
- **Loading discipline:** keys are read once at the I/O boundary, parsed from hex
  to `[u8;16]`, and **zeroized** after use (spec 02 §2.7). The path is supplied
  by config/CLI; **never** hard-coded, **never** committed (spec 10 §10.4).

## 6.6 What freeblue needs vs. what the leak provides

| Needed by freeblue | Provided by the published leak? | Spec |
|---|---|---|
| Device key set (→ processing key) | **Yes** — 253 keys [R] | 03, 02 |
| Processing/media key (shortcut) | **Yes** — demonstrated [R] | 03 |
| Host certificate for drive auth | **[?]** — unclear if included | 04 §4.3 |
| PCL key | provided, but **not needed** by freeblue (it only mattered inside the SGX/PCL wrapper) | — |
| BD+ keys | provided but **blank/unused** on these discs [R 47:05] | — |

The **host certificate** gap (§4.3) is the one open input risk: without it,
live-drive Volume ID retrieval may be blocked, limiting `freeblue` to images
that already captured the Volume ID (spec 04 §4.6).

## 6.7 Open questions

- ~~Exact `KEYDB.cfg` grammar~~ → **[E]** resolved from `res/keydb_eng.zip`
  (§6.5). The **v2 device-key-set** file layout is still **[?]** until a v2 set
  is in hand (§6.5).
- **[?]** Whether a usable **v2** host certificate is part of the user-sourced
  material (§6.6, spec 04 §4.3). (The v1 keydb's host cert does **not** work for
  UHD — it is 160-bit, not P-256.)
- **[?]** Revocation status of the published keys against current corpus discs
  (spec 03 §3.6) — purely a data question, re-checked per disc.
