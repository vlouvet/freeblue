# fixtures/ — local KAT fixtures (git-ignored)

Real disc structures and keys used by the `#[ignore]`d known-answer tests
(spec 09 §9.6). **Everything here except this file is git-ignored** and must
never be committed (CLAUDE.md Rule 4).

Tests that need these load them from the path in `$FREEBLUE_FIXTURES` (default:
this directory) and **skip loudly** when it is unset — they never silently pass.

Expected contents (you provide these locally):

| Fixture | Used by | Notes |
|---------|---------|-------|
| `got_mkb.inf` | `freeblue-mkb` | a real `MKB_RO.inf` |
| `got_pk_mkbv63.key` | `freeblue-mkb` | the matching processing key (hex) |
| `got_unit0.bin` | `freeblue-content` | one real encrypted 6144-B Aligned Unit |
| `got_unit_key.key` | `freeblue-content` | the disc's CPS Unit Key (hex) |
| `got_expected.json` | both | expected media key / TS-sync result |

Run them with `cargo test -- --ignored` once `$FREEBLUE_FIXTURES` is populated.
