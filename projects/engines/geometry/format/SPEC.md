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

## 10. The names record

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

## 11. The history record

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

## 12. The recipe record

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

## 13. The attributes record

Tag 19, schema 1, required. Its payload (CDDL `attributes` in [cddl/attributes.cddl](cddl/attributes.cddl), with names.cddl's rules):

<!-- generated: attributes -->
| Field | CDDL | Holds |
| --- | --- | --- |
| `definitions` | `[* definition]` | every attribute definition in attribute order: owner, name and version, the kinds that may carry it, its value schema and its behaviour at 8.4's events |
| `values` | `[* [attr-id, [* [name-id, carried]]]]` | each attribute's values by name: the owner's opaque bytes, or the values a merge combined |
<!-- end generated -->

Every attribute definition a model holds, and every value keyed by the persistent name of the entity that carries it (architecture 24.4), so one program carries another's values through its own edits without understanding them. A definition gives the kinds of entity that may carry it (body 0, region 1, face 2, edge 3, vertex 4), its value schema (opaque 0 with the owner's schema ID, point 1, direction 2, frame 3, length 4, field 5) and its behaviour at section 8.4's events: changed (keep 0, delete 1); moved, rotated, mirrored and scaled (keep 0, transform 1, delete 2); split (propagate 0, delete 1); merged (keep if equal 0, combine 1, delete 2); copied (copy 0, delete 1). A value is the owner's bytes, `[0, value]`, or the values a merge combined, `[1, [name, value] ...]` in name order. A reader refuses a definition that may transform an opaque value or that no kind may carry, an attribute defined twice otherwise, a value whose attribute is not defined, and any payload that is not the canonical encoding.

## 14. The operation record

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

