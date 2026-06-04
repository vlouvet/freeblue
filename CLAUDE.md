# CLAUDE.md — Working agreement for freeblue

Operating contract for any AI assistant (or human) contributing to `freeblue`.
**Read it before doing any work.** It mirrors `rippidydoodah`'s agreement, plus
the rules that matter most for a decryption project.

The rules below are non-negotiable. If a task seems to require breaking one,
stop and ask the human first.

---

## Rule 1 — Test-Driven Development, always

Every behavioral change lands as a **failing test first, then the code that makes
it pass.** Decryption is the worst possible place for "looks right" — a wrong
constant produces silent garbage, not an error.

- **Red → Green → Refactor**, and **run the tests for real**. Never write "this
  should pass"; paste the actual `cargo test` output.
- **Pin behavior with known-answer tests.** The implemented crates carry
  deterministic KATs (FIPS-197 AES, AES-G/AES-G3 vectors, the content block-key
  vector). Compare against reference outputs (real disc data, MakeMKV), not hopes.
- **No stubbed/`#[ignore]`d tests pretending to pass.** An ignored test must say
  *why* (e.g. needs `$FREEBLUE_FIXTURES`) and have a real body to enable later.
- **Every real-disc bug becomes a minimized fixture + a failing test before the
  fix** (spec 09 §9.7). A single mis-decrypted 6144-byte unit is a perfect fixture.

## Rule 2 — Spec-first; specs and code change together

[`specs/`](specs/) is the source of truth for *what* and *why*. Code implements
the specs, not the reverse.

- **Every behavioral change updates the relevant spec in the same commit.**
- **Confidence tags are load-bearing** (spec 00 §0.6): `[E]` established,
  `[Disc]` verified on a real disc, `[?]` open. **Do not upgrade a tag without
  the citation or byte-match that earns it.**
- **Cite, don't recall.** Implementing against AACS? Quote the AACS book / the
  `libaacs` source you checked — never write a constant from memory. (`libaacs`
  is a *read-only reference oracle*; **copy no code from it** — spec 08 §8.6.)

## Rule 3 — Git is the system of record

- **The repo is the truth.** Small, focused commits; imperative subject ≤ 72
  chars; body explains *why* and cites the spec section implemented.
- **Branch per change; `main` stays green** (all non-ignored tests pass).
- **Never force-push `main`; never amend a pushed commit** unless told to.
- **AI must not commit autonomously.** The human approves each commit; the AI
  stages changes and proposes a message.

## Rule 4 — No keys, no media, EVER (the load-bearing one)

This is a decryption project; this rule is what keeps it legal and safe.

- **No device keys, processing keys, media keys, `KEYDB.cfg`, host certs, Volume
  IDs, or unit keys** in the repo — not in code, tests, fixtures, history, or
  issues. They are supplied at runtime and live only in git-ignored `fixtures/`
  (referenced via `$FREEBLUE_FIXTURES`) — spec 09 §9.6.
- **No copyrighted media**, encrypted or decrypted — not even one Aligned Unit
  as a "convenient" vector. KATs that need real disc data are `#[ignore]`d and
  load from `$FREEBLUE_FIXTURES`.
- **`res/` is git-ignored** (it holds keys, copyrighted PDFs, and conference
  media); only `res/README.md` is tracked.
- The `.gitignore` enforces the patterns; **do not weaken it**. If you must add a
  fixture format, add its ignore rule first.

---

## Clean-room discipline

- Build only from **public** AACS specs, NIST/RFC standards, the 37c3 research,
  and **behavioral** observation of MakeMKV (inputs/outputs, never its source).
- `libaacs` may be *read* to confirm an algorithm, but **its code is not copied**
  — `freeblue` is all-original (spec 08 §8.6, spec 10 §10.2).
- Constants/protocol facts are not copyrightable; code is. Keep the paper trail
  (cite inline) so originality is provable.

## When the rules conflict with speed

They will. A "quick fix without a test" in demux/decryption/timestamp code is the
most expensive shortcut possible — output corruption is silent. **Slow down.
Write the test.** If a deadline forces a shortcut, document it as known debt in
the commit and open an issue.
