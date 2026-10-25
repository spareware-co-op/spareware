## Piece

`<ID>`: <piece title>

Closes #<issue number>

## What changed

-

## How it was checked

- Tests and benchmarks run (Build and Harden stages), or sources read (Mapping and Design stages):

## Outside sources used

Every open project, paper, standard or page you used, one row each, with its licence where it is code. For the geometry kernel this is the fixed form of chapter 29.20, rule 5: reviewers compare this table with the `Source:` and `Consulted:` lines in the headers, and CI compares the headers with `projects/engines/geometry/PROVENANCE.md`. Link code pinned to a full commit. Class is one of chapter 29.1's: DEPEND, PORT, PORT METHOD, ORACLE or STUDY. Write "None" if nothing outside was used.

| Source | Pinned link | Licence | Class (29.1) | Files affected | What was taken |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

## Session attestation

Fill this in when the pull request reads or uses an LGPL source, or a GPL source a kernel owner allowed, under the method-only procedure (chapter 29.20, rule 6). A session is one person, or one agent conversation. Delete it otherwise.

- Method notes added or changed (`projects/engines/geometry/methods/`), and the session that read each source:
- Code written from a method note: each module, the session that wrote it, and the statement that the copyleft source was not in its context (it was not open, read or pasted in that session):
- [ ] The session that wrote the code from a method note is not the session that read the source, and the source was not in its context.
- [ ] Reviewer: as the second reviewer, I checked each new or changed method note against its source for copied expression, not for method (rule 6, step 4).

## Checklist

- [ ] I had this piece checked out (`python tools/piece.py checkout <ID>`) and stayed inside it.
- [ ] Every commit is signed off with the Developer Certificate of Origin (`git commit -s`).
- [ ] Built legally: nothing copied from proprietary software, no reverse engineering, licences respected, no GPL or AGPL code in the core.
- [ ] Geometry kernel: `cargo run -p geom-test --bin source-scan -- check` and `cargo run -p geom-test --bin provenance -- check` pass, and every outside source above is in a header's `Source:` or `Consulted:` line.
- [ ] Interfaces follow `design/DESIGN.md` and use `design/tokens.css`.
- [ ] Plain words, no em or en dashes, no secrets or personal data.
