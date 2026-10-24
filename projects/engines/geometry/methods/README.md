# Method notes

This folder holds the method notes of the geometry kernel's method-only procedure (chapter 29.20, rule 6, of the [kernel architecture](../architecture/29-sources-licences.md)). A method note is how a method reaches Spare Kit from source code whose licence lets us read it but never take it: LGPL code, and GPL code a kernel owner has allowed reading in chapter 29's row (29.2). The note carries the method in plain words; the code is then written from the note and the papers by a session that never had the copyleft source open.

## The procedure, step by step

Rule 6 of chapter 29.20 is the authority; this is how each step leaves a record here.

1. **Look for a paper or a permissive source first**, and use that instead if it exists (JTS rather than GEOS; papers rather than SISL). A note is written only when the copyleft source is the best description of the method.
2. **One session reads the source.** A session is one person, or one agent conversation. It pins the commit and lists the files read, with line ranges. The note's `Pinned link`, `Files read` and `Read by` fields hold that record.
3. **That session writes the note in plain words**, from [TEMPLATE.md](TEMPLATE.md): the problem, the method, the invariants, the failure cases and the published sources the method rests on. No code, and no pseudocode that follows the source line by line. It names nothing from the source except public interfaces and paper names. Its only constants are published ones, or ones derived and justified in the note. Where a note is written from a stored evidence report that the reading session wrote in plain words, rather than by the reading session itself, the `Written by` field says so, and the note claims nothing the report does not support.
4. **A second reviewer checks the note against the source for copied expression** (not for method), then commits it. Here that review is the approval of the pull request that adds the note: the `Second review` field names that pull request's piece, and no code is written from a note before its pull request is approved.
5. **A different session writes the code** from the note and the papers, without the copyleft source open or in its context. The module's header cites the paper and the note: its `Consulted:` record for the source carries a line ``Method note: `projects/engines/geometry/methods/<note>.md` `` (see [HEADERS.md](../HEADERS.md)). The pull request names that session and states that the source was not in its context; the pull-request template's session attestation asks for exactly that. The note's `Code written from it` field records the module and the session.
6. **Tests come from our own understanding.** Test files and test vectors from the copyleft project are never copied: they are under its licence too.

## What checks it

`cargo run -p geom-test --bin provenance -- check` (CI runs it; `-- write` regenerates) reads every note here and fails, naming the note, when:

- the note does not open with `# Method note: <title>`, or a field of the template is missing, written twice, left as the template's `<placeholder>`, or states nothing;
- the licence is not one chapter 29.2 classifies as method only (LGPL) or owner only (GPL), the two classes rule 6 serves;
- the pinned link does not name a full commit;
- a required section is missing, out of order or empty;
- the note holds a fenced or indented code block (rule 6, step 3: "no code");
- a module's `Consulted:` record names the note but credits a different licence or another repository than the note's.

Every note, and every module that names it, is listed in [PROVENANCE.md](../PROVENANCE.md). `TEMPLATE.md` is held to the same fields and sections, so the template and the check cannot drift apart.

## The notes

| Note | Source | Why |
| --- | --- | --- |
| [occt-history-contract.md](occt-history-contract.md) | OpenCascade's `BRepTools_History` | Chapter 20.3 adopts its history contract as a tested invariant |
| [freecad-naming-lessons.md](freecad-naming-lessons.md) | FreeCAD's element map | Chapter 20 takes its idea of names built from operations and sources, and bans its sources of drift |
| [cgal-nextafter-widening.md](cgal-nextafter-widening.md) | CGAL's `FPU.h` | `geom-num`'s interval module consulted it for outward widening in round-to-nearest |
