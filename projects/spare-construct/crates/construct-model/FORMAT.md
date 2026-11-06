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
| `placement` | `{x,y,z}` in whole millimetres. Outside the recipe, so a move rebuilds nothing and every door of one type shares one body |
| `recipe` | How to build it |
| `references` | What it is hosted on or built against |

A recipe is the kernel's envelope (architecture chapter 24):

| Field | Meaning |
| --- | --- |
| `owner`, `schema` | Who owns the operation, and their schema version |
| `op`, `op_version` | The operation, named for what it makes: `wall.straight`, `door.single`, `block` |
| `args` | Its arguments, sorted by name. Numbers are whole |

A reference (chapter 20) points at part of a shape by persistent name, never by position in a list. A hosted door points at a face of a wall, not at the wall:

| Field | Meaning |
| --- | --- |
| `doc` | `null` for this document, otherwise `{id, version}`: the linked document, and the version the reference was made against |
| `element` | The element holding the part |
| `name`, `kind` | Its persistent name, and what it is, such as `face` |
| `want` | `one`, `allpieces` or `carrier`, for when what it named has been split |
| `signature` | Kept only to suggest a repair. Resolving never guesses from it |

## A document

`p:<project>/model/<model>` holds a document's header, including `edition`: `{engine, version, name_scheme}`, what builds its shapes. A document runs one edition. Moving to another is an ordered operation that reports what changed; a server or browser without that edition doesn't build it, because it would give different bytes under the same recipe. A browser in that position is a viewer of the stored shapes.

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
| A project | `element.create`, `element.move`, `element.set`, `element.delete`, `element.claim`, `element.release`, `model.add`, `package.publish`, `package.bridge`, `package.take`, `link.add`, `document.checkpoint`, and `package.arrived`, written by a bridge in the receiving project |
| The directory | `hub.add`, `hub.user`, `hub.external`, `hub.permit`, `project.add`, `project.person`, `team.add`, `team.person`, beside the kit's people commands |

`document.checkpoint` is written by the server after regenerating. Its arguments are `{"through":n,"bodies":{element:body}}`, and it changes nothing: it is in the log so that every browser watching gets it in order with the edits it covers, and compares the bodies it built itself (note A05).

## Package files

A package is one JSON file in file storage, named by its hash: `{project, document, edition, seq, elements}`, each element as `{element, built, display}`, with the coarsest display inline. Finished shapes travel with it because a host draws a linked document and never regenerates it.
