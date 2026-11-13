# A30: Structure: one discipline for design and fabrication detailing

## The question

On 13 November an owner noted that many people prefer Tekla Structures for steel and concrete detailing, and asked what Spare Construct's structure discipline should do about it. Three options were put:

- **(a) A switch** inside the structure discipline between a way of working like Revit's and a way of working like Tekla's.
- **(b) A separate program** for detailing, on the same model and the same geometry kernel.
- **(c) One structure discipline** that takes the best of both.

This note compares them for the people who use them (modelling steel and precast concrete; connections; plates, bolts and welds; marks; fabrication drawings; assembly and precast unit drawings; NC and IFC exports; sharing the model), for the effort, and for the kernel pieces each one needs. It recommends one, and lists board rows a later lane could open.

Today the map treats detailing as late and outside: "Steel and rebar detailing | Advance Steel, Tekla links | Later phase" ([ARCHITECTURE.md](../ARCHITECTURE.md), the add-ins table), and consultants on Tekla publish IFC into their own team space (Sharing between firms). In the features map, connections and steel fabrication output are two `later` parts (CON-A5-F7-P1 and P2), and shop drawings and cut lists sit in CON-A21-F3.

The note is ours. It describes workflows in plain words and uses none of the vendors' names for their tools, screens or settings. Sources were read on 14 November 2026: vendor-hosted help pages, the steel trade body's published format, buildingSMART's IFC 4.3 source, and two papers. "Unverified" marks anything not confirmed on such a page.

## What people mean by "the Tekla way"

Read across the vendor's help, the preference comes from five things the model does, not from the look of the screens:

1. **The model is the fabrication record.** Every plate, bolt, weld, cut and hole is an object in the model, and parts are grouped into the pieces that are made and shipped (an assembly for steel, a precast unit for concrete). Drawings, lists and machine files are read from the model, never drawn beside it.
2. **Connections are made by rules.** A connection is an object that makes its own plates, bolts, welds and cuts from the members it joins, and changes them when a member moves or changes size.
3. **Identical pieces get one mark.** Numbering compares parts and assemblies and gives identical ones the same mark, which fabrication drawings, reports and NC files all need ([what numbering is](https://support.tekla.com/doc/tekla-structures/2026/mod_what_is_numbering)). "Identical" is decided by geometry within a tolerance (1 mm by default for steel, measured point by point), by profile, material and finish, by marking on the part, and by any property a firm sets to count. Parts are compared against older parts, so the order parts were made in can change the result ([why identical parts get different marks](https://support.tekla.com/article/why-identical-parts-get-different-part-position), [list of things affecting numbering](https://support.tekla.com/knowledge/list-of-things-affecting-numbering)).
4. **Drawings are made per piece and kept up to date.** There are three kinds of fabrication drawing: one part, one assembly, and one precast unit ([create fabrication drawings](https://help.trimble.com/doc/tekla-structures/2026/dra_create_fabrication_drawings)). Precast unit drawings show formwork, dimensions, reinforcement and cast-in items, one drawing serving all identical units ([precast unit drawing examples](https://support.tekla.com/doc/tekla-structures/2026/dra_cast_unit_drawings)). When the model changes, the drawings it affects are flagged, and an out-of-date drawing cannot be opened until it is updated ([update drawings when the model changes](https://support.tekla.com/doc/tekla-structures/2026/dra_updating_drawings_when_the_model_changes)).
5. **Machine files come straight from the model.** NC data is written in the steel trade body's DSTV format, usually one text file per part named by its mark, holding the part's holes and slots, cut outlines and marking ([NC files](https://help.trimble.com/doc/tekla-structures/2026/int_create_nc_files)). The format is published by bauforumstahl ([Standard description of steel parts, 8.0](https://bauforumstahl.de/wp-content/uploads/2024/04/109_Standartbeschreibung_von_Stahlbau-Teilen_8.0.pdf)).

Sharing is the sixth thing people value: several people work on one model, either with local copies whose changes are sent and fetched through a service, or with one master model on the network that each person saves into. The two ways cannot be mixed on one model, and the vendor advises giving each person their own area to avoid conflicts ([sharing a model](https://support.tekla.com/doc/tekla-structures/2026/ms_what_is_model_sharing), [multi-user mode](https://support.tekla.com/doc/tekla-structures/2026/sys_multiuser_multiuser_overview)).

### And the Revit way

Revit's model is the design and documentation record. It has steel fabrication elements (plates, bolt and hole patterns, welds) and over 100 standard parametric connections ([detailed steel modelling](https://help.autodesk.com/cloudhelp/2024/ENU/Revit-StructEng/files/GUID-F4AA1E38-F253-41BF-8FB2-AD98A1C994D1.htm), [steel fabrication elements](https://help.autodesk.com/cloudhelp/2023/ENU/Revit-StructEng/files/GUID-0CB9D92E-F41A-46AB-A7D6-955F4B831425.htm)). But the detail is shallow where fabrication needs it to be deep:

- A weld is a symbol on an edge: "The weld does not contain the logical connection between connected objects" ([place a weld](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Model/files/GUID-8B4DB108-67DF-40AD-9569-A955A7A204CF.htm)).
- For fabrication, the vendor's own path sends the model to a separate detailing program through an extension ([detailed steel modelling](https://help.autodesk.com/cloudhelp/2024/ENU/Revit-StructEng/files/GUID-F4AA1E38-F253-41BF-8FB2-AD98A1C994D1.htm)).
- Precast is an add-in. It splits walls and floors into panels with connections, lifters and reinforcement, makes shop drawings from templates, and writes files for precast plants. If a wall's boundary changes, the panels update without the splitting rules, and the person has to run the split again ([structural precast](https://help.autodesk.com/cloudhelp/2020/ENU/Revit-AddIns/files/GUID-3AA696FA-8CE6-43CC-AB60-D0BC305351EC.htm), [splitting walls and floors](https://help.autodesk.com/cloudhelp/2018/ENU/Revit-AddIns/files/GUID-9A1DE94E-947D-4B97-8647-3BA9E061B4EF.htm), [what's new in 2019](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-WhatsNew/files/GUID-C81929D7-02CB-4BF7-A637-9B98EC9EB38B.htm)).
- Under worksharing, making or deleting a plate or beam also takes the bolts, welds and connections attached to it ([worksharing and steel fabrication elements](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Model/files/GUID-EE49CB4F-C16A-4B85-A3DF-AA7132AB389E.htm)).

So the split people feel is not two styles of interface. It is a design model on one side, a fabrication model on the other, and a handover between them. The research says the same: precast design at fabrication detail needs parametric assemblies driven by rules, with the design intent kept as the model changes ([Sacks, Eastman and Lee 2004](https://doi.org/10.1016/S0926-5805(03)00043-8)), and the steel exchange formats lose information at each hop (CIS/2 to IFC maps one to one, one to many, or not at all, [Lipman 2009](https://itcon.org/paper/2009/1)).

### What IFC can carry

IFC 4.3 has the pieces for a fabrication model: a plate, a mechanical fastener whose types include bolt, anchor bolt and stud shear connector, a fastener whose types include weld, and an element assembly whose types include girder, truss, braced frame, rigid frame and reinforcement unit ([IFC 4.3 source at `5d08761`](https://github.com/buildingSMART/IFC4.3.x-development/tree/5d08761e63557bd3ded140139eede665ec31511c/docs/schemas/shared/IfcSharedComponentElements/Types)). There is no assembly type for a precast unit, so one would be user-defined (*unverified* whether a buildingSMART agreement covers it). IFC is the way a detailing model reaches other firms; it is not a way for two programs to share one model live.

## Options

| | (a) A switch | (b) A separate program | (c) One discipline |
| --- | --- | --- | --- |
| What it is | The structure discipline has two modes. Each has its own tools, defaults and screens | A second Spareware program for detailers, reading and writing the same project | One set of structural elements that carry design detail first and fabrication detail when someone adds it. People choose a tool layout, which changes nothing in the data |
| Data | Must be one data model, or the two modes cannot open each other's work. A mode then only hides or shows tools | Either the same entity store and edit log as Spare Construct (then it is option (c) with a second front door) or its own store with IFC between (then it is the handover people already dislike) | One entity store, one edit log, one set of elements |
| Who it suits | Someone who wants to keep their old habits | A detailing firm that never touches design | Engineers, detailers, precast designers and fabricators, on one model |

### What users gain and lose

| Work | (a) A switch | (b) A separate program | (c) One discipline |
| --- | --- | --- | --- |
| Modelling steel and precast | Gains a familiar start either way. Loses: an element made in design mode has no fabrication detail, and switching modes cannot add it | Gains tools shaped only for detailing. Loses: design members come in from Spare Construct by exchange, or the same store is shared (then see (c)) | Gains one member that goes from a design line to a fabricated part. Loses nothing, as long as fabrication detail is optional and never in the way of design |
| Connections | Two sets: simple symbols in one mode, rule-made parts in the other. A connection made in one mode is a stranger in the other | Rule-made, but only in the detailing program; the engineer sees them only after an exchange | Rule-made once. The engineer can place a connection as a requirement (type and design forces) and the detailer completes it in place |
| Plates, bolts and welds | Real objects in one mode, symbols in the other | Real objects | Real objects. A weld is a relationship between named edges of two parts, so it follows them when they change, unlike a symbol on an edge |
| Marks (numbering) | Only meaningful in the detailing mode | In the detailing program | In the model, for every discipline that needs marks (steel, precast, timber framing, Workshop's parts) |
| Fabrication drawings | One mode only | Its own drawing engine and sheets, or Spare Construct's | The shared drawing engine and sheets every discipline uses (A12, CON-A11), with drawing kinds for a part, an assembly and a precast unit |
| NC (DSTV) and precast plant files | One mode only | Yes | Yes, from the same marks |
| IFC | Two exports, which must agree | Its own export; a round trip with Spare Construct loses what IFC cannot carry | One export; nothing is lost between design and detailing because no exchange happens |
| Sharing the model | One store, but each person's mode decides what they can see and change, which is confusing in a team | Two programs on one project need one change log anyway, or every change travels by exchange and has to be compared | The multi-user design of A05 as it stands. The engineer moves a beam; the connection rebuilds; the detailer sees it on the next sync |

### Effort

Effort is given in board sizes and in the parts of the map each option adds, since nothing is timed yet.

- **(c)** needs the detailing parts once: members as fabricated parts, assemblies and precast units, plates and holes, bolts, welds, connection recipes, marks, fabrication drawing kinds, NC and precast plant writers, and the IFC mapping. That is the twelve rows below: three Large, seven Medium and two Small, plus the map and the existing parts CON-A5-F7 and CON-A21-F3, which it replaces or fills in.
- **(a)** needs all of (c), because the detailing mode needs the same data, plus a second set of tools, defaults and help for the design mode, and the rules for what each mode may change. Every test and every lesson is doubled. Roughly (c) and half again, for a result that is worse in a team.
- **(b)** needs all of (c) and a program around it: install and updates, accounts, project files, sheets, versions, sharing, add-ins and its own work board (CON-A1, A2, A11, A12, A22, A26 and A24 again, more than 200 parts of today's map). If it reuses Spare Construct's crates for those, it is (c) with a second front door and the same cost as (c) plus packaging. There is a kernel cost too: the kernel's rule is that a domain toolkit becomes a `geom-<domain>` crate once two programs need it ([26.1](../../engines/geometry/architecture/26-how-it-grows.md)), so connection and fabrication rules would move into the kernel under its chapter 28 gates and an owner's decision like G238's.

## The kernel pieces each option needs

The geometry is the same for all three options; only where the rules live differs. The kernel's build order ([30-build-order.json](../../engines/geometry/architecture/30-build-order.json)) has no rows named for connections, plates, bolts or welds. That is by design: structural framing, copes, cut-backs and connections are Spare Construct's compositions on `geom-sweep` and `geom-bool` ([chapter 13's element table](../../engines/geometry/architecture/13-sweeps-blends-offsets.md)), and the kernel knows nothing about buildings. What each piece of detailing stands on:

| Detailing work | Kernel rows it waits for |
| --- | --- |
| Members: rolled, welded and cold-formed profiles, straight and curved | G229 profiles from sketches; G255 extrude; G256 general sweeps; curved booleans for rolled sections' root radii (G242 to G252, Gate 7, G284) |
| Copes, cut-backs, notches and end cuts | G130 operands and half-spaces; G131 n-ary cut; G244 face splitting on flat and analytic faces |
| Plates, holes and slots | G229 and G255 for sketched plates; G202 2D booleans for outlines; G138 batch cutting, so every hole in a member is one call; bent plates on the `geom-sheet` toolkit (G472, after the patent search G446) |
| Bolts, anchors, studs | G145 placed instances; G295 the instance tree, so 100,000 bolts are one definition placed many times ([23](../../engines/geometry/architecture/23-performance.md)) |
| Welds | Persistent names and reference resolution (G87 to G89), so a weld keeps its edges through edits or reports why it lost them; bevel preparations by G263 chamfers; bead geometry, if wanted, by G256 sweeps |
| Connections that find their members | G151 exact contact predicates and G292 closed-form distances to find which members meet and how; G89 to keep the connection's references |
| Weights, centres of gravity, lifting points | G247 certified mass properties (exact for flat bodies) |
| Precast units, rebar and cast-in items | G256 swept disks for rebar (chapter 13's table); G131 to cut recesses and openings; G145 for repeated embeds |
| Marks | **A gap.** Placed instances of one definition are identical by construction, but two parts modelled separately carry different persistent names, so their content hashes (G105) differ even when their shapes are equal. Marks need an exact "same shape up to placement" comparison, ignoring names. No build-order row has it. Exact coordinates make it possible without a tolerance and without depending on the order parts were made in, which is a real gain over the tolerance-and-order method described above |
| Fabrication drawings | G387 to G406, Gate 11: G389 plane sections; G390 exact outlines of the round faces in rolled sections; G394 hidden lines for flat bodies, which most plates and many members are; G401 dimensions anchored by reference; G402 the incremental engine, which tells which drawings an edit touched (the "out of date" flag); G405 storage and export; G208 and G209 blocks and dimensions |
| IFC | G413 exchange basics; G421 to G423 import; G426 export; G440 the conformance harness |
| DXF for plate nesting, STEP for parts | G433 DXF write; G429 STEP export |
| DSTV NC and precast plant files | No kernel row, and none is needed: they are text formats written by Spare Construct from kernel queries (outlines, holes, marks). Their terms are checked first (CON-A24-F1-P7), like BTLx for timber (CON-A21-F2-P2) |
| Sharing the model | Nothing in the kernel. It is A05's edit log and A02's entity store |

For (b) only, add an owner decision like G238 on whether a `geom-steel` toolkit enters the kernel, and that toolkit's own chapter 28 gates.

## Recommendation

**Option (c): one structure discipline, with fabrication detail as a level of the same elements, not a mode and not a second program.**

The reason is sharing. Everything people value in the detailing way of working (rule-made connections, real plates, bolts and welds, marks, drawings that know they are out of date, machine files from the model) depends on one model that changes in one place. A switch cannot give that without one data model underneath, at which point the switch is only a tool layout. A second program either shares Spare Construct's store and edit log, at which point it is (c) with a second installer, or it exchanges files, which is the handover that makes people keep design and detailing in separate tools today. Spare Construct's own plan already has the parts (c) builds on: one model for every discipline, the edit log and conflict rules of A05, the incremental engine of A03, the drawing engine of A12 and the shared sheets of CON-A11. The Workshop discipline (CON-A27, pieces A15 to A29, in review) is already adding parts, assemblies, drawing views and parts lists from an assembly's structure to the same app, so precast and steel would reuse that pattern rather than start one.

What (c) keeps from (a): each person picks a tool layout and keyboard scheme (one aimed at design, one at detailing), stored as a preference (CON-A1-F6). It changes which tools are on screen, never what the model holds. What (c) keeps from (b): detailing gets its own pieces, its own drawing kinds and its own tests, so it can be built and released at its own pace inside the program.

Three rules make (c) work:

1. **Detail is added, never required.** A member is valid at design level with no connection; a connection can be a requirement (type, forces, the engineer's note) before it is a set of parts. Nothing in design waits for fabrication detail.
2. **Marks are exact and stable.** A mark is given by exact shape comparison plus the properties a firm chooses, never by tolerance or creation order. Once issued, a mark is kept for as long as the piece stays identical, and a change that would renumber issued pieces is shown before it is applied.
3. **Who may change what is part of sharing.** Connections and marks belong to the detailing firm, members to the engineer, and a change across that line becomes a request through the issue model (CON-A12-F8), as A05's rights already allow (*unverified* until A05's rights design is written for firms).

This changes the map's line "Steel and rebar detailing: Later phase" into a planned discipline level. It does not change the priority: the first release (a council set for a house) needs none of it, and the rows below are `later`, except the map row and the decision row. Changing the map needs this note accepted by an owner.

## First work list

Rows a later lane could open, numbered after the highest existing piece (A29). Each names the map parts it fills.

| ID | Piece | Kind | Size | Done when |
| --- | --- | --- | --- | --- |
| A31 | Map: fabrication detailing in the structure discipline | Map change | Small | features.json and FEATURES.md gain the parts this note names (assemblies and precast units, plates, bolts, welds, marks, drawing kinds, NC and plant output), CON-A5-F7 and CON-A21-F3 are folded in, and the add-ins table's "later phase" line points here |
| A32 | Marks: exact numbering for steel, precast and parts | Research and decision note | Medium | A method for marks by exact shape comparison up to placement plus chosen properties, stable once issued, with the kernel query it needs written as a proposed build-order row for the kernel's owner |
| A33 | Members as fabricated parts | Build | Large | Columns, beams and braces (CON-A5-F1-P2) carry part detail: profile from the section library (CON-A5-F1-P4), cuts and copes, material and finish, weight from the kernel; still valid with none of it |
| A34 | Assemblies and precast units | Build | Medium | Parts are grouped into the pieces that are made and shipped, with a main part, for steel and for precast concrete with its rebar and cast-in items; schedules list them |
| A35 | Plates, holes and bolt groups | Build | Medium | Sketched and bent plates, holes and slots by batch cutting, bolt groups as placed instances with their holes, all following their members through edits |
| A36 | Welds as relationships | Build | Medium | A weld joins named edges of two parts, carries its type, size and preparation, shows its symbol, survives edits or reports why it cannot, and is counted in lists |
| A37 | Connection recipes | Build | Large | Rule-made connections (end plate, fin plate, base plate, splice, brace gusset) that make plates, bolts, welds and cuts, rebuild when members move, and can start as an engineer's requirement; written in the family recipe format (A07) |
| A38 | Fabrication drawings: part, assembly and precast unit | Build | Large | Drawings of each kind made by rules on the shared sheets (CON-A11), one per mark, flagged when an edit touches them; waits for the drawing engine (A12, kernel Gate 11) |
| A39 | NC output: DSTV for steel parts | Research note, then Build | Medium | The format's terms checked (CON-A24-F1-P7); one file per marked part with outline, holes, slots and marking, checked against the published description |
| A40 | Precast plant formats | Research note | Small | Which file formats precast plants in New Zealand and Australia accept, with their terms checked, as CON-A21-F7-P3 does for frame and truss plants |
| A41 | Steel and precast in IFC | Build | Medium | Plates, bolts, welds, assemblies and marks written to and read from IFC 4.3, with a precast unit type agreed or user-defined; round trip tested on the kernel's IFC export (G426) |
| A42 | Detailing tool layout and firm rights | Design note | Medium | The detailing tool layout as a preference, not a mode, and how an engineer's firm and a detailer's firm share one model: who owns members, connections and marks, and how a change across that line becomes a request (A05, CON-A12-F8, CON-A25) |

## Open risks

1. **The marks query is not on the kernel's plan.** Until it is (A32's output), marks fall back to placed instances only, which misses parts modelled twice.
2. **Rolled sections need curved booleans.** Root radii are round faces, so coping a rolled beam waits for Gate 7, unless members start with the round faces left out, which would be wrong on drawings and in NC outlines.
3. **One interface for two audiences.** If detail tools crowd the design tools, engineers will find the discipline harder than Revit. A42's layouts are the answer; it needs testing with real detailers.
4. **Issued marks and drawings carry fabrication risk.** A wrong NC file cuts real steel. The detailing pieces need the same never-a-broken-result rule as the rest, and a check that every output matches its mark before it is written.
5. **Format terms.** DSTV's description is published by the trade body; whether implementing it needs permission is *unverified* (A39). Precast plant formats belong to their vendors (A40).
6. **People's habits.** Some will want the detailing program they know. IFC import and export (A41) and the consultant model compare in Sharing between firms keep them working with us without asking them to move.
7. **The evidence is from help pages.** No detailer or fabricator was asked for this note. A42 should include at least one.

## Sources

| Source | Kind | Used for |
| --- | --- | --- |
| [What numbering is](https://support.tekla.com/doc/tekla-structures/2026/mod_what_is_numbering), [why identical parts get different marks](https://support.tekla.com/article/why-identical-parts-get-different-part-position), [things affecting numbering](https://support.tekla.com/knowledge/list-of-things-affecting-numbering) | Vendor help | How marks are given, and what decides "identical" |
| [Create fabrication drawings](https://help.trimble.com/doc/tekla-structures/2026/dra_create_fabrication_drawings), [precast unit drawings](https://support.tekla.com/doc/tekla-structures/2026/dra_cast_unit_drawings), [update drawings when the model changes](https://support.tekla.com/doc/tekla-structures/2026/dra_updating_drawings_when_the_model_changes) | Vendor help | Drawing kinds, one drawing per mark, out-of-date flags |
| [NC files](https://help.trimble.com/doc/tekla-structures/2026/int_create_nc_files) | Vendor help | One NC file per part, what it holds, its link to marks |
| [Sharing a model](https://support.tekla.com/doc/tekla-structures/2026/ms_what_is_model_sharing), [multi-user mode](https://support.tekla.com/doc/tekla-structures/2026/sys_multiuser_multiuser_overview) | Vendor help | The two ways several people work on one model |
| [Detailed steel modelling](https://help.autodesk.com/cloudhelp/2024/ENU/Revit-StructEng/files/GUID-F4AA1E38-F253-41BF-8FB2-AD98A1C994D1.htm), [steel fabrication elements](https://help.autodesk.com/cloudhelp/2023/ENU/Revit-StructEng/files/GUID-0CB9D92E-F41A-46AB-A7D6-955F4B831425.htm), [place a weld](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Model/files/GUID-8B4DB108-67DF-40AD-9569-A955A7A204CF.htm), [worksharing and steel fabrication elements](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-Model/files/GUID-EE49CB4F-C16A-4B85-A3DF-AA7132AB389E.htm) | Vendor help | Revit's steel detail, its weld, and the handover to a detailing program |
| [Structural precast](https://help.autodesk.com/cloudhelp/2020/ENU/Revit-AddIns/files/GUID-3AA696FA-8CE6-43CC-AB60-D0BC305351EC.htm), [splitting walls and floors](https://help.autodesk.com/cloudhelp/2018/ENU/Revit-AddIns/files/GUID-9A1DE94E-947D-4B97-8647-3BA9E061B4EF.htm), [what's new in 2019](https://help.autodesk.com/cloudhelp/2019/ENU/Revit-WhatsNew/files/GUID-C81929D7-02CB-4BF7-A637-9B98EC9EB38B.htm) | Vendor help | Revit's precast add-in, its shop drawings and plant output |
| [bauforumstahl: Standard description of steel parts 8.0](https://bauforumstahl.de/wp-content/uploads/2024/04/109_Standartbeschreibung_von_Stahlbau-Teilen_8.0.pdf) | Trade body's published format | The DSTV NC format |
| [buildingSMART IFC 4.3 source at `5d08761`](https://github.com/buildingSMART/IFC4.3.x-development/tree/5d08761e63557bd3ded140139eede665ec31511c) | Standard | Fastener, mechanical fastener and element assembly types |
| [Sacks, Eastman and Lee 2004, Automation in Construction 13(3), 291-312](https://doi.org/10.1016/S0926-5805(03)00043-8) | Paper | Parametric precast modelling at fabrication detail |
| [Lipman 2009, ITcon 14, 1-13](https://itcon.org/paper/2009/1) | Paper | What is lost between steel exchange formats |
| Kernel map: [13](../../engines/geometry/architecture/13-sweeps-blends-offsets.md), [18](../../engines/geometry/architecture/18-drawings.md), [23](../../engines/geometry/architecture/23-performance.md), [24](../../engines/geometry/architecture/24-file-format-exchange.md), [26](../../engines/geometry/architecture/26-how-it-grows.md), [30-build-order.json](../../engines/geometry/architecture/30-build-order.json), [32.17](../../engines/geometry/architecture/32-decisions.md) | Ours | Which kernel rows each piece of detailing waits for, and the marks gap |
| [ARCHITECTURE.md](../ARCHITECTURE.md), [FEATURES.md](../FEATURES.md), [A05](A05-multi-user.md) | Ours | Where detailing sits in today's map, and how sharing works |
