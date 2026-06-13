# res/ — reference material (git-ignored)

This directory holds large, third-party, and/or sensitive reference material used
to write and validate the specs. **Everything here except this file is
git-ignored** (see `.gitignore`) because it contains keys, copyrighted media, or
redistributable-but-bulky documents (CLAUDE.md Rule 4, spec 10 §10.4).

To repopulate it, fetch the public sources below.

## Public documents (safe, redistributable)

| File | Source |
|------|--------|
| `AACS_Spec_Common_0.91.pdf` | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_Common_0.91.pdf |
| `AACS_Spec_BD_Prerecorded_Final_0_953.pdf` | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_BD_Prerecorded_Final_0_953.pdf |
| `AACS_Spec_Prerecorded_Final_0.953.pdf` | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_Prerecorded_Final_0.953.pdf |
| `sgxfail24.pdf` (SGX.Fail SoK paper) | https://www.cs.purdue.edu/homes/clg/files/sgxfail24.pdf |
| `libaacs_crypto.c` (reference oracle) | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/crypto.c |
| `libaacs_mkb.c` | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/mkb.c |
| `libaacs_aacs.c` (oracle: disc-id, VID/MK/VUK cache flow — spec 06 §6.8) | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/aacs.c |
| `libaacs_keydbcfg.c` (oracle: `keycache` file format — spec 06 §6.8) | https://code.videolan.org/videolan/libaacs/-/raw/master/src/file/keydbcfg.c |
| 37c3 "AACSess" talk + transcript | https://media.ccc.de/v/37c3-12296-full_aacsess_exposing_and_exploiting_aacsv2_uhd_drm_for_your_viewing_pleasure |
| `arch-deck-extracted.txt` | `pdftotext -layout` of the 2014 AACS LA architecture draft deck |

Extracted text (`common.txt`, `bd.txt`) is produced with
`pdftotext -layout <pdf> <txt>`.

## Sensitive material (NEVER commit; keep here only locally)

- `keydb_eng.zip` / `keydb.cfg` — community key database (contains keys).
- `MKB20_v82_*.tgz`, `MKB_RO.inf` — real disc structure dumps.

These are inputs for local validation only. Anything key- or media-bearing stays
out of git permanently.
