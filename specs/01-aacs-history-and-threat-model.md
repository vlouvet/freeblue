# 01 — AACS History and Threat Model

> **Status:** 📄 Reference — the lineage (CSS → AACSv1 → AACS 2.0), what v2
> actually changed, and the threat model that explains *why* v2 was breakable.
> Read this before the crypto specs (02–05) so the design choices make sense.
> Almost everything here is **[R] Reported** from `res/talk-transcript.txt`;
> timestamps are inline.

## 1.1 Why a history section is in a decryption spec

AACS 2.0 is not a from-scratch design. The talk's central technical finding is
that v2 is **"pretty much the exact same thing as AACSv1, just slightly updated
with some new crypto ciphers"** (**[Talk 49:43]**). So the *correct* mental model
for implementing v2 is: "implement v1, then apply v2's diffs." This spec
establishes that baseline and enumerates the diffs. The v1 baseline is where our
**[E] Established** confidence comes from — it is public.

## 1.2 CSS (1996) — the cautionary tale

- Content Scramble System, the DVD scheme; M6 block cipher, multi-keyed
  (authentication / title / disc / player keys + a disc key block) **[R 0:59]**.
- Goal: stop naive bit-for-bit disc copies and force licensing of drives and
  players **[R 1:11]**.
- Broke because (a) 40-bit export-control-era key was brute-forceable in ~18s on
  a 450 MHz PIII, and (b) the revocation model had no tolerance for leaks; once
  `DeCSS` could regenerate the whole key system, revocation was meaningless
  **[R 2:32]**. Dead by 1999.

**Lesson AACS took:** strong ciphers + a *real* revocation model.

## 1.3 AACS v1 (2006) — the baseline freeblue reuses

AACS v1 shipped for HD DVD / Blu-ray. Notable: **the spec was published openly**
(April 2005, **[Talk 3:40]**), which is why the v1 crypto is our `[E]` ground
truth. Key properties (all **[E]**, restated in talk **[R 3:51]**):

- AES-128 + SHA-1, standard cipher suites.
- A **key hierarchy** (detailed in spec 02): title key → volume unique key →
  media key → processing key → device keys.
- **Media Key Block** revocation via a **subset-difference tree** (spec 03),
  invented 2001 **[R 6:16]**.
- Mutual **drive↔host authentication** gating access to the Volume ID and the
  protected sectors **[R 5:03]**.
- **Traitor tracing** support **[R 4:13]**.

### 1.3.1 How v1 fell (the threat model that recurs)

v1 was never broken *cryptographically*. It fell because **players kept the
decrypted keys in RAM**: PowerDVD / WinDVD were RAM-dumped and volume keys, then
a **processing key** (the infamous `09 F9…`), then whole device-key sets were
recovered (Dec 2006 – Feb 2007) **[R 11:32–12:21]**. The PS3 (a Blu-ray player)
root key leak added another key source **[R 12:55]**. AACS LA's response —
"not our fault your software left keys in RAM" **[R 13:22]** — is correct and
irrelevant: keys leaked faster than MKB revisions could revoke them. This
**asymmetry (cheap leak vs. expensive disc remastering)** is the recurring theme
and recurs for v2 (§1.6, spec 06).

## 1.4 AACS 2.0 (2015) — what changed

Built for UHD/4K Blu-ray (launched 2015 per **[Talk 13:57]**; note the talk
misspeaks "2008"). The deltas from v1, as reported:

| Aspect | v1 | v2 | Tag |
|---|---|---|---|
| Spec availability | published | **under NDA** | [R 13:57] |
| Hash | SHA-1 | **SHA-256** | [R 49:56] |
| ECC curve | custom ~160-bit | **NIST P-256** (`prime256v1`) | [R 50:01] |
| AES-based hashes (AES-G family) | present | **unchanged** | [R 50:07] |
| SD-tree revocation | yes | **same scheme, new device keys** | [R 50:12] |
| MKB record types | v1 set | **similar, some new entries** | [R 49:56] |
| Key custody | keys in player RAM | **keys in Intel SGX enclave** | [R 14:16] |
| Player key model | one shared key across all installs | **per-EPID-signature unique key set** | [R 50:23] |
| Platform lock | none | **Windows + Intel 7th-gen+ w/ SGX** | [R 14:16] |
| Device-key set size | (n/a here) | **253 keys** | [R 47:18] |

The cryptographic core is therefore **near-identical to v1**; the genuinely new
barrier is **custody (SGX)**, not **cipher**. That is the single most important
sentence in this spec for an implementer: spend your effort on the v1 pipeline,
parameterized for SHA-256 / P-256, and treat SGX as a *key-acquisition* problem
that someone else (spec 06) already solved.

### 1.4.1 Basic vs. enhanced discs, and title diversity — [Arch]

The 2014 architecture deck (`res/arch-deck-extracted.txt`) describes three
player capability sets and two v2 disc models that matter for `freeblue`'s data
flow (**[Arch]**, "UHD Player (with AACS 2.0)" slide):

- **AACS 1.x** — legacy discs (the two-key-sets coexistence, spec 03 §3.3).
- **AACS 2.0 (basic)** — **Title Keys are delivered on the disc**; no online
  connection required. This is the case `freeblue` targets: everything needed to
  reach plaintext is on the disc + the device keys.
- **AACS 2.0 (enhanced)** — **Title Keys are provided online** by a *Title Key
  Server*; the on-disc CPS Unit Key File is **"(Empty)"** (spec 02 §2.2 diagram).
  Here the title keys must come from the server (or a cached/known key), which is
  also where the talk's **per-EPID unique key issuance** and **"title diversity"**
  (per-player keys, traitor tracing) live (**[Arch]** "Title Diversity";
  **[Talk 50:23]**).

Implication: a **basic** disc is fully `freeblue`-decryptable offline with the
device keys. An **enhanced** disc needs the per-title key out-of-band (the online
server is not something `freeblue` impersonates — that is the SGX/attestation
path, spec 06, out of scope). Detecting which model a disc uses ("Determine what
kind of disc … Is online connection required" — **[Arch]** "Playback from Disc")
is part of spec 04's structure parsing.

## 1.5 The SGX custody model (threat model summary)

Full detail is in spec 06; the threat-model shape:

- v2 hides DRM code + keys inside an SGX **trusted enclave**, implemented by
  CyberLink in PowerDVD 17+ **[R 15:11]**. Enclaves resist kernel/hypervisor/
  SMM/physical inspection; memory leaving the core is transparently encrypted
  **[R 15:53]**.
- Keys are **not shipped** with the software. They are **provisioned online**
  via SGX **remote attestation** from CyberLink's server into the enclave, then
  **sealed to disk** (`CLDShowX2` blob) bound to that one CPU **[R 23:34, 27:31]**.
- So the secrets are: never in the binary, never in shareable form, and bound to
  silicon. On paper, unextractable.

## 1.6 Why v2 broke anyway (and what that buys freeblue)

The talk's exploit chain (spec 06 has the detail) **[R]**:

1. **Foreshadow** (a Meltdown-class speculative-execution + SGX cache side
   channel) leaked the **EPID attestation private key** from the Intel **Quoting
   Enclave** — only 128 bits of seal key needed **[R 43:20, 44:06]**.
2. With the EPID key, the researchers **emulated the entire remote-attestation
   flow** — no SGX, no Windows, no PowerDVD required — and downloaded the key
   blob. Demoed in **Python on a Raspberry Pi** **[R 47:54, 48:24]**.
3. Result: a full **253-key AACS 2.0 device-key set + the PCL key**
   **[R 47:18]**, and later **proof of a valid v2 media key and processing key**
   **[R 52:58]** — the latter able to decrypt *every* UHD disc up to its MKB
   revision.

**What this buys freeblue:** the device keys are now a *public, reproducible
input*. The asymmetry from §1.3.1 returns — a ~$10 vulnerable i3-7100 off eBay
extracts keys faster than discs can be remastered **[R 55:23]** — so the input
is durable. `freeblue` therefore never needs to touch SGX; it consumes the
published keys and does the v1-shaped math.

## 1.7 AACS 2.1 — out of scope, documented for awareness

- v2.1 is effectively **always-online DRM** and, per the talk, **never actually
  deployed**, so it cannot be studied in the wild **[R 50:56]**.
- Its notable feature is aggressive **traitor tracing**: per-user key bits
  encoded into *which chunks of video decrypt*, including **visible** watermarks
  (e.g. "a glimmer in a character's eye") that survive re-encoding — a
  sequence-key scheme revealing the leaker from the video alone **[R 51:14]**.
- **freeblue does not target v2.1** and does not attempt to defeat tracing
  (spec 00 §0.3). If a corpus disc is v2.1, it is excluded and noted.

## 1.8 Threat-model implications for this spec

- **We are not an attacker of SGX.** The spec's trust boundary starts *after*
  key acquisition. Spec 06 documents acquisition as background/reference only.
- **The cipher is not the secret.** Per §1.4, AACS 2.0's algorithms are
  standard or v1-derived. There is no "secret algorithm" to reverse — only
  parameter choices (curve, hash, record-type IDs, constants) to pin. That is
  what spec 07's MakeMKV RE is *for*: pinning parameters, not discovering
  fundamentally new crypto.
- **Revocation is a moving target, not a wall.** A media/processing key works
  until the next MKB revision; newer discs carry newer MKBs. The spec must read
  the MKB on *each* disc rather than assume a fixed processing key (spec 03).

## 1.9 Open questions

- **[?]** Exactly which MKB record types are new in v2 vs. v1, and their layout
  (spec 03 §3.3). Talk says "a couple very similar, just new MKB entries"
  **[R 49:56]** but does not enumerate.
- **[?]** Whether any corpus disc uses v2.1 features (would force exclusion).
- **[?]** Whether bus encryption / newer drive-auth wrinkles affect UHD reads
  the way they do some 2013+ v1 discs (cf. rdd spec 02 §2.4). Resolve during
  on-disc-structure work (spec 04).
