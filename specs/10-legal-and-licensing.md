# 10 — Legal and Licensing

> **Status:** 📄 Reference — the clean-room discipline, the
> reverse-engineering-for-interoperability posture, the no-keys/no-media rule,
> and the project license. This is not legal advice; it is the project's stated
> posture and the obligations contributors accept. Mirrors and extends the
> posture of `rippidydoodah` (rdd spec 10 §10.5) and of VideoLAN's
> `libaacs`/`libbdplus`.

## 10.1 What this project is, legally

`freeblue` is, first, a **written specification** reconstructing how AACS 2.0
decryption works, for the purpose of **interoperability** — enabling Free
software to read UHD discs the user already owns, on platforms (Linux, AMD, ARM)
that the licensed ecosystem refuses to serve (spec 01 §1.4, **[Talk 14:16]**).
This is the same niche, and the same rationale, as `libdvdcss` (CSS) and
`libaacs` (AACS v1): documenting and implementing a published-or-reverse-
engineered format so non-licensed, non-Windows platforms can interoperate.

The specification itself contains **facts and constants** (a curve name, an IV,
a record-type ID, a key-derivation step). Facts and interface/protocol details
are not the kind of expression that copyright protects; the spec transcribes
**no proprietary source code** (spec 07 §7.6).

## 10.2 Clean-room discipline (binding on contributors)

To keep both the spec and any implementation clean-room:

1. **No proprietary source.** Do not read, paste, or paraphrase MakeMKV,
   CyberLink, or AACS LA *source code* into the spec or the implementation.
2. **Behavior, not code.** Resolve `[?]`s by **observing MakeMKV's inputs and
   outputs** (spec 07 §7.2), and by reading the **FLOSS `libaacs`** baseline and
   public standards (spec 02 §2.1) — not by transcribing closed code.
3. **No AACS LA NDA material.** The AACS 2.0 spec is under NDA (spec 01 §1.4).
   Nobody who has signed that NDA should contribute spec text derived from it;
   the project builds only from public research (`res/`), public v1 docs, and
   black-box behavior.
4. **Separate fact from expression in static RE.** Where Ghidra/static RE of a
   proprietary binary is the *only* way to pin a `[?]` (spec 07 §7.5), extract
   the **fact** (the constant, the protocol step) and write the **code** freshly
   from the spec; document the provenance. Constants are facts; their embodiment
   in code is ours.
5. **Legal review gates static RE.** Any static reverse-engineering of a
   proprietary binary (e.g. `CLTASW`) is done only after the maintainer has
   reviewed the approach (spec 07 §7.8). Behavioral and `libaacs`-based
   derivation is always preferred and needs no such gate.

## 10.3 Reverse engineering for interoperability — the rationale

The project's RE is **for interoperability**, the purpose most jurisdictions
treat most favorably:

- It targets a **format/protocol** so Free software can read media the user
  **lawfully owns** — not to redistribute content, not to defeat payment.
- It develops **no new circumvention of the access control on a live service**;
  the hard access control (SGX) was already broken and published by independent
  academic research (spec 06), which `freeblue` only *cites*, and which it
  treats as a non-goal to reproduce (spec 00 §0.3).
- It mirrors prior FLOSS practice (`libdvdcss`, `libaacs`) that has coexisted
  with the same legal landscape for two decades under the VideoLAN umbrella.

This is a *posture*, not a legal guarantee. Anti-circumvention law (e.g. US DMCA
§1201, EU directives) varies by jurisdiction and is contested as applied to
personal-use interoperability. **Contributors and users are responsible for
their own jurisdiction's law.** The project makes no claim that any particular
act is lawful where you live.

## 10.4 No keys, no media, ever (hard rule)

Inherited verbatim in spirit from the parent project (CLAUDE.md Rule 3, rdd
spec 10 §10.7):

- **No device keys, no processing/media keys, no `KEYDB.cfg` contents, no host
  certificates** in the repository — not in code, tests, fixtures, history, or
  issues. They are user-supplied at runtime (spec 06 §6.5) and live only in a
  **git-ignored** fixtures dir (spec 09 §9.6).
- **No copyrighted media**, encrypted or decrypted — not even a single
  decrypted aligned unit as a "convenient" test vector. Secret KAT fixtures stay
  out of the repo (spec 09 §9.2, §9.6).
- **CI enforces it.** A secret/keys/media scanner runs on every commit and
  blocks merges that introduce key-shaped or media-shaped blobs (cf. rdd
  spec 10 §10.7). A leaked key in history is a history-rewrite incident, not a
  shrug.
- This rule is **why** the validation design (spec 09) goes to the trouble of an
  out-of-repo fixtures boundary and skips-loudly behavior: correctness must be
  testable **without** the repo ever containing a secret.

## 10.5 Project license

- **Spec text** (`specs/`, `res/` annotations, README): a permissive docs
  license — **CC BY-SA 4.0** proposed, so the reconstruction stays Free and
  attributable. **[?]** confirm with maintainer.
- **Reference implementation** (spec 08): **GPL-3.0-or-later** — **decided**
  by the maintainer. The full text ships as [`../LICENSE`](../LICENSE) and the
  workspace declares `license = "GPL-3.0-or-later"`. This matches
  `rippidydoodah` (rdd spec 10 §10.5), the primary consumer, which links
  `freeblue` and is itself GPL. The earlier LGPL-2.1+ proposal (to keep the door
  open to upstreaming v2 support into `libaacs`) is **superseded**: aligning the
  code license with rdd was preferred over the upstreaming option.
- The two may differ deliberately: docs CC BY-SA, code GPL-3.0+ — the common
  FLOSS split. With the code license now decided the repo is redistributable
  under GPL-3.0-or-later.

## 10.6 Citations and provenance hygiene

- Every technical claim cites its source per spec 00 §0.6 (`[E]`/`[R]`/`[?]`).
  `[R]` claims cite `res/talk-transcript.txt` with a timestamp; `[E]` claims cite
  the CCE book / `libaacs` / a NIST standard; `[?]` claims cite the deciding test
  once resolved (spec 07 §7.4).
- This citation discipline is also a **legal** asset: it documents that the
  spec's facts come from public research and standards, not from NDA material or
  proprietary code — the clean-room paper trail.

## 10.7 The `res/` materials

The conference recording, transcript, and the circulated architecture PDF in
[`../res/`](../res/) are third-party works included for **research reference**.
They are cited, not relicensed; their own terms apply. The 37c3 talk is publicly
published by the CCC/SGX.fail team. Do not assume the project's license extends
to them.

## 10.8 Open questions

- **[E]** Code license **decided: GPL-3.0-or-later** (§10.5). Spec/docs license
  (CC BY-SA?) still **[?]** pending maintainer confirmation.
- **[E]** Upstreaming into `libaacs` is **not** being pursued — GPL-3.0+ was
  chosen over LGPL to align with `rippidydoodah` (§10.5; spec 08 §8.6).
- **[?]** Jurisdiction-specific review before publishing the reference
  implementation (vs. the spec), given the DVD-CSS/`libaacs` precedents (§10.3).
- **[?]** Legal sign-off on any static-RE of proprietary binaries (§10.2 #5,
  spec 07 §7.8).
