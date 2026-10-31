# SKG: the Spare Kit Geometry native format, version 1 (draft)

*The specification of the kernel's native format (architecture chapter 24; piece G93). Format 1 is a draft: it gains records as the pieces that store each family land (body records, names and history, recipes, attributes), and piece G482 freezes it. The registry of record types is [REGISTRY.md](REGISTRY.md); the schemas are in [cddl/](cddl/). Tables between `generated` markers are written from `geom-format`'s `registry` module and checked against it by `crates/geom-format/tests/registry.rs`.*

This document is the source of truth: a reader written from it alone, in any language, must read every file the kernel writes (24.6).

## 1. Conventions

- **CBOR** is RFC 8949 ([rfc8949](https://www.rfc-editor.org/rfc/rfc8949)); **CDDL** is RFC 8610 ([rfc8610](https://www.rfc-editor.org/rfc/rfc8610)). "Item" is one CBOR data item.
- Every item is in SKG's **canonical form** (section 3); a reader refuses any other encoding.
- Byte offsets count from the first byte of the file, or of the record where a section says so.

## 2. A standalone file

A standalone file (extension `.skgf`, 32.18) is a CBOR Sequence (RFC 8742, [rfc8742](https://www.rfc-editor.org/rfc/rfc8742)) labelled as RFC 9277 describes ([rfc9277](https://www.rfc-editor.org/rfc/rfc9277)):

1. the 12-byte label `d9 d9 f8 da 53 4b 47 46 43 42 4f 52`: tag 55800, the protocol tag 0x534B4746 ("SKGF", requested from IANA, 32.18), and the byte string `h'424f52'`;
2. the header record (section 6), always first;
3. the records, in any order the writer chose, each one item;
4. the END record (section 7), always last; nothing follows it.

Inside a container (24.5) the label and END record are left out: each record's check and content ID stand for them.

## 3. Canonical form

RFC 8949's core deterministic encoding (section 4.2.1), within this subset:

| Item | Allowed | Refused |
| --- | --- | --- |
| Integers | −2^64 to 2^64 − 1, in the shortest head | longer heads |
| Bignums (tags 2, 3) | values beyond 64 bits, with no leading zero byte | values that fit 64 bits, leading zeros |
| Byte and text strings | definite length; text in UTF-8 | indefinite lengths, invalid UTF-8 |
| Arrays and maps | definite length; map keys sorted by the bytes of their encodings, no repeats | indefinite lengths, unsorted or repeated keys |
| Floats | finite, in the shortest of binary16, binary32 and binary64 that holds the value | NaN, infinities, wider forms |
| Columns (tags 79, 86) | a byte string of little-endian `i64` or finite `f64` (RFC 8746) | other lengths, NaN, infinities |
| Simple values | `false`, `true`, `null` | every other |
| Other tags | none | all |

A set is an array sorted by the bytes of each element's encoding, with no repeats; a sorted list is the same with repeats kept (32.19). Text hashed into a name is in Unicode Normalization Form C under Unicode 18.0.0 (32.19).

## 4. Records

Every record is one array (CDDL `record` in [cddl/record.cddl](cddl/record.cddl)):

<!-- generated: envelope -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `tag` | `uint` | the record type, from the registry; never reused |
| `flags` | `uint` | 1: required; 2: copy-always; every other bit refused |
| `schema` | `uint` | the layout version of this record type |
| `depends` | `[* content-id]` | the records this one was derived from, a set sorted by the bytes of each encoding |
| `payload` | `bstr` | one item of SKG's canonical CBOR, at most 64 MiB, nested at most 64 deep |
| `check` | `uint` | the CRC-32C of the encoding of the five fields above |
<!-- end generated -->

- **Flags.** Bit 1 (value 1) marks a *required* record: a reader that does not know its tag and schema stops, naming the tag, the schema and the record's offset. Without it the record is *optional*: an unknown one is skipped and listed. Bit 2 (value 2) marks *copy-always*: a program rewriting a file it only partly understands keeps the record byte for byte. Without it the record is *copy-if-unchanged*: kept only while every record in `depends` is present with the same content ID. Any other bit is refused.
- **Check.** CRC-32C (Castagnoli, polynomial 0x1EDC6F41, reflected, initial value and final XOR 0xFFFFFFFF; RFC 7143) of the bytes of the five fields before it, as stored: from the byte after the record's array head to the byte before the check.
- **Caps.** A payload is at most 64 MiB (67,108,864 bytes) and nests at most 64 deep; a record is at most 64 MiB plus 1 MiB. A reader checks each length before reading the bytes it announces.

## 5. Content IDs

`content-id = [alg: uint, digest: bstr]`. Algorithm 1 is the SHA-256 (FIPS 180-4) of a record's payload bytes, a 32-byte digest; an algorithm-1 ID of any other length is refused. An ID of another algorithm is carried as read, so a stronger hash can join as algorithm 2.

## 6. The header record

Tag 1, schema 1, required. Its payload (CDDL `header` in [cddl/header.cddl](cddl/header.cddl)):

<!-- generated: header -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `format` | `[major: uint, minor: uint]` | the format version; 1.0 while format 1 is a draft |
| `edition` | `uint` | the kernel edition that wrote the file; 0 before the first release |
| `name-scheme` | `uint` | the NameScheme of every NameId in the file; 1 (32.19) |
| `ladder` | `{ * uint => int }` | the tolerance ladder: rung code to value (6.5); see the rung table |
| `unicode` | `[uint, uint, uint]` | the Unicode version of hashed text; 18.0.0 (32.19) |
| `program` | `tstr` | the program that wrote the file |
| `extensions` | `{ * uint => any }` | an odd key must be understood; an even key may be skipped; none in 1.0 |
<!-- end generated -->

The tolerance ladder (architecture 6.5), by rung code, as the kernel writes it:

<!-- generated: rungs -->
| Code | Rung | Unit | Value |
| --- | --- | --- | --- |
| 1 | fit bound, ε_fit | grid steps | 64 |
| 2 | per-operation bound, ε_step | grid steps | 16 |
| 3 | this document's import healing, ε_heal | grid steps | 640 |
| 4 | the cap on import healing | grid steps | 6400 |
| 5 | short-edge warning | grid steps | 3200 |
| 6 | commit rounding per axis | half grid steps | 1 |
| 7 | angular bound, ε_ang | log2 of radians | -34 |
<!-- end generated -->

A reader refuses a major format version other than 1, and an extension key with its must-understand bit (bit 0, an odd key) that it does not know; an even key it does not know is skipped and listed. A newer minor version is read.

## 7. The END record

Tag 2, schema 1, required. Its payload (CDDL `end` in [cddl/end.cddl](cddl/end.cddl)):

<!-- generated: end -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `count` | `uint` | the records before this one, the header included |
| `digest` | `bstr .size 32` | the SHA-256 of every byte before this record, the label included |
<!-- end generated -->

A reader checks both and refuses a file whose count or digest differs, a file without an END record, and bytes after it.

## 8. Reading untrusted bytes

- A reader's result does not depend on how the bytes arrive: read in pieces of any size, a file gives the same records, and the same error at the same offset.
- Damage is found by the check; a **salvage read** keeps every record whose bytes decode and whose check matches, with its offset, and names each damaged span between them.
- Before any body is built, numbers are range-checked (24.7): grid coordinates within ±(2^40 − 1) steps; doubles finite and within the same range; planes with normal components of at most 83 bits, offsets of at most 125 bits and no common divisor of the four. A failure is `OutOfRange { record, offset, field, bits }` or `Malformed`.
- Tables in body records are cut into chunks of 4,096 rows (section 4's caps apply to each chunk record).

## 9. Versions

The header holds the format version. A minor version only adds what an older reader can skip or fall back from: new optional records, new even extension keys, new schema versions read beside the old. A major version runs beside the old one. Record types are versioned on their own by `schema`; a reader keeps a decoder for every schema ever published (24.6).

## 10. Body records

A body is stored as one **index** record (tag 3) and a **chunk** record (tag 4) for each run of 4,096 rows of each of its tables, all schema 1 and required. The tables are the body's arenas and geometry pools (chapter 8.2), each in arena order: a chunk holds rows 4,096·k to 4,096·k + 4,095 of its table (fewer in a table's last chunk), and an empty table has none. A chunk's bytes are a function of its rows alone, so a chunk whose rows did not change keeps its bytes and its content ID: an edit rewrites only the chunks its rows are in, and the index.

The index lists every table, in code order, with its row count and its chunks' content IDs in order; its `depends` is the set of those IDs. So the index's content ID, the body's, is the root of a hash tree over its chunks (Merkle). Its payload (CDDL `body` in [cddl/body.cddl](cddl/body.cddl)):

<!-- generated: body -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `kind` | `0..4` | solid, sheet, wire, acorn or general (8.1) |
| `outside` | `uint` | the infinite void region, as a reference: its row plus one |
| `tables` | `[* [table: uint, rows: uint, chunks: [* content-id]]]` | each table by code, in order, with its rows and its chunks' content IDs in order |
| `named` | `0 / 1` | 1 if the body was published by name, when tables 12 to 15 hold its names |
| `extensions` | `{}` | none in schema 1 |
<!-- end generated -->

A chunk's payload (CDDL `chunk`):

<!-- generated: chunk -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `table` | `uint` | the table's code |
| `position` | `uint` | the chunk's place in its table, from 0 |
| `rows` | `1..4096` | its rows: 4,096 in every chunk but a table's last |
| `columns` | `[+ refs / i64-column / bstr / [+ surface] / [+ curve]]` | the table's columns over those rows, in the table's order |
<!-- end generated -->

The tables, by code, and their columns. A reference to a row is stored as the row plus one, and 0 for none; a sense as 0 (forward) or 1 (reversed). Each column but the faces' bounds, the points and the names is an array of one unsigned integer per row; the surface, curve and pcurve columns are arrays of one kind (section 11) per row.

<!-- generated: tables -->
| Code | Table | Columns |
| --- | --- | --- |
| 1 | regions | kind (0 solid, 1 void), first shell |
| 2 | shells | region, next, first face |
| 3 | faces | shell, next, surface, sense, first loop, bound (an `i64` column of six per face) |
| 4 | loops | face, next, first fin |
| 5 | fins | loop, next, previous, edge, sense, radial, pcurve |
| 6 | edges | first fin, start, end, curve |
| 7 | vertices | point |
| 8 | points | x, y, z in grid steps (an `i64` column of three per point) |
| 9 | surfaces | one surface kind per row |
| 10 | curves | one 3D curve kind per row |
| 11 | pcurves | one 2D curve kind per row |
| 12 | region names | 16 bytes per region, one byte string |
| 13 | face names | 16 bytes per face, one byte string |
| 14 | edge names | 16 bytes per edge, one byte string |
| 15 | vertex names | 16 bytes per vertex, one byte string |
<!-- end generated -->

A reader checks the index against its chunks before anything is built: each listed chunk is present, has the content ID listed, and names its own table and place; its row count is 4,096 unless it is its table's last; the tables are exactly 1 to 11, and 12 to 15 when `named` is 1; no chunk is given that the index does not list; `depends` is the listed IDs. It then rebuilds the arenas and publishes them through the kernel's builder, which checks the topology again (by name when `named` is 1), so a stored body is trusted no further than a built one. A body read back equals the one written, row for row, and the next operation on it gives the same bits.

## 11. Geometry kinds

Each surface, curve and pcurve is an array `[code, fields…]`, its fields the values the kind's constructor takes; reading rebuilds it through that constructor. Doubles are written in their shortest exact width (section 3), lists of doubles as `f64` columns (RFC 8746, tag 86), an absent value as `[]` and a present one as `[value]`, a flag as 0 or 1, and an axial sign or σ as the integer 1 or −1. An interval is `[start, end]`, an end that is infinite on its own side (a ray's or a line's) written `[]`. A frame is `[x, normal]`; axes are `[0, frame, a, b]` or `[1, u, v]`; helix axes `[0, frame, radius, pitch]` or `[1, u, v, w]`. A swept surface's placement is `[c, L]`, L's nine entries by rows. An offset's fitted NURBS cache is not stored: the `cached` flag says whether it was there, and reading fits it again, with the same bits (23.1). A curve on a surface keeps its 3D curve, which defines it.

The curve kinds:

<!-- generated: curves -->
| Code | Kind | Fields |
| --- | --- | --- |
| 1 | NURBS | degree, knots, homogeneous points, weights |
| 2 | line, ray or segment | origin, direction, domain |
| 3 | circle | centre, radius, frame, domain |
| 4 | ellipse | centre, axes, domain |
| 5 | hyperbola | centre, axes, domain |
| 6 | parabola | vertex, axes, domain |
| 7 | helix | centre, helix axes, domain |
| 8 | polynomial | coefficients, domain |
| 9 | involute | centre, axes, domain |
| 10 | bulge polyline | vertices, bulges, widths, closed, start |
| 11 | trimmed | a leaf kind (codes 1 to 10 and 15), interval |
| 12 | composite | trimmed segments, starts |
| 13 | offset | trimmed basis, offset law, cached |
| 14 | on a surface | trimmed pcurve, surface, 3D curve (NURBS fields), error |
| 15 | transition | start, direction, length, curvature law, window, reversed |
<!-- end generated -->

The surface kinds:

<!-- generated: surfaces -->
| Code | Kind | Fields |
| --- | --- | --- |
| 1 | NURBS | u degree, u knots, v degree, v knots, homogeneous points, weights |
| 2 | plane | origin, a, b, domain |
| 3 | cylinder | centre, radius, frame, domain, axial sign |
| 4 | cone | centre, radius, slope, frame, domain, axial sign |
| 5 | sphere | centre, radius, frame, domain, axial sign |
| 6 | torus | centre, major radius, minor radius, frame, domain, axial sign |
| 7 | affine image | centre, terms, domain, angular u, angular v |
| 8 | revolution | profile, axis, frame, angles, placement |
| 9 | extrusion | profile, direction, extent, placement |
| 10 | ruled | first profile, second profile, extent, sigma, placement |
| 11 | offset | base, distance, domain, cached |
<!-- end generated -->

## 12. The names record

Tag 16, schema 1, required. Its payload (CDDL `names` in [cddl/names.cddl](cddl/names.cddl)):

<!-- generated: names -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `nodes` | `[* [id: name-id, name: topo-name]]` | every name node the file keeps, in NameId order; each id is the first 16 bytes                       of the SHA-256 of the scheme's prefix and the name's canonical bytes (32.19) |
<!-- end generated -->

Every face, edge and vertex `NameId` a file holds, and the name nodes reachable from live entities and stored references (architecture 24.4), as one array of `[id, name]` pairs in strictly increasing `NameId` order. Each name is a `TopoName` in its canonical encoding (32.19): a variant number (`Created` 0, `Split` 1, `Merged` 2, `Bounded` 3, `Weak` 4) and its fields, sets and sorted lists in byte order, role names in Normalization Form C. A reader:

- refuses the record when the header's `name-scheme` is one it does not know;
- checks each id: the first 16 bytes of the SHA-256 of the 17 ASCII bytes `SKG NameScheme v1`, a zero byte, and the name's bytes as stored; a node whose id differs is refused, never trusted;
- decodes each name and encodes it again; bytes that differ are a second spelling and are refused;
- refuses a node out of order, and a second node under one id.

## 13. The history record

Tag 17, schema 1, required. Its payload (CDDL `history` in [cddl/history.cddl](cddl/history.cddl), with names.cddl's rules):

<!-- generated: history -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `op` | `op-path` | the operation's path: its OpId and its labels |
| `records` | `[* [output: name-id, origin]]` | one per output, in output-name order: unchanged, modified, generated, split                           or merged (20.3) |
| `deleted` | `[* name-id]` | the inputs the operation deleted, a set |
| `changes` | `[* topology-change]` | the topology changes it made (13.14), in the history's order |
<!-- end generated -->

One operation's history (architecture 20.3): every output's origin and the inputs it deleted, with the topology changes of 13.14. An origin is `[0]` unchanged, `[1, from]` modified, `[2, from, role]` generated (from an empty set when made from nothing), `[3, of, piece]` split or `[4, of]` merged from at least two. A change is its class's number in 13.14's table order (vertex split 0, face vanished 1, self-overlap cut 2, clash trimmed 3, overshoot removed 4, faces merged 5, face added 6, loop changed 7, body split 8) with its fields; why a face vanished is collapsed 0 or inside out 1, and a loop change is hole closed 0 or face split 1. A reader refuses two records of one output, a modification from nothing, a merge of one, and any payload that is not the history's canonical encoding.

## 14. The recipe record

Tag 18, schema 1, required. Its payload (CDDL `recipe` in [cddl/recipe.cddl](cddl/recipe.cddl), with names.cddl's and record.cddl's rules):

<!-- generated: recipe -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `owner` | `tstr` | the owner's namespace, printable ASCII: `kit.geom` for kernel operations, `kit.geom.ifc`, a plugin's ID |
| `owner-schema` | `uint` | the owner's schema version for the payload |
| `op` | `op-path` | the operation's path |
| `version` | `uint` | the operation's version (26.2) |
| `inputs` | `[* ([0, content-id] / [1, name-id])]` | the inputs in the owner's order: a body by content ID, an entity by name |
| `payload` | `bstr` | the owner's own encoding of the recipe, never decoded by the format |
| `outputs` | `[* content-id]` | the content IDs of the bodies the recipe produced |
<!-- end generated -->

How a shape was made, in one envelope every reader knows (architecture 24.4), whoever owns it. The format never decodes the payload; its owner does (the facade from operation schemas, `geom-ifc`, a plugin). A reader without the owner still opens the bodies the recipe produced, by their content IDs, keeps the recipe byte for byte, and refuses only what would re-run it, with `RecipeOwnerMissing`, naming the owner and its schema. A reader refuses an owner that is empty or not printable ASCII, and any payload that is not the recipe's canonical encoding.

## 15. The attributes record

Tag 19, schema 1, required. Its payload (CDDL `attributes` in [cddl/attributes.cddl](cddl/attributes.cddl), with names.cddl's rules):

<!-- generated: attributes -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `definitions` | `[* definition]` | every attribute definition in attribute order: owner, name and version, the kinds that may carry it, its value schema and its behaviour at 8.4's events |
| `values` | `[* [attr-id, [* [name-id, carried]]]]` | each attribute's values by name: the owner's opaque bytes, or the values a merge combined |
<!-- end generated -->

Every attribute definition a model holds, and every value keyed by the persistent name of the entity that carries it (architecture 24.4), so one program carries another's values through its own edits without understanding them. A definition gives the kinds of entity that may carry it (body 0, region 1, face 2, edge 3, vertex 4), its value schema (opaque 0 with the owner's schema ID, point 1, direction 2, frame 3, length 4, field 5) and its behaviour at section 8.4's events: changed (keep 0, delete 1); moved, rotated, mirrored and scaled (keep 0, transform 1, delete 2); split (propagate 0, delete 1); merged (keep if equal 0, combine 1, delete 2); copied (copy 0, delete 1). A value is the owner's bytes, `[0, value]`, or the values a merge combined, `[1, [name, value] ...]` in name order. A reader refuses a definition that may transform an opaque value or that no kind may carry, an attribute defined twice otherwise, a value whose attribute is not defined, and any payload that is not the canonical encoding.

## 16. The operation record

Tag 20, schema 1, required. Its payload (CDDL `operation` in [cddl/operation.cddl](cddl/operation.cddl), with names.cddl's and record.cddl's rules):

<!-- generated: operation -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `op` | `op-path` | the operation's path: its permanent OpId from the log, and labels |
| `name` | `tstr` | its name in the operation schemas, printable ASCII |
| `version` | `uint` | its version, which fixes its results, charges and details (26.2) |
| `inputs` | `[* ([0, content-id] / [1, name-id])]` | its inputs in its schema's order: a body by content ID, an entity by name |
| `params` | `bstr` | its parameters: one item of SKG's canonical CBOR, as its schema writes them |
| `budget` | `[work: uint, logical-bytes: uint]` | the budget it runs under (23.10) |
<!-- end generated -->

One entry of an operation log (architecture 21 and 28.13): everything a replay needs to run the operation again and get the same bits. The record type has its schema, and each record names the operation's own version, which fixes its results, its charge schedule and its error details (26.2). The parameters are one item of SKG's canonical CBOR, written by the operation's schema. The budget is recorded because a budget that can fail an operation in a shared model is fixed by its version or recorded in the log (23.10). Nothing timed and no precision is recorded: only final operations are logged, and a cancelled run writes nothing (10.1). A reader refuses a name that is empty or not printable ASCII, parameters that are not canonical CBOR, and any payload that is not the record's canonical encoding.


## 17. Validation stamps

A **stamp** (tag 5, schema 1) says which checker passed a stored body, which of the checker's groups it passed (architecture chapter 8.3: 1 combinatorial, 2 geometric consistency, 3 self-intersection, 4 the exactness record) and under which kernel edition (24.7). It is a record of its own, never a field of the body index, so checking a body again adds a stamp and leaves the body's content ID unchanged. It is optional, so a reader that does not know it skips it and checks the body in full, and it is not copy-always, so a program rewriting a file keeps it only while its body is there with the same content ID. Its `depends` is exactly the stamped body's index, and its payload names that index again, so stamps about two bodies never share a content ID. Its payload (CDDL `stamp` in [cddl/stamp.cddl](cddl/stamp.cddl)):

<!-- generated: stamp -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `body` | `content-id` | the body index the stamp is about, also its one dependency |
| `checker` | `uint` | the version of the checker that passed the body |
| `groups` | `[* uint]` | the groups of chapter 8.3 it passed, by number, strictly ascending |
| `edition` | `uint` | the kernel edition the check ran under |
<!-- end generated -->

A reader refuses a stamp whose groups are not strictly ascending, whose `depends` is not the body its payload names, or that has bytes after it. A stamp is read only when the program says the document comes from a known source (section 14); from any other source it is skipped, and the body is checked in full.

## 18. Containers

A container stores a document's records as **blobs**, each the bytes of one whole record (flags and check included) under the content ID of its payload, so every lookup verifies itself: a reader refuses a blob that is not one intact record, or whose payload's content ID is not its key. A document is its **document blob**, its header record (with its frames once frame records exist), and its **roots**, the records nothing else in it depends on, in order.

A standalone file made from a container holds the header, then each root after the records it depends on, depth first and each record once: a body's chunks in the order its index lists them, then the index; any other record's dependencies in its `depends` order; then the END record. A file written in that order, as the kernel's writers write them, goes to a container and back byte for byte; any file goes and comes back as the same records, each byte for byte. Salvaging a damaged container writes every intact record out as a standalone file, in the same order, and lists the missing and damaged blobs.

## 19. Trust on open

The program states the trust of what it opens; a reader never infers it from the file (24.7).

- **Known** (the program's own store, or a source whose integrity it checks): a body with a stamp from a checker at least as new as the reader's, listing every group the reader's full check runs, needs only the cheap groups (1 and 4). Any other body needs the full check.
- **Unknown** (everything else): stamps are not read, and every body needs the full check.

Every body is range-checked and rebuilt through the topology builder, and the cheap groups run on it, when it is opened. The full check runs later, body by body, within the caller's budget. A body whose records are missing or damaged opens with no geometry; one that fails a check opens read-only, with the checker's report; neither is ever an operation input, and the rest of the document opens normally. A body that passes its full check earns a stamp for the program to store.
