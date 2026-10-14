# res/ — reference material (git-ignored)

In the repository this directory contains only this file. Everything else in
it is git-ignored (see `.gitignore`) and stays on contributors' machines,
because it is third-party, bulky, or sensitive (CLAUDE.md Rule 4, spec 10
§10.4, §10.7). The specs cite these documents by the tags shown.

## Public documents

Contributors can fetch these to check the specs' citations.

| File | Cited as | Source |
|------|----------|--------|
| `AACS_Spec_Common_0.91.pdf` | `[CCE]` | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_Common_0.91.pdf |
| `AACS_Spec_BD_Prerecorded_Final_0_953.pdf` | `[BD]` | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_BD_Prerecorded_Final_0_953.pdf |
| `AACS_Spec_Prerecorded_Final_0.953.pdf` | | https://aacsla.com/wp-content/uploads/2019/02/AACS_Spec_Prerecorded_Final_0.953.pdf |
| `sgxfail24.pdf` (SGX.Fail SoK paper) | | https://www.cs.purdue.edu/homes/clg/files/sgxfail24.pdf |
| 37c3 "AACSess" talk recording | `[Talk mm:ss]` | https://media.ccc.de/v/37c3-12296-full_aacsess_exposing_and_exploiting_aacsv2_uhd_drm_for_your_viewing_pleasure |
| `talk-transcript.txt` | `[Talk mm:ss]` | transcript of the talk above |
| `libaacs_crypto.c` | `[libaacs]` | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/crypto.c |
| `libaacs_mkb.c` | `[libaacs]` | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/mkb.c |
| `libaacs_aacs.c` (disc ID, VID/MK/VUK cache flow; spec 06 §6.8) | `[libaacs]` | https://code.videolan.org/videolan/libaacs/-/raw/master/src/libaacs/aacs.c |
| `libaacs_keydbcfg.c` (`keycache` file format; spec 06 §6.8) | `[libaacs]` | https://code.videolan.org/videolan/libaacs/-/raw/master/src/file/keydbcfg.c |

`libaacs` source is a read-only reference; no code is copied from it (spec 08
§8.6).

Text extracts used for citation are made with `pdftotext -layout <pdf> <txt>`:
`common.txt` from the CCE book and `bd.txt` from the BD book.

The specs also cite `[Arch]`, a 2014 AACS LA architecture draft deck, via a
text extract `arch-deck-extracted.txt`. It is not redistributed here.

## Sensitive material (never commit)

Contributors validating against real discs may also keep local inputs here,
such as a key database or dumps of a disc's `AACS/` structures. These contain
keys or disc data. They are for local validation only and must stay out of git
permanently; tests read their real-disc inputs from `$FREEBLUE_FIXTURES`
instead (see [`../fixtures/README.md`](../fixtures/README.md)).
