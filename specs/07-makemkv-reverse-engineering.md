# 07 — MakeMKV Reverse-Engineering Methodology

> **Status:** 📋 Design — how we resolve the `[?]` tags scattered through specs
> 02–05 by using MakeMKV as a **behavioral oracle** and Ghidra as a structural
> aid. This is the project's empirical engine: every `[?]` becomes `[E]` only
> when a byte-match earns it. Read with the project's clean-room rules (spec 10).

## 7.1 What we're reverse-engineering, and what we're not

We are **not** trying to learn a secret algorithm — spec 01 §1.4 establishes
AACS 2.0 is v1 with standard-primitive swaps, so there is no hidden cipher. We
are pinning **parameters and constants**: AES-G direction, AES-G3 constants, the
content IV, seed offsets, MKB record-type IDs, the Unit-Key-File unwrap, and
cert/auth specifics. RE here is *measurement*, not *discovery*.

**Two oracles, used differently:**
- **MakeMKV (dynamic, behavioral):** the primary oracle. We never read or copy
  MakeMKV's code; we observe its **inputs and outputs** (the disc it reads, the
  decrypted bytes it produces, the keys it logs) and require `freeblue` to
  reproduce them. This is clean-room "observe behavior, not source" (spec 10).
- **Ghidra / static RE (structural):** a *secondary, optional* aid, used only
  where public sources (CCE book, `libaacs`, the talk) leave a `[?]` that
  behavior alone can't pin. Subject to the clean-room firewall (§7.6, spec 10).

## 7.2 MakeMKV as the behavioral oracle

MakeMKV is the de-facto reference for UHD ripping and already decrypts AACS 2.0
given keys. We use it as ground truth for the §0.4 contract:

```
oracle(disc, keys) := the plaintext M2TS MakeMKV writes for a chosen title
freeblue is correct ⇔ freeblue(disc, keys) == oracle(disc, keys)   (byte-exact)
```

Concretely:
- Rip a corpus disc (spec 09) with MakeMKV → reference plaintext M2TS (or the
  decrypted backup folder).
- Capture the **same disc's** raw structures for `freeblue`: MKB, Unit Key File,
  Volume ID, certs (spec 04).
- Run `freeblue` with the same key set and **diff the M2TS** (spec 09 §9.3).

The diff localizes the bug: a *uniform* mismatch from byte 16 of every unit ⇒
wrong block-key/IV (spec 05 §5.3); a per-unit-boundary mismatch ⇒ aligned-unit
size/seed bug; total noise ⇒ wrong `Ku` or MKB-derivation bug (spec 03).

## 7.3 MakeMKV's debug/log surface

MakeMKV exposes diagnostics useful as **intermediate oracles** (verify against
the actual build's behavior — **[?]** which fields appear):
- **Debug logging** (`makemkvcon … --debug=file`) and the expert/registration
  log settings can surface disc IDs, key/CINF info, and unit structure.
- The **`KEYDB.cfg` interaction** tells us which keys it expects and in what
  form (spec 06 §6.5).
- Any logged **Volume ID / media key / processing key** lets us test spec 03 and
  spec 02 *independently of the video* — the highest-value intermediate oracle,
  because it validates the key hierarchy before any content decryption (spec 03
  §3.5).

Record exactly which log fields a given MakeMKV version emits as a fixture note;
do not assume across versions (parent rule: match version-specific behavior).

## 7.4 The resolution protocol for each `[?]`

For every open question in specs 02–05:

1. **State the hypothesis** (the spec's current `[?]` assumption) and the
   public source that suggests it (CCE book / `libaacs` / talk).
2. **Design the discriminating test** — the smallest byte-match whose pass/fail
   distinguishes the hypothesis from alternatives (e.g. AES-G *D* vs *E*: decrypt
   one unit both ways, compare to MakeMKV).
3. **Run it** against the oracle (§7.2). *See it pass or fail* — never assert
   from reasoning alone (parent rule).
4. **On pass:** re-tag the claim `[E]`, record the vector as a KAT (spec 09
   §9.2), and cite the deciding evidence inline in the spec.
5. **On fail:** record the falsified hypothesis (so it's not retried), form the
   next, repeat.

This is the parent project's red→green TDD loop applied to *specification*: the
spec's `[?]`→`[E]` transitions are gated by real test runs, not by confidence.

## 7.5 Ghidra targets (only where behavior is insufficient)

Where the CCE book + `libaacs` + behavior can't pin a `[?]` (e.g. an undocumented
v2 MKB record layout, spec 03 §3.3), static RE may help. Likely targets and what
each could pin:

| Target | Pins | Spec |
|---|---|---|
| `libaacs` source (FLOSS, not RE) | v1 AES-G/AES-G3/MKB ground truth | 02, 03 |
| The talk's leaked **`CLTASW`** (unobfuscated debug build w/ full algo + strings — **[Talk 25:25]**) | v2 record types, constants, the "exact same as v1" deltas | 02–05 |
| UHD drive MMC traces | drive↔host auth, Volume ID retrieval | 04 §4.3 |

> `CLTASW` is the single most valuable artifact the talk surfaces: an
> accidentally-shipped, **unobfuscated, unencrypted** near-complete AACS 2.0
> implementation with debug strings (**[Talk 25:38]**). It is the closest thing
> to a readable v2 spec in existence. Using it is subject to §7.6.

## 7.6 Clean-room firewall (cross-ref spec 10)

To keep the *specification and any implementation* clean-room and
interoperability-grounded:
- The spec records **facts and constants** (a curve, an IV, a record-type ID),
  not transcribed proprietary code. Constants and protocol facts are not
  copyrightable; code is.
- Prefer **behavioral** derivation (§7.2) and the **FLOSS `libaacs`** baseline
  over static RE of proprietary binaries wherever a `[?]` can be closed that way.
- Where static RE of a proprietary binary is the only path, separate **fact
  extraction** (what constant / protocol step) from **expression** (how to write
  the code), and document provenance. Full posture and the
  reverse-engineering-for-interoperability rationale: spec 10.

## 7.7 Output of this process

A living table (maintained in this spec or spec 09) mapping every `[?]` →
{hypothesis, deciding test, status, KAT id}. The project is "spec-complete"
(spec 00 §0.8) when that table has no unresolved `[?]` in specs 02–05.

## 7.8 Open questions

- **[?]** Which MakeMKV version + invocation yields the most useful key/structure
  logging (§7.3), and whether it logs the media/processing key at all.
- **[?]** Whether `CLTASW` (or equivalent) is obtainable for §7.5 and under what
  terms (§7.6, spec 10).
- **[?]** Legal review sign-off on the static-RE path before any Ghidra work
  (spec 10).
