# The Spare Construct cloud format, version 1

This is a published format under [EXTENDING.md](../../../../EXTENDING.md), section 3. It sits on top of the kit's [store format](../../../kit/crates/kit-store/FORMAT.md), which holds the keys, rows, log entries and versions every program shares; this page adds what Spare Construct keeps. **We may stop writing an old version. We never stop reading it.** [`src/keys.rs`](src/keys.rs), [`src/rows.rs`](src/rows.rs) and [`src/element.rs`](src/element.rs) are the reader library.

## Inputs and what is derived

An element's record holds its inputs, and the edit log is the truth about those. A shape is never the truth: it is built from the record's recipe, and every row marked *derived* below can be deleted and made again from the log, byte for byte. `spare-server regenerate` does exactly that, and `spare-server verify` rebuilds every shape and compares it.

So derived rows aren't in any log, a log entry's `changes` name input records only, and a package or a replay needs nothing else to reach the same model.

## An element

The kit's `p:<project>/record/<hash>` holds an element, named by the SHA-256 of its row, and `p:<project>/piece/<document>/<element>` points at the current one. A document is a model in a team's WIP folder.

| Field | Meaning |
| --- | --- |
| `id`, `document` | Its permanent ID, and the document it belongs to |
| `kind`, `name`, `props` | What it is |
| `level` | An integer, read for ever as the level it names (below, "Levels"); every record written from this version writes `0`, and an element's level is its tie in `props` |
| `placement` | `{x,y,z}` in whole millimetres, each exactly 6,400 of the geometry kernel's grid steps, the unit every other Construct length is stored in ([note CON-A2-F3-P1](../../notes/CON-A2-F3-P1-stored-units.md)). Outside the recipe, so a move rebuilds nothing and every door of one type shares one body |
| `recipe` | How to build it |
| `references` | What it is hosted on or built against |

A recipe is the kernel's envelope (architecture chapter 24):

| Field | Meaning |
| --- | --- |
| `owner`, `schema` | Who owns the operation, and their schema version |
| `op`, `op_version` | The operation, named for what it makes: `wall.straight`, `door.single`, `block` |
| `args` | Its arguments, sorted by name. Numbers are whole; the stand-in recipes (`op_version` 1) give lengths in whole millimetres, exact multiples of the grid step, and a recipe that needs a finer length gives grid steps under a new `op_version`. An argument may instead name a parameter, `{"param":"<id>"}`, or read the height from the element's base to its top tie, `{"tie":"top"}` (below) |

A reference (chapter 20) points at part of a shape by persistent name, never by position in a list. A hosted door points at a face of a wall, not at the wall:

| Field | Meaning |
| --- | --- |
| `doc` | `null` for this document, otherwise `{id, version}`: the linked document, and the version the reference was made against |
| `element` | The element holding the part |
| `name`, `kind` | Its persistent name, and what it is, such as `face` |
| `want` | `one`, `allpieces` or `carrier`, for when what it named has been split |
| `signature` | Kept only to suggest a repair. Resolving never guesses from it |

### Hosted elements

A hosted element (a door) sits on a face of its host (a wall), at an offset along that face and up it, and moves with its host: its record is not rewritten when the host moves or lengthens. Only a `wall.straight` hosts, on its two sides. The reader is [construct-regen's `hosting`](../construct-regen/src/hosting.rs).

| Where | What |
| --- | --- |
| `references` | One reference of kind `face`, `doc` `null`: `element` the host, `name` the persistent name of the face, one of the host's sides (`exterior` or `interior`) as the document's builder names them, `want` `carrier`, `signature` that face's role |
| `props` `@host.along` | How far along the face from the host's start (`end0`), a stored length (`length:q0:<steps>`) |
| `props` `@host.up` | How far up the face from the host's base, a stored length |
| `level` | The host's, when it was placed |
| `placement` | Where it sat when a command last wrote it. Where it sits is read from its host: the host's placement, plus, in the host's frame, the offset along its axis, the offset up, and across it flush with the face's side (along x the exterior is at y = 0, along y at x = the host's thickness), so a door as thick as its wall fills it. Offsets are whole millimetres, as every placement is |

A record with one offset and not the other, an offset that is not a stored length, or offsets without exactly one such reference does not read as hosted: it is built as failed (`BadHost`). One whose host is gone is built as failed (`HostMissing`).

**An older record.** A `door.single` written before offsets were stored, with a reference of kind `face` (`doc` `null`) and no `@host.*` keys, is hosted by the element that reference names, and sits where its own placement puts it: its place in its host is the difference of the two placements. Its host is built with it, but it stays where it was placed when its host moves. `hosted.move` writes its offsets, and from then on it moves with its host ([note CON-A4-F3-P1](../../notes/CON-A4-F3-P1-hosting.md)).

### Categories, types and parameter values

Every record a command writes carries its category, and an element of a type names its type, in `props` under keys that start with `@`, which a permanent ID never does. The keys and the type record are [construct-types' format](../construct-types/FORMAT.md); the values are [construct-params' stored text](../construct-params/FORMAT.md).

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every record written from this version | The category's ID ([construct-categories](../construct-categories/FORMAT.md)) in decimal. An element's is its kind's: the recipe's `op`, with its `owner` in front when that is not `construct` (`workshop.part`). A type's is its own |
| `@type` | An element of a type | The type's permanent ID. The type is of the element's category |
| a parameter's ID | An element, or a type | Its value of that parameter, in its stored text (`length:q0:5248000`). An element holds only parameters each instance holds; a type only those the type holds |

Any other key is plain text, kept and read as it is, so a record written before this version still reads. An element reads a parameter from its own value, then its type's, then the binding's default ([construct-types' `resolve`](../construct-types/src/resolve.rs)). An element with no type has the instance parameters its category is bound to.

A **type** is a record of kind `type`, written by construct-types; it lists every parameter bound to its category. A document's **parameters** are one record, element ID `parameters`, kind `parameters`, at level 0 and placement `{0,0,0}` with no references, recipe `{"owner":"construct","schema":1,"op":"parameters","op_version":1,"args":{}}`, and one `props` entry, `@parameters`: the project parameter table's stored bytes ([construct-params, "Project parameters"](../construct-params/FORMAT.md)). Neither has a shape: neither is built, and both are listed by kind. The [reader library](../construct-regen/src/parameters.rs) reads both strictly.

A recipe argument `{"param":"<id>"}` is filled, before the shape is built, with the value the element reads for that parameter: a length as whole millimetres, a whole number as it is. Nothing holding a value is the build failure `UnsetParameter`; another kind of value, or an ID that is no parameter's, `BadParameter`. The type and the parameters are read as they stood at that point of the log, so a shape is the same however the log was reached.

### Levels

A level is a record of kind `level` in the levels category, with its elevation and whether it is a storey; elements are tied to a level by its ID with an offset, and a wall's top may be tied to another level. The level record, the tie keys (`@level`, `@level.offset`, `@top.level`, `@top.offset`) and the rule that reads the old integer `level` are [construct-levels' format](../construct-levels/FORMAT.md). In short: a record with a tie is on its tie's level; with none, `0` is on no level and any other integer `n` on the implied level `n` (ID `1`, `m1` below zero; `Level n`; `(n - 1) x 3000 mm`, or `n x 3000 mm` below zero), which has no record until a command changes it.

An element's base sits at its level's elevation plus its offset, worked out by construct-levels' `height_of`. A recipe argument `{"tie":"top"}` is filled, before the shape is built, with the height from the base to the top tie, as whole millimetres for the stand-in recipes; a height that is not, or a tie to a level the model does not have, is the build failure `BadLevel` or `MissingLevel`. Levels are read as they stood at that point of the log, so a shape is the same however the log was reached, and moving a level writes no element: the regenerator builds what is tied to it again ([reader](../construct-regen/src/levels.rs)).

### Workshop's elements

Workshop, the discipline for things that are made (feature map area CON-A27), keeps its parts and its assembly as elements like any other, changed by the same commands. They sit at level 0 and placement `{0,0,0}`: a part is a definition, like a type, and has no place in the plan of its own.

| Kind | Recipe `op` | `args` |
| --- | --- | --- |
| `workshop.part` | `part` | `params` (each `{name, value, note, kind}`, where `kind` is `length`, the default, `angle` or `count`), `features` (the history, in order, each `{id, type, name, ...}`), `end` (how many features are built; the rest are rolled back), `material` |
| `workshop.assembly` | `assembly` | `instances` (each `{id, part, name, at, grounded}`, where `part` is a `workshop.part` element's ID) and `joints` (each `{id, name, type, a, b, origin, axis, min, max, value, unit}`) |

The recipe's `owner` is `workshop`, schema 1, op version 1. A value is a whole number, or text: a number with a fraction written as its decimal text (`"8.5"`), so no hash depends on how a float prints, a parameter's name, or a formula of parameters with units, such as `"width / 2 - 4 mm"`, in the formula language of [`construct-formulas`](../construct-formulas/src/lib.rs). A formula is stored as written. The element's `name` is the part's name. The stand-in edition doesn't build these, and says so (`UnknownOperation`): the Workshop page builds a part in the browser with the geometry kernel (`construct-workshop`), and the server will too once a document's edition is the kernel's.

## A document

`p:<project>/model/<model>` holds a document's header, including `edition`: `{engine, version, name_scheme}`, what builds its shapes, and, when a geometry engine of the kernel builds them, `report`: the engine's own account of itself, `{engine, version, steps_per_mm, tolerances, answers}` (its name, its version or pinned source, its grid step in steps per millimetre, each tolerance as `{name, value, unit}` with the value as decimal text, and which part answers each operation as `{operation, case, by}`). An edition without an engine, such as the stand-in's, has no `report` key, so editions written before it existed read and compare as they did. Two editions are the same only when every field, the report's included, is the same. A document runs one edition. Moving to another is an ordered operation that reports what changed; a server or browser without that edition doesn't build it, because it would give different bytes under the same recipe. A browser in that position is a viewer of the stored shapes.

## A project's tables

| Key | Value |
| --- | --- |
| `p:<project>/model/<model>` | A document in a team's WIP folder |
| `p:<project>/team/<team>` | A team |
| `p:<project>/item/<team>/<folder>/<item>` | A package in the `shared` or `consumed` folder |
| `p:<project>/version/<model>` | The last package number of a model |
| `p:<project>/incoming/<id>` | A package bridged in and waiting |
| `p:<project>/link/<model>/<link>` | A link from a model |

### Derived

| Key | Value |
| --- | --- |
| `p:<project>/built/<document>/<element>` | What regeneration made of an element: the input record, the edition, `level` (the ID of the level its cells are under, or `null` on no level; a row written when levels were integers holds the integer, read as its decimal text), `elevation` (how high its base sits, in stored length text; left out on no level), the cells, and `{"shape":{body,bounds,names,displays}}` or `{"failed":{code,message}}`. A failure is model state, the same on every machine. A host is built with the elements hosted on it, in the order of their keys; on the kernel's edition each door cuts its opening, and one that does not fit is the wall's failure in the builder's words, naming the door (a wall shortened past its door is reported so); the stand-in builds the host alone |
| `p:<project>/body/<id>` | A body, named by the SHA-256 of its canonical bytes |
| `p:<project>/display/<id>` | One level of detail of a body, named the same way |
| `p:<project>/area/<document>/<level>/<x>/<y>/<element>` | The element's record hash, in each 5 m cell its shape touches. `<level>` is the ID of the level its base is tied to; the old integers' rows are the IDs of their implied levels unchanged (`1`). An element on no level is in no cell |
| `p:<project>/reads/<document>/<element>` | What an element's shape or place reads besides its own record. An element that neither is hosted nor hosts: the IDs of the records with no shape it reads, ascending (`parameters`, the document's parameters, and its type, when its recipe names a parameter; the levels its base and a recipe's top tie stand on). Any other: `{"reads":[...],"host":...,"hosted":[...]}`, each key left out when empty: those IDs, the element it is hosted on (a change to that builds it again, since where it sits is read from it), and the elements hosted on it in key order (it is built with them, so a change to one builds it again). A change to any record it lists, or its going, builds it again. Older rows read: `null` as `["parameters"]`, a type's ID as `["parameters", that ID]`, and an object with `"parameters":true` and `"type"` as those two in `reads`. A store regenerated before levels were elements lists no levels until `spare-server regenerate` runs once |
| `p:<project>/kind/<document>/<kind>/<element>` | The element's record hash, under its kind, whether or not it built, so a page that works on one kind (Workshop's `workshop.part` and `workshop.assembly`) finds its elements without a place in the plan. A kind that isn't a plain name (letters, digits, `.`, `_`, `-`) isn't listed |
| `p:<project>/regenerated` | How far regeneration has got through the log |

A body or display row says where its bytes are: `{"here":{"hex":...}}`, or above 256 KiB `{"file":{"path":...,"size":...}}` in file storage, linked by hash. Reading one checks the bytes against the name. A body is opaque above the geometry interface: nothing outside the engine that wrote it reads inside. A display record is JSON in the published shape of [`Display`](src/geometry.rs): the level, how far it strays from the body, a mesh and an outline.

## The directory's tables

| Key | Value |
| --- | --- |
| `dir/hub/<hub>` | A hub |
| `dir/hub/<hub>/project/<project>` | A hub's project |
| `dir/project/<project>` | A project |
| `dir/home/<person>` | A person's home hub: the first they joined |

## Access

These rows are what Cedar reads. A row's presence is the fact. The rules are in [`construct-api`](../construct-api/policy/access.cedar).

| Key | Value |
| --- | --- |
| `acl/person/<person>/hub/<hub>` | Their role: `user` or `external` |
| `acl/person/<person>/project/<project>` | They are on this project |
| `acl/person/<person>/team/<project>/<team>` | They are on this team |
| `acl/hub/<hub>/user/<person>` | The hub's side of the first row |
| `acl/hub/<hub>/bridge/<other>` | `hub` permits bridges with `other`. A bridge needs this row from both sides |

## Commands

| In the log of | Commands |
| --- | --- |
| A project | `element.create`, `element.move`, `element.set`, `element.delete`, `element.claim`, `element.release`, `element.param`, `element.type`, `parameter.define`, `parameter.bind`, `type.create`, `type.rename`, `type.duplicate`, `type.delete`, `type.set`, `hosted.place`, `hosted.move`, `level.create`, `level.rename`, `level.elevation`, `level.storey`, `level.delete`, `level.tie`, `level.top`, `edit.undo`, `edit.redo`, `model.add`, `package.publish`, `package.bridge`, `package.take`, `link.add`, `document.checkpoint`, and `package.arrived`, written by a bridge in the receiving project |
| The directory | `hub.add`, `hub.user`, `hub.external`, `hub.permit`, `project.add`, `project.person`, `team.add`, `team.person`, beside the kit's people commands |

The element, parameter, type, hosting and level commands, and undo and redo, are allowed to whoever may edit the model, and each takes the same claim on the piece it changes as `element.set`: another person's claim refuses it. A refusal is the command's result, in the words of the data crate that refused it. Every command fully applies or fully rolls back: what a command that is turned back had written is not stored, though its entry is in the log as turned back.

| Command | Arguments | Writes |
| --- | --- | --- |
| `element.create` | as before, `type` (optional), and `level`: a level's ID, or an old page's integer, which names its implied level; none, `null` or `0` is no level | `@category` from the kind (a kind with no category, and a level, are refused) and `@type`; the tie `@level` and `@level.offset` (`0`), and `level` 0. The level must be in the model. `props` may not hold an `@` key or a parameter's value |
| `element.set` | as before | `@category` again from the recipe; a recipe of another category than the element's type is refused. `props` as for `element.create` |
| `element.param` | `element`, `parameter`, `value` (typed text such as `2.4 m`) | That parameter's entry, replaced; other keys kept |
| `element.type` | `element`, `type` | `@type`; its own values the new type lacks are dropped. Another category is refused |
| `parameter.define` | `definition` (a construct-params definition row) | The parameters record |
| `parameter.bind` | `parameter`, `categories` (top-level category IDs), `default` (typed text, or `null`) | The parameters record, and each type of those categories, which then lists the parameter |
| `type.create` | `type`, `name`, `category` | A type record |
| `type.rename`, `type.duplicate`, `type.set` | `type`, and `name`; `new` and `name`; `parameter` and `value` | The type record, or its copy |
| `type.delete` | `type` | Its pointer goes. Refused while an element is of it |
| `hosted.place` | `element`, `kind` (default `door`), `name`, `host`, `face` (the persistent name of one of the host's sides), `along`, `up` (typed lengths such as `1200 mm`; `up` is 0 when left out), `recipe` | A new element hosted on that face at those offsets ("Hosted elements", above), on the host's level, with `@category` from its kind |
| `hosted.move` | `element`, `along`, `up` (kept when left out) | Its offsets, and its placement to match: it moves along its host. An older record, hosted by its position, gets its offsets |
| `level.create` | `level`, `name`, `elevation` (typed text), `storey` (default `true`) | A level record |
| `level.rename`, `level.elevation`, `level.storey` | `level`, and `name`; `elevation`; `storey` | The level's record; for an implied level, its first |
| `level.delete` | `level` | Its pointer goes. Refused while anything is tied to it: `"<name> can't be deleted: <n> elements are tied to it."` |
| `level.tie` | `element`, `level`, `offset` (typed text, default `0`) | The element's `@level` and `@level.offset`, and `level` 0 |
| `level.top` | `element` (a wall), `level` (or `null`), `offset` (typed text, default `0`) | The wall's `@top.level` and `@top.offset`, and its recipe's `height` as `{"tie":"top"}`; a top at or below its base is refused. `null` takes the top tie away and writes the height it read as a number |
| `edit.undo` | none (`{}`) | Every piece the person's last step changed, put back as it was before it (below) |
| `edit.redo` | none (`{}`) | Every piece the person's last undo changed, put back: the undo of that undo |

`element.*` commands refuse a type or the parameters record: each has its own commands. Typed text is read with construct-params' metric display settings until a project keeps its own.

The hosting commands refuse, as their result: a host that is not in the model (`"There is no element <host> in this model to host it."`), one that is not a wall (`"<name> is not a wall: only a wall hosts a door."`), a face that is not one of its sides, an offset that is not a length or is not whole millimetres, and an element that would not fit with what is already hosted there, in the kernel builder's own words (`"door D-1: it crosses the wall's end"`), whichever builder the document is on. `element.delete` of an element that still hosts others is refused, naming how many (`"<name> still hosts 2 elements: delete them, or host them elsewhere, first."`); an element hosted on a face is moved by `hosted.move`, not `element.move`, and its host is not changed through `element.set`'s `references`.

### Undo and redo

An undo is an entry like any other, after everything before it; nothing is ever taken out of the log. Which step it reverses is not in its arguments: it is read from the log, the same way every time, so a server that starts again or a replay of the log on an empty store gives the same answer ([`construct_api::cloud::undo`](../construct-api/src/cloud/undo.rs)).

- **A step** is one applied entry, in that document, by that person, of `element.create`, `element.move`, `element.set`, `element.delete`, `element.param`, `element.type`, the seven parameter and type commands, `hosted.place`, `hosted.move`, the seven level commands, `edit.undo` or `edit.redo`, with every change it records: a `parameter.bind` that wrote the parameters record and every type of its categories is one step. A run of `element.move` entries of one element by one person, with no other entry of theirs in that document between, is one step (one drag; the drag's `element.claim` starts the next run). Nothing else is a step, and nothing else is undone.
- **The order is each person's own.** Ordinary steps and `edit.redo` go on the person's undo order, `edit.undo` takes the newest off; `edit.undo` goes on their redo order, `edit.redo` takes the newest off, and an ordinary step empties it. Other people's entries change neither. A turned-back entry is no step.
- **What it writes.** For each piece the step changed, from the record before its first change of that piece to the record after its last: a column the step changed goes back to what it was, if it still holds what the step left; every other column keeps what it holds now. The columns are `kind`, `name`, `level`, `placement`, `recipe`, `references` and each `props` key, compared exactly. A piece the step made is deleted, and one it deleted comes back. The undo's own `changes` are these, before and after by content ID, as for any command. A redo is the undo of the undo, so it writes the same way.
- **What turns it back**, whole, with nothing written: another person's live claim on a piece it would write (`"<name> is working on this."`); a column the step changed that someone has changed since, a piece someone deleted since, or a piece the step made that someone has changed since (`"<name> has changed <record's name> since."`); a record it writes that the document's rules can't read, such as an element whose type is gone (`"That can't be undone: "` and the reader's words; when it writes a type or the parameters record, every record of the document is read again), or that would leave a hosted element without its host (`"That can't be undone: <name> would be left without its host, <host>."`); an element it would leave tied to a level no longer in the model (`"That can't be undone: <name> would be tied to level <id>, which is no longer in this model."`; when it writes or takes away a level, every element of the document is checked); nothing to undo or redo (`"There is nothing of yours to undo in this model."`, and the same for redo).

`document.checkpoint` is written by the server after regenerating. Its arguments are `{"through":n,"bodies":{element:body}}`, and it changes nothing: it is in the log so that every browser watching gets it in order with the edits it covers, and compares the bodies it built itself (note A05).

## Package files

A package is one JSON file in file storage, named by its hash: `{project, document, edition, seq, elements}`, each element as `{element, built, display}`, with the coarsest display inline. Finished shapes travel with it because a host draws a linked document and never regenerates it.
