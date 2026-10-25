# Licence texts of ported sources

One file per outside source the geometry kernel ports code from (PORT, chapter 29.1 of the [kernel architecture](../architecture/29-sources-licences.md)). A ported file's header copies its origin's copyright lines (its `Source:` record, [HEADERS.md](../HEADERS.md)); the licence text in full, and any further credit, live here, because chapter 29.20, rule 4, asks that `NOTICE` "holds every origin licence text and every credit". [`NOTICE`](../NOTICE) and [`PROVENANCE.md`](../PROVENANCE.md) are generated from the headers and these files by `cargo run -p geom-test --bin provenance -- write`; never edit those two by hand.

## The format

A file named `<source>.txt`:

```text
Name: <the source's name>
Project: <its home page; optional>
Link: <its repository, pinned to the full commit the ports were taken from>
Licence: <its SPDX expression, on chapter 29.2's allow-list>
Licence file: <its licence file, pinned to the same repository and commit>

Credit:
<further credit owed, as plain text; optional>

Licence text:
<the licence file's text, copied exactly>
```

`Credit:` holds any further credit the origin asks for. For an origin under Apache-2.0 whose repository has a `NOTICE` file, such as NVIDIA's USD BRep release that decision 32.8 ([chapter 32](../architecture/32-decisions.md)) lets the kernel port, the attribution notices of that file are copied here exactly, as the Apache License's section 4(d) requires of a derivative work.

`cargo run -p geom-test --bin provenance -- check` (CI runs it) fails when a key is missing, repeated, unknown or empty, a link is not pinned to a full commit, the licence file is in another repository or at another commit, the licence is not on the allow-list, or the licence text is missing; when the ports of one repository at one commit (every `Source:` record whose link is in it) have no licence text here, or more than one; when a file's licence differs from its ports'; and when no port uses a file. So a new port cannot ship without its licence text, and a licence text cannot outlive its port.

## The files

| File | Source |
| --- | --- |
| [core-math.txt](core-math.txt) | CORE-MATH (Inria), ported into `geom-num`'s `math` module by piece G05 |
