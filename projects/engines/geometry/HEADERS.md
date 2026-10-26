# Header grammar for the geometry kernel

Every crate and every module of the geometry kernel opens with a header that says what it does, what it guarantees, the published method it follows and what it costs (chapters 26.3 and 27 of the kernel architecture, principle 10). This page is the grammar those headers are written in. Two checks read it:

- the map generator, which turns every header into [`MAP.json`](MAP.json) and fails on a header that is incomplete: `cargo run -p geom-test --bin map -- check` (CI runs it in `.github/workflows/kernel-dev.yml`), and `cargo run -p geom-test --bin map -- write` to regenerate the map after changing a header;
- the scaffold check of piece G09 (`projects/engines/crates/engine-geometry/tests/scaffold/header.rs`), which looks for the same four crate fields (job, layer, invariants, sections) in the same places. The map finds each of the four exactly where G09's reader finds it, so a header states a field for one check exactly when it states it for the other (one difference: the map skips a leading byte-order mark, which G09's reader does not, so a file that opens with one fails the scaffold check); the map then also checks what the fields hold (a job and every invariant state something, the layer is in range and fits the crate's kind), which G09's reader leaves to its crate table. G09's reader is a test of its own piece and is not changed here; the piece that next edits the scaffold check can call this reader instead.

A field counts only when it is written in this grammar, with one of the labels below, so a header cannot pass by mentioning a word in passing, by a label that denies the field, or by a placeholder. Each rule below has seeded good and bad fixtures in `crates/geom-test/tests/map_seeded.rs`, and `crates/geom-test/tests/map_repo.rs` fails if this page stops naming a rule, a label, a table or a limit the code uses.

## Where headers go

| File | Header |
| --- | --- |
| `src/lib.rs` of every crate under `projects/engines/geometry/crates/`, and of the facade `engine-geometry` and its binding crates `engine-geometry-*` under `projects/engines/crates/` (or `src/main.rs` of a crate with no library) | A crate header. A crate root that defines anything besides `mod` and `use` lines (a type, a function, a constant, an `impl`, a macro) is a module too, and its header also carries the module fields |
| Every other `.rs` file under `src/`: modules (`src/a.rs`, `src/a/mod.rs`, `src/a/b.rs`), `src/main.rs` beside a library, binaries (`src/bin/<name>.rs`, `src/bin/<name>/main.rs`) and their modules (`src/bin/<name>/<module>.rs`, mapped as `<name>::<module>`) | A module header |
| `tests/`, `benches/`, `examples/`, `build.rs` | None required: they are not part of the crate's map |

A header is the run of `//!` lines that opens the file, written at the start of the line. The first line that does not start with `//!` ends it, so the header must come first: before `#![...]` attributes, `use` lines and plain `//` comments. A leading byte-order mark is skipped. Each line is read without its `//!` and the one space after it.

The crate folder's name is the crate's name (chapter 27), its `Cargo.toml` names that package, and no target moves its root with `path =`, because the map finds crate roots where Cargo finds them by default. A source file or crate folder that is a symbolic link (or a Windows junction) is reported, never followed or skipped, and so is a folder more than 8 folders below `src/`.

## Reading a header

- **Paragraph:** a run of non-blank lines that are neither list items nor provenance records. A fenced code block (```` ``` ```` to ```` ``` ````) belongs to the paragraph or list item it is in, and nothing inside it is read as a field.
- **List:** lines that start with `- `. A line that starts with a space continues the item above it, even after a blank line; nested items are part of their parent item. A list ends at a line that is neither.
- **Provenance record:** a line that starts with `Source: `, `Consulted: ` or `SPDX-License-Identifier: `, and the lines after it that start with a space. A record ends the paragraph above it, so records may follow a paragraph without a blank line.
- **Label:** the text of a paragraph before its first colon. A field is a paragraph that opens with its label: `Cost: constant.` is a cost; `This has a cost: none.` is not.
- **Sentence** (for the crate fields that G09 reads as sentences): a sentence opens a run of non-blank lines or follows ". ", and runs to the next ". " or the end of its run.
- **States something:** a field's text has a letter or digit and is not, in any case and without its full stop, one of the placeholders `none`, `nothing`, `n/a`, `na`, `tbd`, `tba`, `tk`, `todo`, `fixme`, `xxx`, `unknown`, `same`, `see above` or `as above`.

## Crate headers

| Field | Required | Exact syntax | MAP.json |
| --- | --- | --- | --- |
| Summary | Yes | The first paragraph: for kernel crates `Spare Kit geometry kernel: <topic>.`; for development crates `Spare Kit geometry kernel, development only: <topic>.`; for the facade and its binding crates, as for every shared engine, `Shared engine: <topic>.`. The topic states something | `summary` |
| Job | Yes | A sentence opening with `One job: `, `Job: `, `Job (` or `Its one job is `, followed by the job, which states something. After `Job (`, the job is what follows the closing `): ` | `job` |
| Layer | Yes | The first sentence opening with `Layer`, an optional colon and a space, then a word that is `Development` or a number. The number is from 1 to 13 of chapter 27's layer table; the facade and its binding crates are 13; the development crates (`geom-test`, `geom-fuzz`, `geom-gen`, `geom-bench`) say `Development` | `layer` (as written) |
| Invariants | Yes | The first line that opens a paragraph (the header's first line, or one after a blank line) and starts with `Invariants`, with a `- ` item in the rest of its paragraph or as the first line of the next; that list's items are the invariants, each stating something | `invariants` |
| Sections | Yes | A sentence opening with `Sections: ` or `Map: ` that names at least one chapter: a number from 1 to 32 that does not continue a word, a number or a section number (`G08` and the `.6` of `15.6` are not chapters) | `sections`, and `chapters`, the chapters named |
| Family | For the crates in the table below; otherwise when the crate's job is a catalogue family | A paragraph `Family: ` and one or more families of chapter 10.2's catalogue, separated by `, ` or ` and `, ending in a full stop | `family` |
| Status | No | G09's line `Status: scaffold only; built by pieces ...` and its continuation lines | `status` |
| Licences | When a kernel or facade crate's manifest names a published crate (DEPEND, chapter 29.1) | A paragraph starting `Licences`, then one `- <licence>: `package`, `package`.` item per licence, as the development crates write theirs (`geom_test::licence::records`). Each published crate in `[dependencies]` or `[build-dependencies]` (any target) has a record, under a licence met by chapter 29.2's allow-list | `depends` |
| Provenance | When the crate root took or consulted an outside source | As for modules, below | `provenance`, `spdx` |

The three summary openings are `Spare Kit geometry kernel: `, `Spare Kit geometry kernel, development only: ` and `Shared engine: `. G09's scaffold lines (the status line, the pending-dependency lines and the proposal marks) are checked by the scaffold check, not by the map.

**The families of the crates.** A crate whose job in chapter 27's crate table is the operations of a family's row in chapter 10.2's catalogue states that family. Each row below matches the crate's job to the row's words (geom-make's job names its two families outright):

| Crate | Family |
| --- | --- |
| `geom-2d` | 2D |
| `geom-align` | Ground |
| `geom-analysis` | Query |
| `geom-blend` | Blend |
| `geom-bool` | Combine |
| `geom-combine` | Combine |
| `geom-convert` | Free-form |
| `geom-draw` | Draw |
| `geom-field` | Free-form |
| `geom-heal` | Repair |
| `geom-make` | Make and Transform |
| `geom-mesh` | Mesh |
| `geom-offset` | Offset |
| `geom-panel` | Free-form |
| `geom-profile` | Sketch |
| `geom-skeleton` | Ground |
| `geom-sketch` | Sketch |
| `geom-spatial` | Query |
| `geom-subd` | Free-form |
| `geom-sweep` | Sweep and Manufacture |
| `geom-terrain` | Ground |

The families of chapter 10.2's catalogue, as the `Family` field writes them: `Make`, `2D`, `Sketch`, `Sweep`, `Combine`, `Blend`, `Offset`, `Transform`, `Repair`, `Query`, `Draw`, `Mesh`, `Manufacture`, `Free-form`, `Ground`. Every family has a crate above, so an agent asked to "add a fillet option" finds `geom-blend` by its family in `MAP.json` (26.3). A module states its family too once the operation contract lands (`geom-op`), when the check can tell which modules define operations.

```rust
//! Spare Kit geometry kernel: fillets, chamfers and blends.
//!
//! One job: Fillets, chamfers, vertex blends.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never grows a tolerance, retries with nudged inputs, or stitches a result afterwards so it
//!   passes the checker (13.21).
//!
//! Sections: 13.10, 13.21 and 13.22; open question OQ-13.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Blend.
```

## Module headers

| Field | Required | Exact syntax | MAP.json |
| --- | --- | --- | --- |
| Summary (the job) | Yes | The first paragraph, one or more sentences ending in a full stop that say what the module is for; it states something and is not a field (a first paragraph `Guarantees: ...` states no job) | `job` |
| What it does | No | A paragraph opening `What it does: ` | `does` |
| Guarantees | Yes | A paragraph labelled `Guarantees` (write this one), and then a `- ` list, one guarantee per item, or the guarantee itself after the colon; each guarantee states something. The other labels: `What it guarantees`, `What this module guarantees`, `What this crate guarantees`, `What every function here guarantees` and `What each function here guarantees`, and, for a crate root that is a module, `Invariants`, `Invariants it keeps`, `Invariants this module keeps` and `Invariants this crate keeps`. Each may be followed by `, ` and a qualifier (`Guarantees, for every function here and every argument:`); no other label counts (`Guarantees nothing:` and `What this module never guarantees:` are not the field) | `guarantees` |
| Method | Yes | A paragraph labelled `Method`, `Methods`, `Method followed`, `Methods followed` or `Method (<qualifier>)`. The method is the paragraph after the colon and a `- ` list that directly follows it (a reference list). It names at least one link: a web address (bare, in `<...>` or in a Markdown link; on `doi.org`, a whole DOI), a DOI written `doi:10.<registrant>/<suffix>` (a registrant code of digits with sub-divisions after full stops, a `/`, and a suffix that is not empty), or a path in this repository in backticks under `projects/engines/geometry/` or `projects/kit/notes/` that ends in `.md` or `/` (an architecture chapter or an evidence note) and exists, with exactly that name, in the tree the map reads | `method`: `links` and `text`, or `none` and the reason |
| | | `Method: none; <the reason>.`, where no published method applies, fits only a module that defines no function outside trait implementations (types, a table of constants, an error enum and its `Display` and `Error` implementations) or a binary and its modules (argument parsing). A module that defines a function implements an algorithm and names its method | |
| Cost | Yes | A paragraph opening `Cost: `, followed by the cost (time, memory, allocation, recursion), which states something | `cost` |
| Family | No | As for crates | `family` |
| Provenance | When the module took or consulted an outside source | Below | `provenance`, `spdx` |
| Recursion | When the module holds a function of a recursive cycle, or defines a recursive type | [The `Recursion` field](#the-recursion-field) below | Not in the map: the recursion check reads it |

Each module field is written once; a second `Guarantees`, `Method`, `Cost` or `Family` paragraph is a problem rather than a silent choice. Other paragraphs (`Why: `, derivations, tables, notation) are prose: the map leaves them out.

```rust
//! Error-free transformations: a rounded sum or product of two doubles and its exact error.
//!
//! What it does: ...
//!
//! Guarantees:
//!
//! - **Exact or reported, never silently wrong.** ...
//!
//! Method: Dekker 1971 and Knuth, in the form and with the proofs of Shewchuk 1997.
//!
//! - J. R. Shewchuk, "Adaptive precision floating-point arithmetic and fast robust geometric
//!   predicates", *Discrete & Computational Geometry* 18 (1997), 305–363,
//!   <https://doi.org/10.1007/PL00009321>.
//!
//! Cost: constant time and no allocation.
```

## The `Recursion` field

WebAssembly has no guard page, so recursion that runs too deep corrupts memory instead of trapping; every recursive cycle in a crate that ships therefore states its depth bound (chapter 28.8; the rule follows solidcore's, G01 evidence 13). The recursion check (`geom-test`'s `src/recursion`, piece G13) finds every cycle on the program's call graph and reads this field from the headers of the modules that hold them.

```rust
//! Recursion:
//!
//! - Cycle: `Tree::walk`, `walk_children`.
//!   Bound: 64 (`MAX_DEPTH`).
//!   Why: each call goes one level down the tree, and `Tree::walk` refuses past `MAX_DEPTH`.
//!   Test: `walk_at_its_bound`.
//! - Types: `Expr`.
//!   Bound: 32 (`MAX_NESTING`).
//!   Why: the parser refuses an expression nested deeper than `MAX_NESTING`.
//!   Test: `deepest_expr_drops_and_clones`.
```

| Part | Exact syntax |
| --- | --- |
| `Cycle:` | Every function of the cycle, in backticks, as the module names them (relative to it, from `crate::`, or through its `use` declarations); a closure is named by the function it is written in, an implementation's method as `Type::method` or `<Type as Trait>::method`. Exactly the cycle's members: `recursion list` prints them |
| `Types:` | Instead of `Cycle:`, the structs, enums or unions that contain themselves through their fields: their drop glue and derived implementations recurse over their nesting, so one statement covers those derived cycles too |
| `Bound:` | The most frames of the cycle that are ever on the stack at once (for types, the deepest nesting of their values): a positive whole number, then the constant that enforces it in backticks when there is one (`const NAME: T = <integer literal>;` in the crate, whose value must be the bound) |
| `Why:` | Why the bound holds, as a sentence that states something |
| `Test:` | A `#[test]` function of the crate's `tests/recursion.rs`, not ignored, that runs the cycle at its bound; the check requires its call graph to reach the cycle |

Each item starts with `- Cycle: ` or `- Types: ` and each other part opens a line of its own; the field is written once per file, in the header of a module that holds one of the cycle's functions (or defines one of the types). `cargo run -p geom-test --bin recursion -- list` prints every cycle and recursive type with a path round it and the line of each call; `-- check` fails on a cycle without a statement (`Unbounded`), a statement that is not exactly one cycle's members (`Stale`), a statement in the wrong file or malformed (`Statement`), a bound constant that disagrees (`Bound`), a missing, ignored or unreaching test (`Test`), and code the check cannot read (`Unread`); `-- gate`, which the merge gate's recursion job runs, also runs every crate's `tests/recursion.rs` for `wasm32-wasip1` in debug and in release on a 256 KiB shadow stack and a 256 KiB engine stack (`Stack`).

## Provenance lines

These are the lines of chapter 29.20, rule 4. The map reads them into one record per source (`sources` in `MAP.json`). The same records produce [`PROVENANCE.md`](PROVENANCE.md), the ledger of every source the code takes, and [`NOTICE`](NOTICE), every ported source's licence text and copyright lines and the credit owed to every source consulted: `cargo run -p geom-test --bin provenance -- write` writes both from the map, the licence texts in [`notices/`](notices/README.md) and the method notes in [`methods/`](methods/README.md), and `-- check` (CI) fails when either differs, when a port has no licence text, or when a method note breaks its template (`geom_test::ledger`). Each record in a module's `provenance` list keeps its `notice`: for a `Source:` record, every line after the one holding its link and before `Changed for Spare Kit: ` (the origin's copyright lines, and the links of any further origin files with theirs), which `NOTICE` reproduces. Each origin file's copyright lines follow its link; a run of links with no line between them (two files of one author) shares the lines after the last link of the run, and an origin file left with no copyright line fails the ledger's check.

| Line | Syntax | Checked |
| --- | --- | --- |
| `Source: ` | `Source: PORT from <name>, <licence>,` then, on continuation lines: the link to the file ported, pinned to a full commit; the origin's copyright lines, copied exactly; and `Changed for Spare Kit: <what changed>`, which may run over several lines | Every part present. The licence is met by chapter 29.2's allow-list (below): GPL, LGPL, AGPL, non-commercial and custom-terms code is never ported, and public-domain code is taken as PORT METHOD, with a `Consulted:` record. At least one line before `Changed for Spare Kit: ` opens with `Copyright`, `©`, `(c)` or `SPDX-FileCopyrightText:` (any case) and goes on to name a year or a holder. The link is pinned |
| `Consulted: ` | `Consulted: <name>, <licence>, <link>, <what it was consulted for>`, over as many continuation lines as needed; the link is a web address or a DOI. The name may contain commas; the licence never does. A source read under the method-only procedure adds a continuation line `Method note: ` and the note's path in backticks, ``Method note: `projects/engines/geometry/methods/<note>.md` `` | The licence is classified by chapter 29.2 (below), so a word that is no licence (`TBD`, `unknown`, a file name) fails. A link on a code-hosting site is pinned to a full commit. The purpose states something. Code (below) under a licence 29.2 never lets us read fails; LGPL code, and GPL code an owner allowed in chapter 29's row, carries its method note (29.20, rule 6), and the note exists |
| `SPDX-License-Identifier: ` | `SPDX-License-Identifier: <SPDX expression>` | It parses. A header with a `Source:` line has one, met by the allow-list, and each ported licence is that expression or one of its `AND` operands. A header with no `Source:` line is ours: its line, if any, is `MPL-2.0` (29.20, rule 4: "A PORT METHOD module is MPL-2.0 and ours") |

- A licence may be written in backticks (`` `LGPL-3.0-or-later OR LicenseRef-Commercial` ``), as Clippy's `doc_markdown` lint asks of an identifier; the backticks are not part of it.
- **The crate's licence.** A crate with a ported file names each ported licence beside MPL-2.0 in its own `Cargo.toml` `license` field (29.20, rule 4: `license = "MPL-2.0 AND BSD-3-Clause"`); `license.workspace = true` does not.
- **One record per source.** Records are merged by how the source was taken and its link without the fragment, so one file at one commit, cited at two line ranges, is one source. Two headers that give one source different licences are a problem in each (29.20, rule 1: where licences disagree, the strictest applies; stop and ask an owner).
- **Every link to outside code is credited.** A link to source code anywhere else in the header, or in any doc comment of the file outside `#[cfg(test)]` items, needs a `Source:` or `Consulted:` record in the same header for the same repository (on a code-hosting site) or the same file (elsewhere). Links to papers, standards, documentation and a project's home page need none. A commit named by an abbreviated name (the word "commit" and seven to 39 hexadecimal digits) is a source read but not credited: write a record whose link names the full commit.

**Chapter 29.2's licence classes.** Every identifier below is on the SPDX License List 3.29.0. An identifier in none of them is not classified, and the record fails until chapter 29.2 has a row for it and this table a class (29.20, rules 1 and 11). An `OR` is read as its most permissive operand (rule 3), an `AND` as its strictest (rule 1); `WITH` an exception changes neither the LGPL nor the GPL class ("with or without exceptions"), and on the allow-list only `Apache-2.0 WITH LLVM-exception` is a `WITH` form.

| Class | What 29.2 allows | Identifiers |
| --- | --- | --- |
| Allowed | Port with the notice, depend, read | `MIT`, `MIT-0`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `BSL-1.0`, `ISC`, `Zlib`, `0BSD`, `Unlicense`, `CC0-1.0`, `Unicode-3.0`, `Unicode-DFS-2016`, `MPL-2.0` |
| Readable | Read and cite, never port | `public domain` (taken as PORT METHOD), and the documentation and data licences 29.2 and rule 9 name: `CC-BY-3.0`, `CC-BY-4.0`, `CC-BY-SA-4.0`, `CC-BY-ND-4.0` |
| Method only | Read only under the method-only procedure, with a method note | `LGPL-2.0-only`, `LGPL-2.0-or-later`, `LGPL-2.1-only`, `LGPL-2.1-or-later`, `LGPL-3.0-only`, `LGPL-3.0-or-later`, and the deprecated `LGPL-2.0`, `LGPL-2.0+`, `LGPL-2.1`, `LGPL-2.1+`, `LGPL-3.0`, `LGPL-3.0+` |
| Owner only | Not read unless a kernel owner allowed it; then the method-only procedure, with a method note | `GPL-1.0-only`, `GPL-1.0-or-later`, `GPL-2.0-only`, `GPL-2.0-or-later`, `GPL-3.0-only`, `GPL-3.0-or-later`, and the deprecated `GPL-1.0`, `GPL-1.0+`, `GPL-2.0`, `GPL-2.0+`, `GPL-3.0`, `GPL-3.0+`, `GPL-2.0-with-GCC-exception`, `GPL-2.0-with-autoconf-exception`, `GPL-2.0-with-bison-exception`, `GPL-2.0-with-classpath-exception`, `GPL-2.0-with-font-exception`, `GPL-3.0-with-GCC-exception`, `GPL-3.0-with-autoconf-exception` |
| Never read | AGPL, non-commercial and custom terms ("Read the source? No") | `AGPL-1.0`, `AGPL-1.0-only`, `AGPL-1.0-or-later`, `AGPL-3.0`, `AGPL-3.0-only`, `AGPL-3.0-or-later`; `CC-BY-NC-1.0`, `CC-BY-NC-2.0`, `CC-BY-NC-2.5`, `CC-BY-NC-3.0`, `CC-BY-NC-3.0-DE`, `CC-BY-NC-3.0-IGO`, `CC-BY-NC-4.0`, `CC-BY-NC-ND-1.0`, `CC-BY-NC-ND-2.0`, `CC-BY-NC-ND-2.5`, `CC-BY-NC-ND-3.0`, `CC-BY-NC-ND-3.0-DE`, `CC-BY-NC-ND-3.0-IGO`, `CC-BY-NC-ND-4.0`, `CC-BY-NC-SA-1.0`, `CC-BY-NC-SA-2.0`, `CC-BY-NC-SA-2.0-DE`, `CC-BY-NC-SA-2.0-FR`, `CC-BY-NC-SA-2.0-UK`, `CC-BY-NC-SA-2.5`, `CC-BY-NC-SA-3.0`, `CC-BY-NC-SA-3.0-DE`, `CC-BY-NC-SA-3.0-IGO`, `CC-BY-NC-SA-4.0`, `PolyForm-Noncommercial-1.0.0`, `PolyForm-Small-Business-1.0.0`; and any `LicenseRef-` |

The never-read class fails a `Consulted:` record only when its link is code: 29.2 lets a paper under a non-commercial licence be read and cited.

**Code and code hosts.** A link is on a code-hosting site when its host is `github.com`, `gist.github.com`, `raw.githubusercontent.com`, `gist.githubusercontent.com`, `codeberg.org`, `gitee.com`, `bitbucket.org`, `notabug.org`, `git.sr.ht`, `sourceforge.net`, `gitlab.com`, `salsa.debian.org`, `invent.kde.org`, `framagit.org` or `foss.heptapod.net`, starts with `gitlab.`, `gitea.`, `forgejo.`, `cgit.` or `git.`, or ends with `.googlesource.com`; or when its path or query has one of these layouts on any host. It is **pinned** when the revision slot of its layout, and only that slot, is a full Git commit name: 40 hexadecimal digits for SHA-1 (64 for SHA-256), in lower case. An abbreviated commit, a branch or `HEAD` is not pinned.

- GitHub's layout, shared by Gitea, Forgejo, Codeberg, Gitee, Bitbucket, sourcehut and SourceForge: `<owner>/<repository>/<view>/<revision>/<path>`, the view one of `blob`, `tree`, `raw`, `blame`, `commit`, `commits`, `src`, `plain`, `log`, `diff`, `patch`, `ci` or `snapshot` (Gitea's `src/commit/<revision>` included); `raw.githubusercontent.com/<owner>/<repository>/<revision>/<path>`; gists, `gist.github.com/<user>/<id>/<revision>`.
- GitLab's, on any host: `<group>/.../<repository>/-/<view>/<revision>/<path>`.
- Gitiles' (`*.googlesource.com`, any Gerrit host): `<repository>/+/<revision>/<path>`, and its other `+` commands.
- cgit's: `<repository>/<view>/<path>?id=<revision>`.
- gitweb's, on any host: `?p=<repository>;a=<action>;f=<path>;hb=<revision>`.

A web address is **code** when it shows a file, folder, commit or diff in a repository on a code-hosting site, when it is a gist, or when its last path segment, or a value in its query, ends in a source file's extension (directly or before a viewer's `.html`, as in `.rs.html`): `.c`, `.h`, `.cc`, `.cpp`, `.cxx`, `.c++`, `.hpp`, `.hh`, `.hxx`, `.h++`, `.inl`, `.ipp`, `.tcc`, `.cu`, `.cuh`, `.cl`, `.glsl`, `.hlsl`, `.wgsl`, `.metal`, `.rs`, `.py`, `.pyx`, `.f`, `.f77`, `.f90`, `.f95`, `.f03`, `.f08`, `.for`, `.java`, `.js`, `.mjs`, `.ts`, `.go`, `.jl`, `.m`, `.mm`, `.cs`, `.swift`, `.kt`, `.scala`, `.hs`, `.ml`, `.lisp`, `.rb`, `.lua`, `.zig`, `.s`, `.asm`.

Chapter 29's two examples, in this grammar (the fields open paragraphs, so a blank line comes before `Method:`; the records may follow the method directly):

```rust
//! Semi-static filter generator for exact predicates.
//!
//! Method: Meyer and Pion's FPG (inria-00344297); bit budgets in section 6.2.
//! Source: PORT from Geogram PCK, BSD-3-Clause,
//!   https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/bin/pck
//!   <the origin's copyright lines, copied exactly>
//!   Changed for Spare Kit: integer-grid inputs, Rust output, constants checked against our bit budgets.
//! SPDX-License-Identifier: BSD-3-Clause
```

```rust
//! Knot removal for NURBS curves, with a bound on the error it introduces.
//!
//! Method: The NURBS Book (Piegl and Tiller, 2nd edition), chapter 5.
//! Consulted: NVIDIA USD BRep NLib, Apache-2.0, <pinned link to the file consulted>,
//!   for failure cases only. No code copied.
//! SPDX-License-Identifier: MPL-2.0
```

(A real header also states its guarantees and cost, and its method names a link.)

## Error kinds

The map lists every error kind of every kernel and facade crate (not the development crates), for the error catalogue of chapter 26.3 and the base kinds of chapter 22.

- **How they are found.** An error enum is a `pub enum`, declared at the top level of a module file outside any `#[cfg(test)]` item, that is an error type: its name contains `Error` (`NumError`, and chapter 22's `ErrorKind`, `OpErrorDetail` and `IntersectError` when they arrive), or its crate implements a trait named `Error` for it (`impl core::error::Error for SweepFailure`), or names it as the error of a `Result<_, E>` anywhere. Each variant is one error kind. The source is read as Rust tokens, so an enum in a comment, a string or a doc example is never found. `pub(crate)` and private enums are not the crate's interface and are not listed.
- **Why by name and by use.** Chapter 22 names the shared error types, but leaves the name of each family's detail enum (the enum a sweep's `MitreMismatch { .. }` sits in) to its piece. The name rule needs no marker that could be forgotten, the use rule finds every enum the code treats as an error, and the checks make both complete: a public error enum inside a function, an inline module or another block is a problem, never skipped.
- **What is recorded:** the enum's name, its module, the first paragraph of its doc comment, whether it is `#[non_exhaustive]` (chapter 22: the base kinds are not, the details are), and each kind's name, shape (`unit`, `tuple` or `struct`) and the first paragraph of its doc comment. Doc comments are `///` or `/** */` lines; `#[doc = "..."]` is not read.
- **Every kind is explained:** an error enum or a variant without a doc comment is a problem.

rustdoc's JSON output would give the same doc text with every item resolved, which is how a map could otherwise be made. It is not used: it needs a nightly compiler (`--output-format json` is unstable, tracking issue rust-lang/rust#76578), its format changes with every breaking release (`FORMAT_VERSION` 61 in `rustdoc-types` at commit `2e63bdc44b94e86fe12d6e0034d1ed9c2ac6e9dc`), and the kernel's toolchain is pinned to a stable release. Reading the source works with the pinned toolchain today and forever.

## The map

`MAP.json` is generated; never edit it by hand. Run `cargo run -p geom-test --bin map -- write` and commit the result.

| Key | Holds |
| --- | --- |
| `crates` | One entry per crate folder, by name: `name`, `folder`, `file` (its root), `kind` (`kernel`, `development` or `facade`), `summary`, `job`, `layer`, `invariants`, `sections`, `chapters`, `family`, `status`, `provenance`, `spdx`, `depends`, `modules` and `errors` |
| `crates[].modules` | One entry per module file (a crate root that holds code included), by path: `file`, `path` (`geom_num::interval`, a binary's name, or `<binary>::<module>`), `target` (`lib` or `bin`), `job`, `does`, `guarantees`, `method`, `cost`, `family`, `provenance`, `spdx` |
| `crates[].depends` | Each published crate the crate depends on: `name`, `link` (its crates.io page at the manifest's version), `licence` (from the crate header's `Licences` list), `manifest`, `purpose` (the version requirement), `take` (`depend`) |
| `crates[].errors` | Each error enum: `name`, `module`, `file`, `doc`, `non_exhaustive`, `kinds` (`name`, `shape`, `doc`) |
| `sources` | One record per source and how it was taken, in chapter 29.20's `Take` terms: `port` (a `Source:` record), `port-method` (a `Consulted:` record: a source a module that implements a published method consulted) or `depend` (a published crate a kernel or facade crate's manifest names). Each has `name`, `licence`, `link` (without its fragment), `pinned`, `take`, `method_note` and `used_by`. ORACLE and STUDY records come from the oracle runner and design notes, not from headers, and join when those land |
| `gaps` | The known gaps below, each with its file, rule, exact problem and reason |
| `pending` | The pending folders below that the tree does not hold yet, each with its reason |
| `format`, `grammar`, `generated_by` | The map's format (1; it changes only when a field's meaning changes or a field goes), this page, and the command that writes it |

The map is deterministic: crates in name order, modules in path order, object keys sorted, list items in header order, paths relative to the repository root with `/`, `\n` line ends, no timestamps. A field a header lacks is `null` or empty, and the check fails. The check compares line ends as `\n`, because a Windows checkout may turn them into `\r\n`; every other byte must match.

Operations and geometry kinds, which chapter 26.3 also asks the map to list, join it when the pieces that define them land (the operation contract in `geom-op`, the kinds in `geom-curve` and `geom-surface`).

## Rules

`cargo run -p geom-test --bin map -- check` prints each problem as `<file>: <rule>: <what to write>` and exits 1 if there is any; `write` prints the same, writes the map, and exits 1 too while a header is incomplete; a usage or file-system error exits 2.

| Rule | What fails it | Fixture in `tests/map_seeded.rs` |
| --- | --- | --- |
| `Manifest` | The crate folder has no `Cargo.toml`, its package name is not the folder's, or a `[lib]` or `[[bin]]` sets `path` | `manifest` |
| `Root` | The crate has neither `src/lib.rs` nor `src/main.rs` | `root` |
| `Unreadable` | A source file is not UTF-8, is a symbolic link, lies more than 8 folders below `src/`, or cannot be read as Rust tokens (an unterminated comment or literal); a folder under `src/` or a crate folder that is a symbolic link or junction | `unreadable` |
| `Header` | The file does not open with a `//!` header | `header` |
| `Summary` | The first paragraph does not end in a full stop, states nothing, is a field, or is not in its crate kind's form | `summary` |
| `Job` | A crate header states no job | `job` |
| `Layer` | A crate header states no layer, one outside 1 to 13, or the wrong one for its kind (a number for a development crate, `Development` for any other, anything but 13 for the facade) | `layer` |
| `Invariants` | A crate header's `Invariants` paragraph is missing, does not open a paragraph, has no list, or has an item that states nothing | `invariants` |
| `Sections` | A crate header names no chapter from 1 to 32 | `sections` |
| `Guarantees` | A module header states no guarantees, states them twice, or has a guarantee that states nothing | `guarantees` |
| `Method` | A module header names no method, a method without a link, a link into the repository that does not exist, `none` without a reason, or `none` for a module that defines functions | `method` |
| `Cost` | A module header states no cost | `cost` |
| `Family` | A family that is not one of chapter 10.2's, or a crate in the families table above without its family | `family` |
| `Source` | A `Source:` record without `PORT from`, a name and licence, a licence on the allow-list, a pinned link, a copyright line or a `Changed for Spare Kit:` line; or one source ported under two licences | `source`, `a_port_needs_a_licence_on_the_allow_list` |
| `Consulted` | A `Consulted:` record without a name and a classified licence, with an unpinned link on a code-hosting site or no purpose; code never to be read; LGPL or GPL code without its method note, or a note that does not exist; or one source under two licences | `consulted` |
| `SPDX` | An unreadable `SPDX-License-Identifier:` line; a ported file whose line is missing, off the allow-list or does not carry the origin's licence; a file that ports nothing with a line other than `MPL-2.0`; or a crate with a ported file whose `license` field does not name it beside MPL-2.0 | `spdx` |
| `Depend` | A kernel or facade crate names a published crate whose licence its header's `Licences` list does not record, or records off the allow-list | `depend` |
| `Uncredited` | A link to outside code in the header or a doc comment that no `Source:` or `Consulted:` record credits, or a commit named by an abbreviated name | `uncredited` |
| `Error enum` | A public error enum declared inside a block, where the map cannot place it | `error_enum` |
| `Error doc` | An error enum or error kind without a doc comment | `error_doc` |
| `Stale map` | `MAP.json` is missing, or differs from the map the headers give | `stale` |
| `Stale gap` | A known gap that no longer occurs, or a pending folder the tree now holds | `gaps` (and `a_pending_folder_is_checked_once_it_exists` in `tests/map_files.rs`) |

## Known gaps and pending folders

A problem that cannot be fixed in the piece that finds it is listed in `geom_test::map::GAPS` with its file, its rule, its exact message and its reason, and the piece that closes it, the way the scaffold check lists its header gaps. A listed gap hides that one problem and no other (a different problem in the same file and rule is still reported), `MAP.json` shows it, and a gap that no longer occurs fails the check, so the list cannot outlive its reason. Today there are none. The two first listed here are closed: geom-num's missing `Layer` line, when piece G09 added the line and deleted the scaffold check's matching record; and `geom-num`'s interval module crediting CGAL's `FPU.h` (LGPL) without a method note, when piece G16 created `projects/engines/geometry/methods/` and wrote the note, `methods/cgal-nextafter-widening.md`, which the record now names.

A folder the headers link into that this tree does not hold yet is listed in `geom_test::map::PENDING` with the piece that brings it. While the folder is absent, links into it are not checked for existence; once it exists, every link into it must resolve and the entry fails the check until it is deleted. Today there are none: the two first listed, `projects/engines/geometry/architecture/` and `projects/kit/notes/G01-evidence/` (the kernel architecture's chapters and G01's evidence, written on piece G01's branch), arrived when G01 merged, and every link into them is now checked.

## Sources

What this grammar and its checks were built from:

| Source | Licence | What was taken |
| --- | --- | --- |
| Kernel architecture chapters 10.2 (the catalogue families), 22 (error kinds), 26.3 (the map), 27 (crate and module headers, layers, the crate table), 29.2 (the licence classes and the allow-list) and 29.20 (`Source:` and `Consulted:` lines, the `SourceRecord`, rules 1, 3, 4 and 6) | Ours | The fields, their meaning, the record forms and the licence classes |
| G09's crate header reader, `projects/engines/crates/engine-geometry/tests/scaffold/header.rs` | Ours | Where the four crate fields are found: labels, paragraphs, sentences, the first numbered `Layer` sentence, the `Invariants` line that opens a paragraph, chapter numbers |
| The headers of `geom-num` (G02) and of `geom-test`, `geom-fuzz` and `geom-gen` (G11) | Ours | The module grammar: the labels already in use (`Guarantees`, `What every function here guarantees`, `Methods followed`, `What it does`, `Cost`) |
| CommonMark 0.31.2, appendix A, <https://spec.commonmark.org/0.31.2/#appendix-a-parsing-strategy> | CC BY-SA 4.0 (the text; nothing copied) | Block structure first, inline text after |
| The Rust Reference on comments, tokens, enumerations, implementations, functions and attributes, <https://doc.rust-lang.org/reference/> | MIT OR Apache-2.0 | The lexical and item grammar the scanners follow |
| `rustc_lexer`, <https://github.com/rust-lang/rust/blob/1731faea0da5b4ebf881d8e53f98066c17a74711/compiler/rustc_lexer/src/lib.rs> | MIT OR Apache-2.0 | The rule telling a character literal from a lifetime; which block comments are doc comments. Method only |
| Clippy's `clippy_dev` (`update_lints` check mode, its token cursor), <https://github.com/rust-lang/rust-clippy/blob/3583eb8a9ae1161bd7d80907acfa09b3612a93f9/clippy_dev/src/parse/cursor.rs> | MIT OR Apache-2.0 | Finding declarations with a token cursor rather than a full parser; regenerate and check in CI. Method only |
| rust-analyzer's tidy test, <https://github.com/rust-lang/rust-analyzer/blob/ea09c7b724320fdecea41830c77c6a32c31102a5/xtask/src/tidy.rs> | MIT OR Apache-2.0 | Every module file must open with a `//!` comment, tests excluded |
| rustc's tidy error-code check, <https://github.com/rust-lang/rust/blob/1731faea0da5b4ebf881d8e53f98066c17a74711/src/tools/tidy/src/error_codes.rs> | MIT OR Apache-2.0 | A catalogue read from one place and checked in CI, with named exemption lists |
| rustdoc's JSON output and `rustdoc-types`, <https://github.com/rust-lang/rustdoc-types/blob/2e63bdc44b94e86fe12d6e0034d1ed9c2ac6e9dc/src/lib.rs> | MIT OR Apache-2.0 | Read to decide not to use it (above) |
| Cargo's target auto-discovery, <https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery>; `cargo metadata`'s format, <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html> | MIT OR Apache-2.0 | Where crate roots, binaries and their modules are. `cargo metadata` would name the roots too, but needs cargo at run time; the folder rule of chapter 27 and the manifest check make it unnecessary |
| SPDX specification 2.3, annexes D and E, <https://spdx.github.io/spdx-spec/v2.3/> | CC BY 3.0 | Licence expressions and the `SPDX-License-Identifier:` line |
| SPDX License List 3.29.0, <https://github.com/spdx/license-list-data/blob/31ba1a50e5397e00a304dbadc76531740e89ee48/json/licenses.json> | CC BY 3.0 | Every identifier in the licence classes checked against it, and the deprecated GPL, LGPL and AGPL forms and the non-commercial licences listed from it |
| REUSE Specification 3.3, "Copyright notice", <https://reuse.software/spec-3.3/> | CC BY-SA 4.0 (the text; nothing copied) | The words a copyright line opens with (`SPDX-FileCopyrightText:`, `Copyright`, `©`), with `(c)` added because an origin's lines are copied as they are |
| The DOI Foundation on DOI syntax (ISO 26324), <https://www.doi.org/the-identifier/what-is-a-doi/> | Their page; nothing copied | A DOI is a prefix `10.` and a registrant code, a `/`, and a suffix |
| npm `hosted-git-info` 10.1.1, <https://github.com/npm/hosted-git-info/blob/8c3bebb5e42142b7bce06b085b4c1bd1adba7371/lib/hosts.js> | ISC | The address layouts of GitHub, Bitbucket, GitLab, gists and sourcehut, and where each puts its revision. Method only |
| Gitiles' view filter, <https://github.com/google/gitiles/blob/e54582fdd2a8783a6078f3a25eb08bb622f2e497/java/com/google/gitiles/ViewFilter.java> | Apache-2.0 | Its `+` view commands and the revision after them. Method only |
| gitweb.conf(5), <https://git-scm.com/docs/gitweb.conf> | Git's documentation | gitweb's `p`, `h` and `hb` parameters: the repository, the object and the commit ("hash base") |
| RFC 8259 (JSON) and RFC 3986 (URIs) | IETF Trust | The map's text and link syntax |
