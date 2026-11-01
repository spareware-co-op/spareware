# 24. File format, versions and exchange

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) sections A1 to A3 and D; [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) sections 1 to 3; [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) section 1; [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) sections 1 and 6; [07](../../../kit/notes/G01-evidence/07-persistent-naming.md) sections 2, 5 and 8; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) sections 2, 5 and 8; [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) sections 4 and 6; [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) sections 1 and 2; [10](../../../kit/notes/G01-evidence/10-github-sweep.md); [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) (cadcore); [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) (DWG); [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) (USD, OpenCascade's format); [EXTENDING.md](../../../EXTENDING.md) section 3; Spare Construct's [Storage and scale](../../spare-construct/ARCHITECTURE.md#storage-and-scale), [Point clouds](../../spare-construct/ARCHITECTURE.md#point-clouds) and [Interop](../../spare-construct/ARCHITECTURE.md#interop). Specifications read for this chapter are linked where used, pinned to a commit where they live in a repository.

The kernel keeps geometry in two ways. Its own format, SKG, holds a model exactly as the kernel made it, for as long as anyone needs it. Exchange formats trade geometry with other programs. One rule covers both: nothing exact is made less exact without saying so, and nothing is left out without a typed reason in the report.

Reading and writing are operations under the contract of section 10.1: transactional, budgeted, cancellable and batch-first, with typed errors and remedies. Each has a schema, so its AI tool is generated (section 22). Programs reach them through their own adapters (EXTENDING.md, section 2.7).

## 24.1 The native format, and why this encoding

The native format must be readable forever without any Spareware program, give the same bytes for the same model on every machine, lose nothing, let an old reader open a newer file or refuse it clearly, find damage, open without replaying anything (section 26.2), and keep names and history (section 20). EXTENDING.md section 3 asks for an existing standard before a new format, and a note saying why when none will do. These were weighed:

| Option | Canonical bytes | Readable without our code | Verdict |
| --- | --- | --- | --- |
| Our own binary framing | Ours to define | Only through our documents | No |
| Deterministic CBOR ([RFC 8949](https://www.rfc-editor.org/rfc/rfc8949), section 4.2) as a CBOR Sequence ([RFC 8742](https://www.rfc-editor.org/rfc/rfc8742)), schemas in CDDL ([RFC 8610](https://www.rfc-editor.org/rfc/rfc8610)) | Yes, by the standard's own rules | Decoders in every major language; a standard text notation; machine-checkable schemas | **Yes** |
| STEP Part 21 text | No; decimals only | Many STEP tools | No: no chunks, no random access, slow |
| SQLite | No: page layout depends on history | Any SQLite tool | A container, not an encoding (24.5) |
| Protocol Buffers | No: "Deterministic serialization is not canonical" ([protobuf.dev](https://protobuf.dev/programming-guides/serialization-not-canonical/)) | Many | No |

So SKG (Spare Kit Geometry; standalone files end `.skgf`, owner action G77, 32.18) is deterministic CBOR, an IETF Internet Standard (STD 94), with a record layer on top. CBOR brings what a format of our own would not: anyone can decode its structure with an off-the-shelf tool; every item is self-delimiting, so any unknown part can be skipped; typed arrays ([RFC 8746](https://www.rfc-editor.org/rfc/rfc8746)) hold coordinate columns as raw little-endian numbers; and the canonical rules are already written. The record layer adds a registry of record types, required and optional records, checksums, content IDs, chunks and fallbacks.

Two ideas come from Rhino's 3dm format, not its format: skippable typed chunks with a checksum, and geometry pools that topology refers to by index ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 1; [opennurbs_3dm.h L17-52](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/opennurbs_3dm.h#L17-L52)). 3dm's specification is effectively its C++ code, its type codes are McNeel's, and it has no exact ellipse, cone or clothoid.

`geom-format` reads and writes SKG. How a program packs records into its project file is the program's choice (24.5). Standalone `.skgf` files carry test corpora, bug reports, examples for AI agents, and geometry passed between Spareware programs.

## 24.2 Records

A standalone file is a labelled CBOR Sequence: the 12-byte label of [RFC 9277](https://www.rfc-editor.org/rfc/rfc9277) (`d9 d9 f8 da`, a four-byte protocol tag, `43 42 4f 52`; tag `0x534B4746`, 1397442374, "SKGF", requested from IANA's first-come-first-served range, 32.18), a header record, the records, and an END record.

```cddl
record = [
  tag:     uint,            ; record type, from the registry; never reused
  flags:   uint,            ; criticality and copy policy
  schema:  uint,            ; layout version of this record type
  depends: [* content-id],  ; records this one was derived from, sorted
  payload: bstr,            ; one deterministic CBOR item, described by the tag's CDDL
  check:   uint,            ; CRC-32C of the encoding of the five items above
]
content-id = [alg: uint, digest: bstr]   ; alg 1: SHA-256 of the payload bytes
```

- **The registry**, `geometry/format/REGISTRY.md`, gives every tag's name, schema versions, CDDL and the part of `SPEC.md` that defines it.
- **Criticality.** An unknown *required* record stops the reader with `UnsupportedRecord` (tag, schema, byte offset). An unknown *optional* record is skipped and listed in the read report.
- **Copy policy**, for a program rewriting a file it only partly understands. PNG copies an unknown safe-to-copy chunk whatever changed, and an unsafe one only if no critical chunk changed ([PNG, section 5.4](https://www.w3.org/TR/png-3/#5Chunk-naming-conventions)). SKG makes the test exact: a *copy-always* record is kept byte for byte; a *copy-if-unchanged* record is kept only while every record in its `depends` list is present with the same content ID, and is otherwise dropped with a diagnostic.
- **Checksums and content IDs.** CRC-32C (Castagnoli polynomial, [RFC 7143](https://www.rfc-editor.org/rfc/rfc7143)) finds damage cheaply. The content ID is the SHA-256 ([FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)) of the payload behind an algorithm code, so a stronger hash can join within section 2's fifty-year horizon. The owner chose SHA-256 and RustCrypto's `sha2` crate for it (owner action G79, 32.20). The END record holds the record count and the SHA-256 of everything before it. A salvage read keeps every intact record and names the damaged ones.
- **Chunks.** A payload is at most 64 MiB (proposed). A body is an index record plus chunk records: each table (points, planes, curves, faces and so on) is cut into fixed runs of 4,096 rows (proposed, fixed by the specification and versioned). Chunk boundaries are thus a pure function of the body, never of how memory was shared. The index lists its chunks' content IDs, so a body's content ID is the root of a hash tree, and saving a new version writes only changed chunks. A row added in name order shifts the rows after it, so those chunks count as changed: 32.45 measures this and keeps the fixed runs.
- **Compression sits outside the canonical form.** A container may compress records (for example zstd, [RFC 8878](https://www.rfc-editor.org/rfc/rfc8878)); content IDs and section 21's golden hashes are always over uncompressed bytes. Standalone files are uncompressed.

## 24.3 Canonical bytes

The bytes are a pure function of the stored value, on every machine and for any thread count (section 21). CBOR's core deterministic rules apply (RFC 8949, section 4.2.1), plus these:

| Value | Encoded as |
| --- | --- |
| Coordinate or length | Whole grid steps of its frame; columns as little-endian `i64` typed arrays (RFC 8746 tag 79) |
| Plane of a plane-based solid | Four integers divided by their greatest common divisor, never scaled by a negative number, so orientation is kept. Normals need at most 83 bits, offsets 125 ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4); above 64 bits they are CBOR bignums without leading zero bytes |
| Vertex of a plane-based solid | Positions of the three planes the body already holds for it, chosen when it was built (section 11.3's rule for a vertex on more than three planes). The writer never chooses again, so a reload matches index for index. Never rounded, never stored as coordinates |
| Any other real | The exact `f64` value in CBOR's preferred form, the shortest float that keeps the value (RFC 8949, section 4.1), so it reads back bit for bit; columns as `f64` typed arrays (tag 86). NaN and infinities are refused both ways; −0.0 is kept as itself |
| Counts, positions, kinds | Unsigned integers in shortest form; kinds from the registry |
| Persistent names | The 128-bit `NameId` as 16 bytes, and the canonical encoding of its `TopoName` (section 20) |
| Text | UTF-8. Text hashed into a name must be in Unicode Normalization Form C under the scheme's Unicode version, 18.0.0 for `NameScheme v1` (32.19); the writer refuses other text |
| Sets and maps | Arrays sorted by key; never hash-map order |

**Positions inside a body record.** A body's tables are written in its arena order (section 8.2), which is persistent-name order (owner action G76, 32.17), and refer to each other by position. These record positions are defined by the specification, valid only inside that body record, and never used as references: everything outside a body names its entities by `NameId`, as section 20.1 requires. Keeping arena order means a reloaded body equals the in-memory one index for index, so the next operation gives the same bits on a replica that built the body and on one that opened it.

**Growing inside a record**, so a new kind or field needs only a minor version:

- Every geometry pool entry starts with its kind. An entry of a kind added after format 1.0 carries a fallback beside it, which a reader that does not know the kind uses.
- Every required record type ends with an extension map of integer keys, each with a must-understand bit. An unknown optional key is ignored and kept when the record is copied unchanged; an unknown must-understand key makes the reader refuse the record.
- Fallbacks by family: a curve or surface kind falls back to its NURBS conversion with exactness flag and bound; a new field operation to the field's certified mesh; a new subdivision rule to its limit surface as NURBS, exact on regular regions and approximate elsewhere with a stored deviation (section 9); a new representation to a certified mesh. A new topology entity or body kind has no fallback: it is a major version.

## 24.4 What a file holds

| Record family | Holds | Flags |
| --- | --- | --- |
| Header | Format version, kernel edition, `NameScheme` version, the tolerance ladder in steps, Unicode version, writing program | Required |
| Frame | Section 17.1's frame record | Required |
| Body index and chunks | Geometry pools; topology tables referring to them; body kind; exactness class and bounds; Tolerant flag; kernel edition; validation stamp (24.7) | Required |
| Names, history | Every face, edge and vertex `NameId` and the name nodes reachable from live entities and stored references; each operation's origins and deletions (section 20.3) | Required |
| Recipe | The envelope below | Required |
| Procedural caches | Fitted curves and surfaces of intersection curves, blends and offsets, with certified errors; they feed decisions, and a later edition might fit differently | Required |
| Other representations | Exact meshes; subdivision cages with creases; fields as canonical bytecode ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 2) | Required |
| 2D drafting | Regions, hatches, dimensions as typed snap references, text anchors with extents, blocks and inserts (section 14) | Required |
| Drawings | Each sheet's drawing curves in section 18's canonical form, with sources, piece keys, classes and the edition that made them, so a sheet opens without recomputing (section 18.10) | Required |
| Sketches | Each stored sketch's plane, entities, constraints, root choices and values (what section 15.13's text form holds, encoded by 24.3's rules, not as text); its last committed geometry; and, where the regenerating layer records it, its `SketchUnsolved` state with that geometry marked stale (section 15.14) | Required |
| Point-cloud reference | A link to a scan, not its points (below) | Required |
| Attributes | Definitions with their class; values keyed by `NameId` (below) | Required envelope |
| Import provenance | Source format and schema, SHA-256 of the source, units, declared precision, ε_heal used, every rounding and healing action | Required |
| Display | Tessellations and levels of detail, each with deviation, class and the edition that made it (section 23) | Optional; copy-if-unchanged on its body |

So `geom-format` depends on each crate whose values these records hold: `geom-num`, `geom-math`, `geom-curve`, `geom-surface`, `geom-topo`, `geom-poly`, `geom-name`, `geom-mesh`, `geom-subd`, `geom-field`, `geom-2d`, `geom-cloud`, `geom-draw` and `geom-sketch`. It never depends on an exchange crate or on a recipe's owner (below).

**Exactness and the Tolerant state are separate fields.** *Tolerant* is the import state of section 6.5: some edges and vertices carry tolerances up to ε_heal. A Tolerant body's class is that of its non-tolerant entities, plus the Tolerant flag and per-entity tolerances of at most ε_heal. Each stored tolerance was measured with certified bounds and is kept per entity, apart from the Certified bound. It never counts toward the ε_fit ceiling or toward section 9's sum of bounds. Those apply only to entities that are not Tolerant and to everything an operation creates, which section 11.4 certifies to the ladder. So an operation works within a Tolerant body's stored tolerances, as section 6.5 promises, or refuses with `ToleranceExceeded`. Sections 6.5 and 9 do not yet say which class a Tolerant body carries; this is the rule proposed to them.

```rust
enum StoredVertex {
    Point { at: PointPos, tolerance: Option<Steps> }, // tolerance only when Tolerant, then at least
                                                      // that of every edge through it (Parasolid's rule)
    Meet([PlanePos; 3]),                              // where three planes meet: exact, never rounded
}
struct StoredEdge {
    curve: Option<CurvePos>,   // None only on a Tolerant edge, whose fins' pcurves are primary
    nominal: Option<CurvePos>, // a Tolerant edge's sender curve, kept as Parasolid may keep one
    ends: Ends,                // two vertices, or none for a closed edge
    tolerance: Option<Steps>,  // Some only on a Tolerant edge, never above ε_heal
}
```

**The recipe envelope.** Section 26.1 promises that a file opens and draws where the plugin that made a shape is absent. So every recipe, whoever owns it, sits in one envelope every reader knows: owner namespace (`kit.geom` for kernel operations, `kit.geom.ifc`, a plugin's ID), the owner's schema version, the `OpPath` and operation version, inputs by content ID and `NameId`, an opaque payload, and the content IDs of the evaluated bodies it produced. `geom-format` never decodes the payload; its owner does (the facade from operation schemas, `geom-ifc`, the plugin). A reader without the owner opens the evaluated bodies, keeps the payload byte for byte, and refuses only what would re-run the recipe, with `RecipeOwnerMissing`. The crate graph stays one-way: exchange crates depend on `geom-format`, never the reverse.

**Attributes** follow Parasolid's attribute classes: each definition says what its values do when an entity is split, merged, transformed, copied or deleted (propagate, keep if equal, combine, delete) ([07](../../../kit/notes/G01-evidence/07-persistent-naming.md), section 5; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 8). Definitions sit in a record every reader knows, so one program carries another's fire ratings through its own edits without understanding them.

**Point clouds stay in their own open format.** Spare Construct stores scans as LAZ 1.4 octrees in the [COPC](https://github.com/copcio/copcio.github.io/blob/a6336f7bbcf920da0be4984a0993d67d2f082656/README.md) layout (MIT), an existing standard that keeps every per-point attribute ([Point clouds](../../spare-construct/ARCHITECTURE.md#point-clouds)). The model holds a reference record: the scan's content ID, frame, placement, scale and offset in steps, and bounds. Scans stay out of the operation log and out of solids (section 9).

**History is pruned only on purpose.** Names cost space: in FreeCAD's informal test on 148 objects, files grew from 17.8 to 22.6 MiB with naming on ([07](../../../kit/notes/G01-evidence/07-persistent-naming.md), section 2). But the references that keep history alive live in programs, out of a save's sight. So saving never prunes. A `PruneHistory` operation takes the set of names held outside the kernel as input and is recorded in the log, so every replica prunes alike; without that set nothing is pruned. A reference reaching pruned history resolves to `HistoryPruned`, never `Deleted`.

## 24.5 Containers

A project file is a container of SKG records. For Spare Construct's proposed SQLite container ([Storage and scale](../../spare-construct/ARCHITECTURE.md#storage-and-scale)):

- A blob holds whole records with their flags and CRCs: a body index, a chunk, or a document's header and frames. Blobs are keyed by content ID, so every lookup verifies itself.
- Header and frames live in one blob per document; every body index names its frame by content ID and carries its own kernel edition.
- The label and END record exist only in standalone files. Inside a container, integrity is each record's CRC plus its content ID.
- Damage stays local: a failing record opens its body read-only with a structured error, and a bad chunk never stops the file (section 23). Salvage writes intact records out as a standalone file.
- Container to `.skgf` and back gives the same records, byte for byte.

## 24.6 Versions, and reading forever

- **The format version** (major.minor) is in the header. A minor version only adds what an older reader can skip or fall back from; a major version runs beside the old one.
- **Each record type is versioned on its own.** A decoder for every tag and schema version ever published is kept, upgrading to the current value in small steps tested on archived files (EXTENDING.md, section 3).
- **Kinds are permanent** (section 26.1). Opening with fallbacks is an explicit read option, because a fallback is less exact (principle 3): the bodies open read-only and are listed in the report; editing one needs an acknowledgement recorded in the log; their original bytes are copied unchanged.
- **The kernel edition** is recorded per document and per body. Nothing is re-evaluated on reading (section 26.2); moving editions is an explicit upgrade operation.
- **The naming scheme** is recorded. Each `NameId` is recomputed from its encoding under that scheme, and a mismatch is damage. A new scheme is a new version, adopted by an operation that records the mapping.
- **Writing.** A writer writes the current version, or an older minor version when the content fits, and otherwise refuses with a typed error listing the records that need the newer one. We may stop writing a version; we never stop reading it.
- **The archive corpus** in `geom-test` keeps files from every released version, plus real files contributors give. Every pull request reads all of them on every target, and again through a simulated older reader that exercises every fallback. None is ever deleted.
- **The specification is the source of truth.** `SPEC.md`, the registry and the CDDL describe every record; field tables are generated from code annotations. A second reader in Python, written from the specification alone, lives in `geometry/tools/format-reader/` (27, accepted in 32.13) and must produce the same text dump of the whole archive. Two independent interoperating implementations is the IETF's bar for a full Internet Standard ([RFC 6410, section 2.2](https://www.rfc-editor.org/rfc/rfc6410#section-2.2)); we hold our format to it.
- **A lossless text form**: CBOR's extended diagnostic notation with every float as a hexadecimal float (RFC 8610, Appendix G.5), converting both ways bit for bit. For diffs, fixtures and AI agents; never for storage.

## 24.7 Reading untrusted bytes, and trust on open

Every reader, native or exchange:

- reads through two `no_std` traits the program supplies, so one code path reads a file, a network range, a zip entry or a SQLite blob, and the result never depends on how the bytes arrive (fuzzing feeds random split points):

  ```rust
  pub trait ByteSource  { fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError>; }
  pub trait RangeSource { fn len(&self) -> u64;
                          fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError>; }
  ```

- checks every length against the bytes remaining and the memory budget before allocating;
- walks nested structures with an explicit stack, never unbounded recursion, because WebAssembly stacks have no guard page (principle 13);
- returns a typed error with byte offset or line, never panics, honours budget and cancellation, and publishes nothing from a cancelled read;
- is a fuzz target from its first commit (`geom-fuzz`).

**XML and zip** (3MF, IPC-2581, LandXML, later ifcXML) go through one hardened reader in `geom-exchange`: no document type definitions or external entities, so external-entity and entity-expansion attacks have nothing to act on; budgets on depth, attributes and text; zip entries checked against the central directory, duplicate and path-escaping names refused, part names validated as the Open Packaging Conventions require, and decompression held to size and ratio budgets. Inflate and deflate are ported into `geom-exchange` from miniz_oxide (MIT, Zlib or Apache-2.0; [`forbid(unsafe_code)`, `no_std`](https://github.com/Frommi/miniz_oxide/blob/cdd7ddd56cf57a6255203bb9f5c6a92bf1144bbf/miniz_oxide/src/lib.rs#L25-L26)), keeping its notice (29.20, rule 4), and run inside our budgets. It is a PORT, not a dependency, because inside kernel crates the only DEPEND is `libm` (29.1). The port needs a row in section 29 and a kernel owner's yes before it lands.

**Range checks keep the predicates sound.** Section 6.2's fixed-width predicates are exact only inside their bit budgets, and Rust integer overflow wraps silently in release builds. A file can pass every checksum and still hold a coordinate near 2^50. So before building any body, every reader checks, and every importer checks again after numbers reach the grid:

- grid coordinates within ±(2^40 − 1) steps of the frame origin (section 6.1), and `f64` coordinates, control points included, finite and within the same range;
- planes reduced, normals within 83 bits and offsets within 125;
- each `Meet` vertex's three planes independent (an exact determinant test), and every other plane through it passing through it exactly;
- NURBS well formed: knots non-decreasing, multiplicities at most degree plus one, weights positive, degree within the registry's limit.

A failure is `OutOfRange { record, offset, field, bits }` or `Malformed`, never a body. Property tests and Kani proofs sit at ±(2^40 − 1) and just beyond.

**Trust on open.** A content ID proves bytes are intact, not valid, and not who wrote them. The program states the trust of what it opens; the kernel never infers it from the file.

- *Known*: the program's own store, or a source whose integrity it checks, such as a signed sharing package. Each body index carries a validation stamp (checker version, groups passed, kernel edition). With a stamp from a checker no newer than the reader's, only the range checks and the cheap combinatorial group run, which keeps the kernel's own loops finite. Section 23's open share assumes this.
- *Unknown*: everything else, and stamps from older checkers. The full checker (section 8.3) runs lazily, chunk by chunk, within budget. A failing body opens read-only with a structured error and is never an operation input until healed by an operation in the log.

Foreign files are always unknown: every import runs the full checker before publishing (section 8.3).

## 24.8 Importing

Three new crates serve every format crate (open questions):

- `geom-exchange`: each format unit as an exact ratio of grid steps and angle units; numbers to grid; frames from files (frames themselves are `geom-math`'s, section 17.1); the ISO 10303-42 entity mapping, shared by STEP and IFC because IFC's advanced B-rep is adapted from ISO 10303-514 ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A2); the hardened XML and zip reader; reports.
- `geom-p21`: the ISO 10303-21 clear-text encoding STEP and IFC share. Schema-free: tokenizer, streaming parser, canonical writer.
- `geom-heal`: healing and sewing Tolerant B-rep bodies (24.9) and rounding imported polyhedra (below). These are the only Repair operations of section 10.2 it holds. The others keep their homes: `close_near_contacts` in `geom-bool` (section 11.5), mesh heal and reconstruct where section 19.7 places them, and defeaturing, a face edit, in `geom-offset` (section 13.13). Rounding imported polyhedra calls mesh heal's quantise and weld (section 19.7, steps 1 and 2, with a weld distance of zero), never a second copy.

**Import:**

1. **Decode** bytes into the format's own entity graph.
2. **Units and frame.** Where a format declares no unit (STL, OBJ, PLY, PTS, XYZ, DXF with `$INSUNITS` 0), the caller names one; the importer never guesses.
3. **Numbers to grid**, below.
4. **Map** each item to a foreign recipe in the 24.4 envelope, keeping every source attribute (an IfcExtrudedAreaSolid of an IfcIShapeProfileDef stays exactly that), plus an evaluated body in native kinds: the plugin-shape pattern of section 26.1 and IfcOpenShell's taxonomy layer ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3). An unmapped entity becomes `Unsupported { entity, source_id, remedy }`. Truck's STEP reader drops edges that fail to convert with a printed warning ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 1); nothing is dropped silently here.
5. **Round polyhedra and check**, below. Edge gaps are measured with certified bounds, not samples: all within ε_fit is Certified; larger, up to ε_heal, is Tolerant.
6. **Heal** within ε_heal (24.9).
7. **Report**, as data: source hash, format, units, frame, declared precision, ε_heal used, each body's class, Tolerant flag, bounds and **import deviation** (how far its stored numbers moved from the file's, recorded apart from its class), and every rounding, healing action and unsupported item. One failed element is one failed item, never a failed file.

The same bytes, options and kernel edition always give a bit-identical model.

**Numbers to grid.** A decimal length is parsed straight into integers (digits, exponent, the unit's exact ratio, the step) and rounded once, half to even, never through `f64`; a binary float is taken at its exact value and rounded once. Which attributes are lengths comes from the schema's measure types (IFC's IfcLengthMeasure family, STEP's length measures); a positive length that rounds to zero is an error offering a finer frame. Other reals become the nearest `f64`, and come back as the same decimal on export when it had at most 15 significant digits. A rotation or scale into a frame runs in double-double arithmetic and rounds once, within half a step plus about 2^-100 of the coordinate's size (derived). Exactness depends on unit and step:

| Unit | Exact at q0 (buildings, sites) | Exact at q1 (parts, boards) |
| --- | --- | --- |
| Millimetre, metre | Every value to 0.01 mm | Every value to 0.1 µm |
| Inch | Multiples of 1/1,280 inch (0.1, 0.05, 1/256 inch) | Every value to 0.001 inch |
| Foot | Every value to 0.1 foot | Every value to 0.001 foot |
| US survey foot (1,200/3,937 m) | Never, except multiples of 3,937 feet | The same |
| Degree, gon | To 10^-6 degree or 10^-5 gon: whole angle units (section 17.2) | The same |
| Radian | Never, except zero: kept as `f64` | The same |

One inch is 162,560 q0 = 2^8 × 5 × 127 steps, so a decimal inch is exact at q0 only in whole 1/1,280 inches. Imperial mechanical work should use q1 frames. Every rounding is recorded; at q0 none exceeds 78 nm.

**Angles and trims.** Exact angles are kept as angle units, so a quarter turn stays an exact rotation (section 7); a radian value is snapped to an exact angle only as a recorded healing action the caller asks for. Trimmed curves honour the file's preferred master representation (parameter or point); parameter trims on conics use the file's angle unit; a point and parameter disagreeing by more than ε_heal give `TrimDisagreement`.

**Frames from files.** IFC's [IfcMapConversion](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcRepresentationResource/Entities/IfcMapConversion.md) becomes section 17.1's frame record, and local coordinates become frame coordinates unchanged. The origin comes from the decimals, exact when on the grid and otherwise rounded once and recorded; the rotation, given as two numbers (XAxisAbscissa, XAxisOrdinate), becomes the nearest angle unit, the difference recorded and the two numbers kept in the foreign recipe so an unedited frame writes back exactly; the scale is an exact decimal ratio. IfcMapConversionScaled maps to a Ground frame when FactorX equals FactorY and FactorZ is one; other factors stay in the recipe and the geometry goes through them once, rounded and recorded (open questions). Files holding raw map coordinates (around 6,000,000 m, about 2^45 steps from a zero origin) go into the frame on section 17.1's lattice nearest their bounding box's centre; wider data splits across lattice frames or fails with `FrameRangeExceeded`. No coordinate outside the range is ever stored.

**Rounding imported polyhedra without breaking them.** Rounding vertices can flip an orientation, collapse a triangle, merge vertices or make faces cross; EMBER warns of the same at export ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4). So, recording every step:

1. **Weld on source values**: vertices that are one instance, or equal bit for bit, are one vertex. STL is welded this way only.
2. **Round** each vertex once. Vertices landing on one grid point merge (`Merged` names, section 20.2); collapsed edges and faces are removed and reported.
3. **Planes from grid points.** A face whose rounded vertices are exactly coplanar gets the reduced plane through a canonical three of them, which meets section 6.2's 83- and 125-bit budgets by construction. A face no longer flat is split by a deterministic triangulation, coplanar pieces rejoined, the pieces named `Split`, marked weak.
4. **Planes given by a direction** (IfcPlane, half-spaces, STEP `plane`) become the plane through three grid points: the rounded location and two points a power-of-two distance L along the plane, rounded, L as large as range allows. Offset moves at most q/2, angle about q/L, both recorded.
5. **Check exactly.** Where rounding made faces cross or flipped an orientation, that region is re-rounded by vertex rounding of a polyhedral subdivision, which keeps each face within a bounded distance and changes topology only by merging ([Fortune 1999](https://doi.org/10.1007/PL00009480); [Devillers, Lazard and Lenhart 2020](https://doi.org/10.1007/s00454-020-00202-2); abstracts read, details *unverified*).

Whole-model 3D rounding with proven guarantees is solved in theory but costly: O(n^4) output vertices for Fortune, O(n^13) worst case for Devillers and colleagues, who expect far less in practice. No open kernel applies either. Spare Kit leads by rounding plainly, checking exactly and applying the method only where the checker flags. It promises meanwhile that no rounded body is published invalid: past its budget, the repair fails with remedies, such as a finer frame (25 times smaller moves) or a Sheet body.

**What is exact on import, and what is healed:**

| The file has | Import | Result |
| --- | --- | --- |
| Lengths on the grid | Taken as they are | Exact; deviation zero |
| Lengths off the grid | Rounded once | Deviation at most q/2 per axis |
| Lines, arcs, conics, quadrics, tori, spirals | Kept as their own kinds | Exact definitions; spirals evaluate Certified |
| Faceted solids, triangle and polygon sets, terrain networks | Polyhedra or exact meshes, rounded as above | Exact, with recorded deviation, splits and merges |
| Curved B-rep within the sender's tolerance | Tolerant, then healed (24.9) | Certified within ε_fit, Exact where all faces are flat, or still Tolerant |
| Gaps larger than ε_heal | Not a solid. The error names edges and gaps, with remedies in order: an exact Sheet or General body with free edges and a "declared solid, not closed" diagnostic; ε_heal raised up to the cap; a mesh, Approximate, on request | Error, then the chosen remedy |

**ε_heal.** The default is 640q (section 6.5). The importer also proposes the file's declared precision (IFC's [Precision](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcRepresentationResource/Entities/IfcGeometricRepresentationContext.md), "the tolerance under which two given points are still assumed to be identical", or STEP's uncertainty, [`global_uncertainty_assigned_context`](https://www.steptools.com/stds/stp_aim/html/t_global_uncertainty_assigned_context.html), found through a third party's pages, 24.11), raised to 640q when smaller and capped at 6,400q. The proposal is listed in the report and applies only when the caller accepts it. The caller may pick any value from one step to the cap; used and declared values are both recorded (section 6.5). Where tolerance is part of a format's meaning, the declared value applies: an IfcArcIndex arc whose points are collinear within Precision is a polyline ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A1). IFC does not define the measure; we use the middle point's distance from the chord, decided exactly, and the report says this is our reading.

**Names.** Imported entities are `Created { op, role: Imported(key), from: [] }`, keyed by the source's persistent ID: STEP persistent IDs ([MBx-IF practices](https://www.mbx-if.org/home/cax/recpractices/), version 1.8, *unverified*), an IFC GlobalId plus the item's path, a 3dm object ID; otherwise the instance number, marked weak. Edges and vertices without IDs get `Bounded` names from their faces. **Re-importing a revised file is a new version of the same import operation** (same `OpId`, new source bytes as a changed input), as changing an extrusion's height keeps its operation. Every name whose key is unchanged comes back identical; the report lists weak ones that may not.

## 24.9 Healing

Healing follows Parasolid's published tolerant model, where only imported edges and vertices carry tolerances (Jackson 1995; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 2), and covers the defects OpenCascade's shape-healing guide lists (method only, in our words; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 5). OpenCascade heals by raising tolerances; `geom-heal` replaces geometry and never raises one. Each step is a recorded action in one operation:

0. **Certify the sender's curve first.** Each Tolerant edge's 3D curve is bounded against both surfaces. Within ε_fit of both, it becomes the primary curve, Certified, its pcurves derived by certified projection (section 8.1). No intersection is needed; this makes most edges of a good file Certified, fillet boundaries included.
1. **Transversal edges** (normals certified to stay apart): replaced by the certified intersection branch inside the tolerance tube (`geom-intersect`, section 12). Two planes give an exact line.
2. **Tangent and near-tangent edges**, such as fillet boundaries, where intersection is ill-conditioned and marching fails ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 3; Patrikalakis and Maekawa, [section 6.4](https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node122.html)): the sender's curve is projected onto the surface of the face whose name sorts first and its distance to the other bounded; within ε_fit it becomes the Certified edge, otherwise it stays Tolerant. ACIS relaxes near-tangency for the same reason ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 2). Certified tangential intersection is where Spare Kit leads next; until then these are honest Tolerant edges, never guessed ones.
3. **Vertices** become the certified meeting of their edges: exact meets of planes where every face is flat and the planes meet in one point, grid points otherwise.
4. **Sewing** follows Barequet and Kumar ([1997](https://doi.org/10.1145/266989.267105); citation verified, content *unverified*): free edges pair only when their curves lie within ε_heal along their whole length, by a certified bound, and the match is unique, candidates taken in source-key order. Otherwise nothing in the group is sewn and the report gives `Ambiguous` with the candidates. Missing pcurves come from certified projection; seams and period-shifted pcurves are fixed; orientation is fixed by exact tests.
5. **Edges shorter than one step** after rounding collapse. Nothing else is removed: short edges are legal (section 6.5), and small-feature removal is the explicit defeature operation (section 13.13).
6. **The rest stays Tolerant.** Operations work within its tolerances or refuse with `ToleranceExceeded`.

One-to-one modified entities keep their names; merged vertices get `Merged` names (section 20.2). The healed fraction (Certified at step 0, at steps 1 and 2, still Tolerant) is reported per file and tracked across the STEP corpus.

## 24.10 Exporting

1. **Choose the form**: the richest the target holds exactly: the recipe where the target has a parametric match (an IFC extrusion, a CSG tree), else a B-rep in native kinds, else a tessellation.
2. **Convert what the target lacks** through `geom-convert`, with its bound (matrix below).
3. **Commit numbers.** Metric grid values are exact decimals (one q0 step is 0.00015625 mm). In inches or feet they generally do not terminate, so the writer writes the shortest decimal that reads back to the same grid point: the fixed point holds and the sub-half-step difference is reported. `f64` values are correctly rounded and written relative to the frame origin, so within 1/8,192 of a step; `f32` formats are relative to each element's origin, the error reported. Plane-based vertices are computed exactly, then rounded once.
4. **Re-check what is written** with exact predicates (orientation, self-intersection). A flip fails the export with `ExportRoundingFlip { faces }` and remedies: a finer unit or more digits; local snap rounding with recorded deviation, by the import method; splitting the faces; or an explicit `accept_invalid_rounding`, recorded in the report and the file's metadata.
5. **State the bound** in the file where possible (IFC Precision, STEP uncertainty, glTF `extras`) and in the report. Before writing a Precision P, the writer rewrites any IfcArcIndex arc collinear within P as an IfcTrimmedCurve on an IfcCircle, and checks no two distinct vertices lie closer than P; a conflict is `ExportPrecisionConflict`, with remedies (a smaller P, or curves re-fitted tighter so P can shrink). A 1 km radius façade arc in 0.2 m segments has a 5 µm sagitta: under a 0.01 mm Precision, a receiver would draw it straight.
6. **Write deterministically**: canonical instance order; each `f64` as its shortest round-trip decimal (Ryū, [Adams 2018](https://doi.org/10.1145/3192366.3192369)); zip compression, by the ported deflate of 24.7, pinned by the kernel edition; timestamps and authors as inputs, since the kernel reads no clock (section 10.1).

**What each kind becomes.** The full matrix (every kind against IFC 4.3, IFC4, IFC2x3, AP242, AP214, DXF, 3dm, glTF, 3MF, OBJ, STL, PLY) is generated from writer annotations, and every cell is a test. Its core:

| Kernel kind | IFC 4.3 | STEP AP242 | DXF | 3dm |
| --- | --- | --- | --- | --- |
| Lines, circles, arcs, ellipses, NURBS, bulge polylines, composites | Exact | Exact | Exact | Exact |
| Parabola, hyperbola, polynomial curve | Own entity or exact B-spline | Exact | SPLINE: Exact | Exact |
| Helix | B-spline: Certified | B-spline: Certified | HELIX: Exact (*unverified*) | Certified |
| Clothoid; other IFC spirals | Exact | Clothoid Exact (in AP242 *unverified*); others Certified | SPLINE: Certified | Certified |
| Circular involute | Certified | Exact | Certified | Certified |
| Offset curve | Exact | Exact | Lines and arcs Exact; else Certified | Certified |
| Intersection curve | Certified fit | Certified fit | Certified | Certified |
| Plane, cylinder, sphere, torus, revolution, extrusion | Exact | Exact | Mesh: Certified | Exact |
| Cone | Revolution of a line: Exact (IFC 4.3 has no cone) | Exact | Mesh | Exact |
| Ruled surface | B-spline: Exact if rails are exact NURBS, else Certified | Same | Mesh | Same |
| Sweep, offset surface, rolling-ball blend | B-spline: Certified | Matching sweeps and offset_surface Exact; blends Certified | Mesh | Certified |
| Subdivision limit surface | NURBS: Exact on regular regions, Approximate at extraordinary points | Same | MESH cage (*unverified*) | SubD where rules match |
| Exact mesh | Exact | Exact | Exact | Vertex precision *unverified* |

IfcAdvancedBrep edges may only be lines, conics, polylines and B-splines ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A1), so spirals and offset curves on edges go as B-splines. 3MF, OBJ and text PLY carry exact decimals; glTF and STL are `f32`, Certified by their bound; curved surfaces in any mesh format are Certified by the tessellation bound (section 19).

**Lossless, defined.** (a) Our own output: write, read, write gives identical bytes in every format, on every pull request ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D). (b) A foreign file read and written back keeps every geometric item and placement with its kind and attributes; non-length reals return as the same decimal when it had at most 15 significant digits; lengths move at most q/2, and not at all on the grid. Relationships such as IFC's IfcRel entities belong to the program.

## 24.11 Format by format

| Format | Crate | Stage |
| --- | --- | --- |
| IFC 4.3 (ISO 16739-1:2024), IFC4, IFC2x3; IFC4x1 and 4x2 alignments as far as they map | `geom-ifc` | First |
| STEP AP242 (ISO 10303-242:2025) and AP214; AP203 read | `geom-step` | First; semantic PMI later |
| DXF | `geom-dxf` | First; DWG is an owner's decision after legal advice (29.14) |
| glTF 2.0, OBJ, STL, PLY, 3MF 1.4 | `geom-meshio` | First; glTF and 3MF after a terms check gives them rows in section 29 (29.19) |
| E57, LAS, LAZ, PTS | Program adapters, calling `geom-exchange`'s rules through `engine-geometry` | First for LAS and PTS; E57 after a terms check (29.19); LAZ after an owner reviews laz-rs (29.12) |
| LandXML 1.2 | `geom-landxml` (section 17.11) | First, after a terms check gives it a row in section 29 (29.19) |
| 3dm | `geom-3dm` | Blocked until McNeel confirms the licence in writing |
| 3MF manufacturing extensions; board formats; others | Below | Later, after a legal check or decision note |

Each format crate's design note, in the Design stage, holds its entity-by-entity mapping. This section fixes what each must do.

**IFC.** `geom-ifc` maps every representation type of [IfcShapeRepresentation](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcRepresentationResource/Entities/IfcShapeRepresentation.md), both ways:

| Representation types | Kernel mapping | After import |
| --- | --- | --- |
| Point, PointCloud (deprecated) | Points; large sets as point-cloud references | Exact |
| Curve, Curve2D, Curve3D, Segment, GeometricCurveSet | Curve kinds; IfcArcIndex by its three points; every IfcSpiral; IfcCurveSegment, IfcGradientCurve, IfcSegmentedReferenceCurve via `geom-align` | Exact kinds; spirals Certified |
| Surface, Surface2D, Surface3D, AdvancedSurface, SectionedSurface | Surface kinds; bounded surfaces become faces | Exact or Certified |
| FillArea, Text, Annotation2D | `geom-2d` regions, hatches, text anchors | Exact |
| SweptSolid, AdvancedSweptSolid | `geom-sweep` recipes: extrusions (tapered with ruled sides), revolutions, disk sweeps with half-angle mitres, directrix sweeps, IfcSectionedSolidHorizontal | Exact for flat and quadric results, else Certified |
| CSG, Clipping | `geom-bool` recipes, every half-space kind. No cutter is dropped for being thin or touching | Exact for flat results |
| Brep, AdvancedBrep | Faceted: plane-based polyhedra. Advanced: the shared ISO 10303-42 mapping | Exact, Certified, or Tolerant then healed |
| Tessellation, SurfaceModel, SurfaceOrSolidModel | Face sets (honouring PnIndex); IfcTriangulatedIrregularNetwork with void, hole and breakline flags into `geom-terrain` | Exact |
| SolidModel, GeometricSet, BoundingBox | As above; a box | Exact |
| SectionedSpine | A recipe only: IFC leaves the interpolation undefined, so no solid unless the caller picks a recorded loft rule | Recipe |
| MappedRepresentation | One shared body placed by a transform; non-uniform scale changes kinds explicitly (section 7) | As the source |
| LightSource | Not geometry: placement kept, the rest to the program | |

- **Profiles**, every parameterised and arbitrary one, stay foreign recipes and write back as the same profile.
- **Placements**: IfcLocalPlacement chains, IfcGridPlacement and IfcLinearPlacement (stations along an alignment, new in 4.3, via `geom-align`). Grid shifts and axis-aligned rotations are exact; other rotations place a shared body by a general transform, made into geometry lazily and labelled with its class (sections 7 and 23).
- **Openings**: the program's adapter reads IfcRelVoidsElement; `geom-ifc` offers one batch operation that evaluates a host with its openings and cuts them with `geom-bool`, as IfcOpenShell's opening hook does ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3), so every program cuts alike. The regression corpus is seeded where IfcOpenShell and web-ifc fail: rebar, MEP sweeps, half-space cuts, alignment sweeps, advanced B-reps with holes.
- **Export** writes `Body` as the most parametric exact form, optionally with `Body-FallBack` (an identifier IFC defines) carrying the evaluated B-rep or tessellation and its deviation. IFC2x3 gets tessellations as faceted B-reps or surface models, exactly, and advanced B-reps as faceted B-reps Certified by their tessellation bound, listed in the report. ifcZIP both ways; ifcXML later; IFC5 gets a decision note on publication.
- **Licence.** The documentation is CC BY-ND 4.0: link and quote, never adapt; our documentation is written fresh. Its copyright notice lets technical documentation be "used in software development, or translated into another computer language" with full attribution ([LICENSE.md](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/LICENSE.md)), so Rust types are generated, with attribution, from the official pinned IFC 4.3 EXPRESS release, not the development repository linked here. The same file's Copyright Exclusion Notice leaves material based on other standards, such as the ISO 10303 parts, outside that grant. Entity and attribute names are treated as interface facts.

**STEP.** `geom-step` reads AP242, AP214 and AP203 and writes AP242 and AP214, through `geom-p21` and the shared mapping.

- Every Part 42 curve and surface maps to its own kind ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 2). An `intersection_curve`'s 3D curve is the sender's approximation, re-certified (24.9, step 0). Dupin cyclides become rational NURBS, exact by Pratt's construction (*unverified*). B-spline volumes and locally refined splines are not mapped yet.
- Conversion-based units (inches, degrees) are read; the context's uncertainty sets the ε_heal proposal. Assemblies become placed instances; colours and layers attributes; product data goes to the program; multi-file assemblies come later. `geom-p21` reads Part 21's third edition (anchor and reference sections, several data sections, `\X2\` and `\X4\` text; details *unverified*).
- Following the [MBx-IF practices](https://www.mbx-if.org/home/cax/recpractices/) (read as a listing, *unverified*): geometric validation properties written and checked (version 4.6), persistent IDs carrying `NameId`s (1.8), tessellated geometry (1.1).
- **Every written file passes a validator generated from the EXPRESS schemas** (types, WHERE rules, the AP's global rules), and independent readers read our files nightly. A young Rust STEP writer shows why: it pointed a LINE at a DIRECTION instead of a VECTOR and wrapped an ADVANCED_FACE's one surface in a list, both against the schema, and OpenCascade read zero faces while the writer's own check only counted edge uses. Once a third party fixed it, OpenCascade read their four outputs back as closed solids ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), cadcore; [issue #2](https://github.com/YATSKOVSKYI/cadcore/issues/2)).
- Semantic PMI comes later; it needs this kernel's persistent names and a PMI model owned by the mechanical program.
- **Licence.** ISO sells the parts of ISO 10303. Entity knowledge comes from licensed copies of ISO 10303-21, ISO 10303-42 and the application protocols read and written, implemented without copying their text (29.14). So `geom-p21`, `geom-exchange`'s shared Part 42 mapping and `geom-step` start only once those copies are obtained. STEP Tools' public pages, a third party's rendering of the schemas, are used only to find entities, pending an owner's check (rule 11). Where AP schemas may be obtained and what may be generated from them is *unverified* and needs an owner's check. Parser methods are studied in ruststep (Apache-2.0) and foxtrot (MIT or Apache-2.0); we write our own ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), section 6).

**DXF.** `geom-dxf` follows Autodesk's public 2018 DXF reference ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md), section 1), whose ENTITIES section lists 45 entities ([index](https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-7D07C886-FD1D-4A0C-A7AB-B4D21F18E484.htm)):

| Entities | Mapping |
| --- | --- |
| LINE, XLINE, RAY, CIRCLE, ARC, ELLIPSE, SPLINE (fit points kept as intent), LWPOLYLINE, POLYLINE with VERTEX and SEQEND, HELIX, POINT | Curve kinds and Acorn bodies: Exact |
| Thickness (group 39) on curves, points, SOLID and TRACE | Extrusion recipes: Exact |
| HATCH (loops, island rule, pattern family), SOLID, TRACE, WIPEOUT, MLINE | Regions and offset curves: Exact |
| TEXT, MTEXT, ATTRIB, ATTDEF, SHAPE, TOLERANCE | Text anchors with extents; content as attributes |
| DIMENSION, LEADER, MLEADER, MLEADERSTYLE | DIMASSOC as typed snap references; the dimension's block kept as its evaluated geometry, regenerated only when its references change |
| INSERT (with rows and columns), blocks | Placed instances; non-uniform scale makes ellipses explicitly |
| 3DFACE, polyface and polygon meshes, MESH | Exact meshes; MESH to `geom-subd` where its rules match (*unverified*) |
| IMAGE, UNDERLAY, VIEWPORT, SECTION, TABLE, LIGHT, SUN | Clip boundaries as exact regions; table geometry from its block; the rest to the program |
| 3DSOLID, REGION, BODY, SURFACE, ACAD_PROXY_ENTITY, OLEFRAME, OLE2FRAME | Proprietary or opaque: unsupported, bytes kept as an attribute, remedy "export STEP from the source program" |

Arc-length dimensions are not in the reference, so they are written as block geometry and read only as their block. The entity coordinate system is handled at the boundary; binary DXF and pre-2007 code pages are read as the reference describes; spirals go out as SPLINE with their bound. ezdxf (MIT) is the reference reader and an oracle. **DWG** is an owner's decision after legal advice (29.14): LibreDWG is GPL and never used, and other DWG knowledge traces to reverse engineering or unclear provenance ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md)). A DWG writer would reuse `geom-dxf`'s mapping.

**3dm.** openNURBS's LICENSE file holds a copyright line, a disclaimer and the zlib text, but no grant; the permission is only on McNeel's web page ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 1). Nothing is built until McNeel confirms in writing. Then `geom-3dm` ports what it needs, keeping the licence: knot-vector conversion (3dm stores two fewer knots than STEP), tolerances imported as Tolerant bodies, SubD cages where Rhino's rules match ours ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md)). Meanwhile, Rhino users exchange through STEP.

**Meshes.** `geom-meshio`: STL (twelve 32-bit floats per triangle, no unit, [Library of Congress](https://www.loc.gov/preservation/digital/formats/fdd/fdd000505.shtml)); OBJ and PLY (no unit; OBJ carries subdivision cages but not creases); glTF 2.0 (metres, +Y up, `f32` positions, [specification L694-701](https://github.com/KhronosGroup/glTF/blob/7cee0fcddd5730e90d488bfb5aa2e52171483dde/specification/2.0/Specification.adoc#L694-L701); +Y to +Z is an exact quarter turn; export for display, Certified by its stated bounds, and the base of Spare Construct's 3D Tiles); 3MF core 1.4 (units from micron to metre, inch and foot included, [L415](https://github.com/3MFConsortium/spec_core/blob/997b385e06f3181cf9aae0c578e0b45ccd48ccb2/3MF%20Core%20Specification.md#L415); decimals "to arbitrary precision", [L639](https://github.com/3MFConsortium/spec_core/blob/997b385e06f3181cf9aae0c578e0b45ccd48ccb2/3MF%20Core%20Specification.md#L639); closed outward-facing meshes, [L577](https://github.com/3MFConsortium/spec_core/blob/997b385e06f3181cf9aae0c578e0b45ccd48ccb2/3MF%20Core%20Specification.md#L577); so exact metric grid meshes round-trip exactly). Imported meshes are repaired by `geom-mesh`: exact welding, orientation by exact tests, hole filling only on request, each step recorded.

**Point clouds.** The kernel reads no scan files (section 17.10). Spare Construct's adapter reads LAS with [las-rs](https://github.com/gadomski/las-rs/tree/60a73c02f7ae399452f64d6f74aac7168029b075) (MIT) and stores scans as COPC (24.4). Two readers wait:

- E57 with the [e57 crate](https://github.com/cry-inc/e57/tree/c759b1da81184bea3db5e093e7416729cb77cd27) (cry-inc, MIT) needs a row in section 29, and E57 itself a terms check (29.19), before use.
- [laz-rs](https://github.com/laz-rs/laz-rs/tree/e82f4453a3c83a9fb1309eb2167d3d9bad2052da) is stop and ask (29.12, rule 2). Its README calls it a translation of LASzip, which became Apache-2.0 only [in 2022](https://github.com/LASzip/LASzip/commit/8d82288effd827ee90bcb2f1f24cb3cc49026f5c). LASzip's earlier licence, believed to be LGPL, and when the translation began are *unverified*. laz-rs is not used until an owner reviews its chain of title against that earlier licence. COPC is LAZ, so until then LAZ and COPC go through an out-of-process tool, or wait for a decision note.

None of these crates is a kernel dependency: laz-rs uses `unsafe` (for example [packers.rs L47-73](https://github.com/laz-rs/laz-rs/blob/e82f4453a3c83a9fb1309eb2167d3d9bad2052da/src/packers.rs#L47-L73)) and all three use standard file input and output, against principle 13 and section 21's `no_std` rule. If a second program needs them, they move into Spare Kit beside the kernel. `geom-exchange` implements the conversions below, and adapters call them through `engine-geometry`, the only crate a program may depend on (section 5):

- LAS 1.4 and 1.5 ([ASPRS source](https://github.com/ASPRSorg/LAS/blob/997fc22457171669f2805831221048a4a738d828/source/02.05_point.sub#L107-L110); 1.5 approved August 2025) and COPC store `int32` times a scale plus an offset. A scale is read as the shortest decimal giving its `f64` (0.001, not 0.00100000000000000002...), our reading of the intent. A scale of whole steps (1 mm is 6,400 q0) with an offset on the grid makes every point exactly a grid point.
- E57 (ASTM E2807, *unverified here*) keeps each scan in its frame with its pose. Scaled integers convert exactly, floats round once; spherical coordinates convert with the kernel's deterministic maths, rounded once to the COPC scale, the bound (half the scale plus maths error times range) stored with the scan.
- PTS and XYZ: decimals to grid, unit named by the caller. Every per-point attribute, LAS extra bytes included, passes through unchanged.

**Circuit boards, later.** Board frames use q1 (section 6.1). Layers become `geom-2d` regions with exact arcs; traces stroked by round apertures, exact lines and arcs; drills, exact cylinders; nets and components, attributes. Every format needs an owner's legal check first:

- **Gerber X2 and X3** ([Ucamco](https://www.ucamco.com/en/gerber/downloads), revision 2026.05, drill and rout layers in its section 6.6). Section 1.4 grants no licence to the document's intellectual property and asks interface developers not to make "derivative versions, modifications, or extensions" or "alternative interpretations of the data" without Ucamco's approval. Section 4.2 requires six decimals, so millimetre files resolve 1 nm, and says imperial "will be deprecated". Coordinates on a 25 nm lattice (every micron and mil grid) are exact at q1 both ways; others round by up to 3.1 nm, recorded; inch files resolve a micro-inch, exact on no step (open questions).
- **Gerber job files, XNC and Excellon drill files, IDX (ProSTEP iViP), IDF 3.0**: terms *unverified*. **IPC-2581 revision C** is sold by IPC ([listing](https://shop.electronics.org/ipc-2581/ipc-2581-standard-only)); that the format is licence-free comes from secondary sources only (*unverified*). **ODB++** is Siemens'; not read until cleared. The 3D board travels as STEP AP242.
- The crates are `geom-gerber` and `geom-ipc2581` once two programs need them (section 26.1); until then, the board program's adapter, calling `geom-2d`'s operations through `engine-geometry`. No program needs board exchange yet; Gerber is implemented exactly as published, with no extensions, and IPC-2581 only after its own text or IPC's word (owner action G447, 32.40).

**Manufacturing, later.** 3MF's extensions map onto kernel kinds: Beam Lattice, Slice, [Volumetric and Implicit 1.0](https://github.com/3MFConsortium/spec_volumetric/blob/75968527c3609805d021b94c7fb6ca2be6e60583/3MF%20Volumetric%20Extension.md) for fields whose every node matches, [Boolean Operations 1.1.1](https://github.com/3MFConsortium/spec_booleans/blob/7b70e6e573b3bd4bdfe781cca10e160ec28497a9/3MF%20Boolean%20operations.md), and Toolpath (a release candidate); what has no match goes as a certified mesh, with the reason. Machined and moulded parts travel as AP242; G-code and STEP-NC belong to a manufacturing program.

**Other formats**, each needing a decision note:

| Format | Owner now | Terms |
| --- | --- | --- |
| CityGML, CityJSON, GeoJSON ([RFC 7946](https://www.rfc-editor.org/rfc/rfc7946)), GeoPackage, Shapefile | Spare Construct's adapter | Open standards (*unverified* except GeoJSON) |
| ASAM OpenDRIVE | The simulation engine's adapter; libOpenDRIVE (Apache-2.0) studied ([10](../../../kit/notes/G01-evidence/10-github-sweep.md)) | ASAM's (*unverified*) |
| 3D Tiles | Spare Construct's writer, on glTF | OGC community standard (*unverified*) |
| USD, Alembic | None | OpenUSD is TOST-1.0, a modified Apache licence; the AOUSD UsdSolid proposal, which NVIDIA's omniSolid schema follows, has separate supplemental terms ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)); Alembic *unverified* |
| FBX; Parasolid XT, ACIS SAT and SAB, JT (ISO 14306) | None | Proprietary or needing legal advice: not read unless an owner clears them; STEP meanwhile |
| OpenCascade `.brep` | None | Defined by LGPL code; NVIDIA's reader mirrors OpenCascade's LGPL reader by its own account, so method study only ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)) |
| IGES 5.3, DGN, X3D, PRC | None | Availability and terms *unverified* |
| IFC5 | Watched | buildingSMART's |

**Subdivision exchange is a gap today**: OBJ has no creases, 3dm is blocked, DXF MESH is unverified. `geom-meshio` owns USD subdivision exchange, with Chaikin creasing in a namespaced attribute (owner action G312, 32.32). Mapping USD's Catmull-Clark scheme, with creases, corners and the options section 16.1 leaves to this chapter, onto `geom-subd` at OpenSubdiv parity ([09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 1) waits for USD's decision note and its TOST-1.0 check.

## 24.12 Testing exchange

- **Fixed point**: write, read, write gives identical bytes for every format, on every pull request; every STEP and IFC file we write passes the EXPRESS validator.
- **Corpora, by licence** ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section D; section 29.15): buildingSMART's sample files (CC BY 4.0, vendored); the IFC 4.3 sample models (CC BY-ND, unmodified); NIST's PMI STEP models, fetched at test time by pinned hash once an owner confirms the terms; [step-conformance](https://github.com/dan335/step-conformance/tree/96f30bbe09549ef37dbd0a956d70bd8a183fd5d2) (code MIT OR Apache-2.0, `corpus/` CC0-1.0, checked 2026-10-21; created 12 October 2026; its shop parts given under CC0 by their author; `corpus/` vendored with credit, the code not taken: an early benchmark); Manifold's models (Apache-2.0); the [3MF conformance suites](https://github.com/3MFConsortium/test_suites/tree/f483d3beee069e7adbac125d1f7baa36b5b3e5bf) (BSD-2-Clause) and ezdxf's files (MIT), once each has a row in 29.15. The alignment test set (no licence) and the ABC dataset are not used until an owner decides.
- **Tolerant bodies stay usable**: a boolean on an imported Tolerant body with 640q gaps succeeds, its stored tolerances unchanged and counted in no ε_fit sum (24.4).
- **Independent readers, nightly, out of process, never linked**: IfcConvert, web-ifc, OpenCascade, ezdxf and NIST's STEP File Analyzer (once an owner confirms its terms) read our output; volumes, areas and Hausdorff distances are compared and every disagreement triaged, the way IfcOpenShell's 12.9% volume error was caught ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), section A3).
- **Fuzzing** of every codec with random read splits; **determinism**: imports and exports join section 21's replay corpus, hashes compared on every target.

## 24.13 What the kernel never does here

- Stores decimal text in its own format, or rounds a number twice.
- Guesses a unit, a frame or a tolerance.
- Raises a tolerance, or heals beyond the recorded ε_heal.
- Drops an entity without a typed reason, or sews an ambiguous match.
- Publishes a rounded body, on import or export, that fails its exact checks, unless the caller asked and the file says so.
- Re-evaluates stored geometry on reading, or prunes history it was not told is unreferenced.
- Writes a file it cannot read back to identical bytes.
- Lets an unknown required record, or a number outside its budget, pass.
- Reads a format without a published specification, or whose terms forbid it, until an owner clears it.
