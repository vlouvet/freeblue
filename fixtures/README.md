# fixtures/ — real-disc test inputs (git-ignored)

In the repository this directory contains only this file. Everything else in
it is git-ignored and must never be committed (CLAUDE.md Rule 4). The
`.gitignore` also ignores a top-level `FREEBLUE_FIXTURES/` directory, which
can be used instead.

The `#[ignore]`d tests that need real disc data read their inputs from the
directory named by `$FREEBLUE_FIXTURES` (spec 09 §9.6). There is no default:
if the variable is unset, those tests fail with a message rather than pass.

```sh
FREEBLUE_FIXTURES=/path/to/your/fixtures cargo test -- --ignored
```

Files the tests currently read, all supplied by you from a disc you own and
your own key database:

| File | Test | Contents |
|------|------|----------|
| `keydb.cfg` | `freeblue-keys`: `parses_real_keydb_at_scale` | a `KEYDB.cfg`-format key database (structure is checked; no key values are asserted) |
| `Unit_Key_RO.inf` | `freeblue-keys`: `disc_id_matches_keydb` | a disc's `AACS/Unit_Key_RO.inf` |
| `disc_id.hex` | `freeblue-keys`: `disc_id_matches_keydb` | that disc's 20-byte disc ID (SHA-1 of the file above) as hex |
| `discatt.dat` | `freeblue-keys`: `parse_discatt_real_fixture` | a `discatt.dat` written by MakeMKV for a disc |

Three more ignored tests are placeholders with no body yet, so they pass
without reading anything: `freeblue-mkb` `derive_media_key_matches_keydb`,
`freeblue-content` `aligned_unit_decrypts_to_valid_ts`, and `freeblue-content`
`bus_then_content_decrypts_bee_unit_to_valid_ts`. Their real-disc results are
recorded in specs 03, 05 and 11, not reproduced by the test suite.
