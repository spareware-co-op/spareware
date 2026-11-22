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
| `kind`, `name`, `level`, `props` | What it is |
| `placement` | `{x,y,z}` in whole millimetres, each exactly 6,400 of the geometry kernel's grid steps, the unit every other Construct length is stored in ([note CON-A2-F3-P1](../../notes/CON-A2-F3-P1-stored-units.md)). Outside the recipe, so a move rebuilds nothing and every door of one type shares one body |
| `recipe` | How to build it |
| `references` | What it is hosted on or built against |

A recipe is the kernel's envelope (architecture chapter 24):

| Field | Meaning |
| --- | --- |
| `owner`, `schema` | Who owns the operation, and their schema version |
| `op`, `op_version` | The operation, named for what it makes: `wall.straight`, `door.single`, `block` |
| `args` | Its arguments, sorted by name. Numbers are whole; the stand-in recipes (`op_version` 1) give lengths in whole millimetres, exact multiples of the grid step, and a recipe that needs a finer length gives grid steps under a new `op_version`. An argument may instead name a parameter, `{"param":"<id>"}` (below) |

A reference (chapter 20) points at part of a shape by persistent name, never by position in a list. A hosted door points at a face of a wall, not at the wall:

| Field | Meaning |
| --- | --- |
| `doc` | `null` for this document, otherwise `{id, version}`: the linked document, and the version the reference was made against |
| `element` | The element holding the part |
| `name`, `kind` | Its persistent name, and what it is, such as `face` |
| `want` | `one`, `allpieces` or `carrier`, for when what it named has been split |
| `signature` | Kept only to suggest a repair. Resolving never guesses from it |

### Categories, types and parameter values

Every record a command writes carries its category, and an element of a type names its type, in `props` under keys that start with `@`, which a permanent ID never does. The keys and the type record are [construct-types' format](../construct-types/FORMAT.md); the values are [construct-params' stored text](../construct-params/FORMAT.md).

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every record written from this version | The category's ID ([construct-categories](../construct-categories/FORMAT.md)) in decimal. An element's is its kind's: the recipe's `op`, with its `owner` in front when that is not `construct` (`workshop.part`). A type's is its own |
| `@type` | An element of a type | The type's permanent ID. The type is of the element's category |
| a parameter's ID | An element, or a type | Its value of that parameter, in its stored text (`length:820`). An element holds only parameters each instance holds; a type only those the type holds |

Any other key is plain text, kept and read as it is, so a record written before this version still reads. An element reads a parameter from its own value, then its type's, then the binding's default ([construct-types' `resolve`](../construct-types/src/resolve.rs)). An element with no type has the instance parameters its category is bound to.

A **type** is a record of kind `type`, written by construct-types; it lists every parameter bound to its category. A document's **parameters** are one record, element ID `parameters`, kind `parameters`, at level 0 and placement `{0,0,0}` with no references, recipe `{"owner":"construct","schema":1,"op":"parameters","op_version":1,"args":{}}`, and one `props` entry, `@parameters`: the project parameter table's stored bytes ([construct-params, "Project parameters"](../construct-params/FORMAT.md)). Neither has a shape: neither is built, and both are listed by kind. The [reader library](../construct-regen/src/parameters.rs) reads both strictly.

A recipe argument `{"param":"<id>"}` is filled, before the shape is built, with the value the element reads for that parameter: a length as whole millimetres, a whole number as it is. Nothing holding a value is the build failure `UnsetParameter`; another kind of value, or an ID that is no parameter's, `BadParameter`. The type and the parameters are read as they stood at that point of the log, so a shape is the same however the log was reached.

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
| `p:<project>/built/<document>/<element>` | What regeneration made of an element: the input record, the edition, the cells, and `{"shape":{body,bounds,names,displays}}` or `{"failed":{code,message}}`. A failure is model state, the same on every machine |
| `p:<project>/body/<id>` | A body, named by the SHA-256 of its canonical bytes |
| `p:<project>/display/<id>` | One level of detail of a body, named the same way |
| `p:<project>/area/<document>/<level>/<x>/<y>/<element>` | The element's record hash, in each 5 m cell its shape touches |
| `p:<project>/reads/<document>/<element>` | An element whose recipe names a parameter: the ID of its type, or `null`. A change to that type, or to the document's parameters, builds it again |
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
| A project | `element.create`, `element.move`, `element.set`, `element.delete`, `element.claim`, `element.release`, `element.param`, `element.type`, `parameter.define`, `parameter.bind`, `type.create`, `type.rename`, `type.duplicate`, `type.delete`, `type.set`, `model.add`, `package.publish`, `package.bridge`, `package.take`, `link.add`, `document.checkpoint`, and `package.arrived`, written by a bridge in the receiving project |
| The directory | `hub.add`, `hub.user`, `hub.external`, `hub.permit`, `project.add`, `project.person`, `team.add`, `team.person`, beside the kit's people commands |

The element, parameter and type commands are allowed to whoever may edit the model, and each takes the same claim on the piece it changes as `element.set`: another person's claim refuses it. A refusal is the command's result, in the words of the data crate that refused it.

| Command | Arguments | Writes |
| --- | --- | --- |
| `element.create` | as before, and `type` (optional) | `@category` from the kind (a kind with no category is refused) and `@type`. `props` may not hold an `@` key or a parameter's value |
| `element.set` | as before | `@category` again from the recipe; a recipe of another category than the element's type is refused. `props` as for `element.create` |
| `element.param` | `element`, `parameter`, `value` (typed text such as `2.4 m`) | That parameter's entry, replaced; other keys kept |
| `element.type` | `element`, `type` | `@type`; its own values the new type lacks are dropped. Another category is refused |
| `parameter.define` | `definition` (a construct-params definition row) | The parameters record |
| `parameter.bind` | `parameter`, `categories` (top-level category IDs), `default` (typed text, or `null`) | The parameters record, and each type of those categories, which then lists the parameter |
| `type.create` | `type`, `name`, `category` | A type record |
| `type.rename`, `type.duplicate`, `type.set` | `type`, and `name`; `new` and `name`; `parameter` and `value` | The type record, or its copy |
| `type.delete` | `type` | Its pointer goes. Refused while an element is of it |

`element.*` commands refuse a type or the parameters record: each has its own commands. Typed text is read with construct-params' metric display settings until a project keeps its own.

`document.checkpoint` is written by the server after regenerating. Its arguments are `{"through":n,"bodies":{element:body}}`, and it changes nothing: it is in the log so that every browser watching gets it in order with the edits it covers, and compares the bodies it built itself (note A05).

## Package files

A package is one JSON file in file storage, named by its hash: `{project, document, edition, seq, elements}`, each element as `{element, built, display}`, with the coarsest display inline. Finished shapes travel with it because a host draws a linked document and never regenerates it.
