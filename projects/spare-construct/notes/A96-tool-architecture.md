# A96: A tool is one module on each side, and the core never names it

**Status, 2026-11-28:** Decided for the build (piece A96). The page half is built by A96 (below); the Rust half is the next job (job 11.0b) and is written here so that it can start at once. Part: CON-A8 (the page, all of it), and the command layer under it.

## The question

The owner, 28 Nov 2026: the page, and the whole tool architecture under it, is to be "designed in a smart, expandable way", expandable for ever and easy to maintain.

It is not. Every tool is written into the middle of shared lists:

- **The page.** `app/connect.js` is one file (5,564 lines on main, 7,303 with train 11) holding every tool. The core's functions name each tool: `drawPlan` has an `if` per kind, `propertiesPalette` a branch per tool, `hint` a line per tool, the plan's click handler a branch per placing tool, `connectRibbon` a call per tool, and Delete and the tool switch are chains of wrappers each tool adds around the last. Every train fights conflicts in this one file, and adding a tool means editing the middle of it.
- **The command layer.** A tool's commands live in its own module (`construct-api/src/cloud/<tool>.rs`), but it is also written into: `cloud/commands.rs` three times (`permit`, `permit_group` and `apply`, the last with nested chains of `generic_rules`), `cloud/undo.rs` twice (`is_step` and the `undo_rules` chain), `cloud/mod.rs`, construct-categories' `kinds.rs`, construct-warnings' `RULES`, the schedule fields, the server's `service.rs` reads and `http.rs` routes, construct-model's `FORMAT.md` command list, and `server/tests/service.rs`. Train 11 met conflicts in most of these in a single merge.

The rest of Spareware already has the shape the owner wants: the shared ribbon (`design/ribbon/`), one ribbon per discipline, and the Transport page's registry with country packs (`demo/transport/js/registry.js`, `packs/nz/`, `packs/README.md`, note [CON-A22-F5-P1](CON-A22-F5-P1-transport-country-packs.md)). This note follows that shape and does not invent a second one.

## The rule

**A tool is three things, each in its own files: its Rust part, its page module, and its section of the two-browser test. The core never names a tool. Adding a tool adds files and one line in one list per side, the line a module system needs anyway, and touches no logic outside the tool.**

The one list per side is the same idea as Transport's pack loading (its `index.html` lists a pack's files in order): a list of names, in a fixed order, holding no logic. Its order is the order the core asks the tools in, so it is what makes the page and the server deterministic.

## The page (built by A96)

### Files

```
app/
  connect.js                    the entry: loads the core, then the tools' list, then starts
  core/                         the core (below); never imports from tools/
    registry.js                 the registry: disciplines, tools, their hooks, packs
    late.js                     the names the core still calls a tool by (below), bound once the tool has loaded
  tools/
    index.js                    the ordered list of tool modules, one line each
    project/<tool>.js           tools of the whole project: levels, warnings, views, graphics, schedules, crop, families
    architecture/<tool>.js      wall, floor, roof, room, grid, dimension, curtain, door, family, opening, stair
    workshop/elements.js        Workshop's parts and assemblies
  packs/README.md               how a country or content pack plugs in
```

Plain ES modules: no framework and no build step. `body.html` loads `connect.js` as a module (`type="module"`); its name stays, so everything that knew the page by it (Workshop's page finds the app by that script) still does. The server serves `app/core` and `app/tools` as two trees embedded when it is built (the `include_dir` crate, 0.7.4, MIT), at `/core/...` and `/tools/...`, so a new file needs no route; `server/build.rs` rebuilds the server when a file is added to either.

### The core

The core is what every tool needs and no tool owns:

| File | What it holds |
| --- | --- |
| `state.js` | The connection's state (`C`: me, the project, the open model, its elements, claims, links), the constants, small helpers, `api` and `post` |
| `frame.js` | The ribbon host: the demo's frame, with the sample taken out of what is connected, and `connectRibbon` |
| `hub.js`, `browser.js`, `open.js` | Signing in, the project browser, opening a model or a package |
| `shapes.js` | What the page builds with the geometry compiled for the browser, and the checks against the server's checkpoints |
| `channel.js` | The connection and the ordered log: the socket, `edit`, `receive`, `apply`, `turnBack`, claims let go |
| `undo.js` | This person's own steps, undo and redo |
| `parameters.js`, `problems.js`, `dialogs.js` | The side panel host's own rows, problems and what a palette or dialog sends, the dialog frame and the Types and Project parameters dialogs |
| `plan.js` | The drawing surface: the plan viewport, its redraw, picking and dragging, claims held, the properties palette, the hint line |
| `viewport3d.js` | The 3D viewport, loaded only when the view opens |

### The contract a tool module meets

A tool module imports what it needs from the core, holds its own functions, and registers what the core asks it, once, when it loads. The kinds are the registry's, each a list in `order` (then in the order of registration), Transport's way (`demo/transport/js/registry.js`): one call per kind, its fields checked when it registers, so a mistake is a `TypeError` naming the tool and the field at load, not on the first click. The floor tool's, for example:

```js
register.tools({ id: 'floor', discipline: 'architecture', order: 20 });
register.ribbons({ id: 'floor', order: 10, ribbon: (config) => floorRibbon(config) });
register.hints({ id: 'floor', order: 30, hint: () => (S.tool === 'floor' || S.tool === 'roof' ? floorHint() : undefined) });
register.palettes({ id: 'floor', order: 10, palette: (piece) => floorPalette(piece) || undefined });
register.elements({ id: 'floor', order: 20, owns: isFloor, inPlan: (piece, data, scale) => floorInPlan(piece, data, scale) });
register.drawOrders({ id: 'floor', order: 30, drawOrder: (el) => (isFloor(el) ? 0 : undefined) });
register.layers({ id: 'floor-sketch', order: 50, over: () => floorSketch() });
```

| Kind | What the tool gives | Where the core asks it |
| --- | --- | --- |
| `disciplines` | id, label | the discipline a tool's ribbon belongs to |
| `tools` | its `S.tool` id, discipline, `place(at, event)`, `commands`, `test` (its two-browser section) | a click in the plan while it is the tool |
| `ribbons` | `ribbon(config)` | the ribbon host: under 1000 beside the demo's own buttons, 1000 and over after the core's (a selection's own Modify tab) |
| `hints` | `hint()` | the line under the ribbon: the first answer |
| `palettes` | `palette(piece)` (`always` to be asked after an earlier answer) | the side panel: the first answer |
| `properties` | `properties(piece)` | rows under the core's own for a selected element: every answer |
| `selections` | `selection(id)` | the side panel for a selection that is not an element (a dimension, a grid) |
| `elements` | `owns(el)`, `inPlan`, `unbuilt`, `pickOnly`, `drags(el, hosted)`, `move` | the plan: who draws an element, whether a pick claims and drags it, the command a hosted one moves with |
| `drawOrders` | `drawOrder(el)` | how the plan stacks the elements |
| `layers` | `before()`, `under(scale)`, `over(scale)`, `note()` | the plan's layers around the elements |
| `models` | `open(model)` (awaited in order), `forget()` | a model opening and closing |
| `logs` | `entry(entry)` | each entry of the log for the open model |
| `dialogs` | `html(m)` by the dialog's name | the Manage tab's dialog frame |

A hook that has nothing to say returns `undefined`; for a question one tool answers, the core takes the first answer that is not `undefined`. Every registration names its `order`, unique in its kind: the place the core asked it in before the cut (the if-chain's position, by tens), so the order the files load in never decides anything. A new tool picks the number that puts it where it belongs.

A tool's own buttons and fields keep the demo's tables (`ACT.<name>` for a click, `CHG.<name>` for a change, `DTOOLS`, `TOOLNAME`, `WORDS`, `STEP_WORDS`): they are already registries by name, and the ribbon, the palettes and the log already read them. A tool's keys are its own listeners, in its own file (the floor tool's Enter and Escape, the opening tools' corners), added when it loads, in the list's order, as they were added before; the core keeps only Delete. A tool's `commands` and `test` fields are in the contract and checked when given; the cut leaves them empty: the job on command shapes (below) fills `commands` and checks the page's edits against them, and each tool's `test` is filled once the two-browser test names its sections.

**Adding a tool** is a file under `tools/<discipline>/`, its line in `tools/index.js`, and its section of the two-browser test. The core is not edited.

### Where the core still names a tool

The cut moved every tool out and gave the core hooks where it asked tools most (the ribbon, the hint, the side panel, the plan's drawing, picking and dragging, a model opening and closing, the log, the dialogs). The rest the split binds late: where a core file still calls a tool's function, it calls `late.<name>`, filled when the tool's file has loaded (`core/late.js`), so the core never imports a tool and every such place is listed. After the cut there are 33 (the split's report lists them): the seating of families and curtain walls in a wall's shape (`shapes.js`), Workshop's elements, the browser's Views, Schedules and Warnings parts, the levels' names, the 3D view's colours and the status bar's warning count. Each is a hook still to make; the next jobs turn them into kinds as above, one at a time, the same way.

Delete and the tool switch stay chains of `ACT.stackdelete` and `ACT.tool` wrappers, each tool around the last, as before: they work through the demo's own table, and turning them into a `remove(piece)` hook is one of those jobs.

### Disciplines and packs

A discipline registers as a tool does (`register.disciplines({ id, label, order })`; Architecture and Workshop today), and a tool names its discipline, so one ribbon per discipline (the shared ribbon's rule) is built from what is registered.

Packs follow Transport exactly: a pack's files load between `beginPack({ id, name })` and `endPack()`; everything they register carries the pack's id and takes part only on a project of that pack; a pack that registers the same id as a core entry replaces it for that pack only. A country pack (a country's wall and floor types, its sheet borders, its rules) and a content pack (a firm's or a supplier's tools) plug in the same way. No Construct pack exists yet; `app/packs/README.md` says how one is added.

One registry shape across Spareware: Construct's has Transport's kinds, field checks, ordering and packs. It is an ES module where Transport's is a classic script; a later job may lift the two into one shared file under `design/`.

### What did not change at the cut

The cut moves code; it does not change what any line does:

- Each tool's functions moved to its module unchanged, as text (only de-indented, outside strings).
- The core calls a tool's hooks directly, in the order it called the functions they wrap, so an exception still goes where it went. (Transport's `Registry.call` catches a plug-in's exception; whether Construct does is decided later, separately.)
- Every top-level statement that does something when the page loads (a listener added, an `ACT` entry wrapped) runs in the same order as before, and the split checks it (below).
- One thing moves in time, by the nature of a module: the page's code runs once the document has been read, as every module does, rather than at the point the script stood, which was already the end of the document. The demo's own start has run before it either way. One part of the page leaned on the old moment: Workshop's frame, which the demo's start opens on a reload of Workshop's view, finds its way into the model (`spare.workshop`) once, as it loads, and could now load first and find none. The frame host loads such a frame again when it starts (`reloadEarlyFrame`), so it finds it as before.
- The two-browser test ran unedited.

### The split, repeatable on any newer main

Other lanes keep changing `connect.js` while this piece is built, so the split is a program (lane tooling, `page-split`), and the cut is made by running it on the newest main, never by hand-merging. It:

1. Parses `connect.js` with a JavaScript parser and its scope analysis (the `oxc` crates, 0.160.0, MIT), so a name a function declares for itself is never mistaken for a top-level one.
2. Applies the seams first: small edits inside named core statements, each found by its exact text (the place the core named a tool becomes a loop over a kind), and each tool's registrations appended to its file. When another change has moved a seam's text, it stops and names it.
3. Gives every top-level statement a file, by its section heading (`// --- ... ---`) and, inside a section holding two tools, by cut points (a name). A heading it does not know stops it.
4. Writes each file: its statements, then `export` of what files loaded after it use, and at the top `import` of what it uses from files loaded before it; a use the other way round is `late.<name>`.
5. Refuses what ES modules cannot do as one file did: a binding assigned from another file, a name read at load before its file has run (it works out the browser's own evaluation order of the modules), a change of order between two statements that both touch the same thing at load (an `ACT` entry, a listener of one kind, anything it cannot read), two registrations of a kind with one order, and `this` in a top-level function.

### Carrying open branches across

A branch made before the cut changes the one-file `connect.js`. The port helper (beside the split) carries it over: the branch first merges the main the cut was made from (as its owner merges main anyway); then the split runs on that main's `connect.js` and on the branch's, which gives the branch's change in the new files; the branch's other changes merge as usual, with the cut's entry for `connect.js`; and each new file is a three-way merge (`git merge-file`) of the tree's file with the two splits. A branch that adds a section, or changes a line a seam moved, stops the split with its name, for the branch's owner to place.

## The command layer (the next job)

### The same rule in Rust

A tool's Rust part is its module (`construct-api/src/cloud/<tool>.rs`, already there for every tool) implementing one trait, plus its server module for its reads and routes. Nothing outside names it but the one list.

```rust
pub trait Tool {
    /// Its name, as the page's `register.tool` names it.
    const NAME: &'static str;
    /// The commands it runs, every one behind `model.edit`, in the same access group as an element's.
    const COMMANDS: &'static [&'static str];
    /// The record kinds it writes and their categories.
    const KINDS: &'static [(&'static str, CategoryId)] = &[];
    /// Its warning kinds.
    const RULES: &'static [Rule] = &[];
    /// Its schedule fields.
    const FIELDS: &'static [FieldDef] = &[];
    /// Runs one of its commands.
    async fn apply<T: Txn>(staged: &mut Staged<'_, T>, project: &Id, context: &Context<'_>, edit: &Edit, claim_ms: u64) -> Result<Outcome, StoreError>;
    /// Refuses a generic command (an element's or a parameter's) on its own records, naming its own command.
    async fn guard<T: Txn>(staged: &mut Staged<'_, T>, project: &Id, edit: &Edit) -> Result<Result<(), String>, StoreError> { Ok(Ok(())) }
    /// Turns back an undo that would leave its records inconsistent.
    async fn undo_rules<T: Txn>(txn: &mut T, project: &Id, document: &Id, written: &Written, deleted: &Deleted) -> Result<Result<(), String>, StoreError> { Ok(Ok(())) }
}
```

(The signatures are today's functions, gathered; the job settles the exact types.)

**The one list.** A small declarative macro in `cloud/mod.rs` takes the tools in order and declares their modules: `tools! { walls, floors, roofs, curtains, rooms, stairs, ... }`. Its line is the ONLY shared line a new tool adds on the Rust side (the server's own list, for its reads and routes, is the same macro in the server crate, one line per tool that has reads). It expands to the `pub mod` lines (which Rust needs anyway) and to the dispatch that today is written out by hand:

| Today | From the list |
| --- | --- |
| `permit`'s and `permit_group`'s chains of `is_command` | any tool's `COMMANDS` |
| `apply`'s `if ... else if` over the modules | the tool whose `COMMANDS` hold the command |
| the nested `generic_rules` chains | every tool's `guard`, in the list's order, before a generic command |
| `undo::is_step`'s chain and the `undo_rules` chain | every tool's `COMMANDS`, and every tool's `undo_rules` in order |
| construct-categories' kinds table, for a tool's kinds | the tools' `KINDS`, checked against the built-in tree by a test |
| construct-warnings' `RULES` | the tools' `RULES`, sorted by kind as today |
| the schedule fields | the tools' `FIELDS` |
| the server's reads and routes | each tool's server module's `routes()`, merged by the server's own list |

The list's order is today's order of the chains, so every answer, refusals' words included, stays the same.

**Why a list and not distributed registration.** Crates that gather registrations from all over the program at link time (`linkme`, `inventory`, both MIT or Apache-2.0) were considered. They are not used, for two reasons. The commands are generic over the store's transaction (`T: Txn`, the database's on the server and the memory store's in tests), and a generic function cannot be registered as a value without boxing every call behind a type-erased transaction, a change to kit-store. And their order is the linker's, which the golden rules forbid in anything compared (a refusal's words depend on which guard answers first); sorting fixes that only by adding an order number to every tool, a list in another form. A one-line-per-tool list has neither problem and costs the same line the `mod` declaration already costs.

**Layers.** construct-categories, construct-warnings and construct-schedules sit below construct-api and cannot see the tools. They keep their own types; the tools' constants are handed to them by the api layer (the warnings take a slice of rules, the schedules a slice of fields), and the built-in category tree stays a table of construct-categories (main's ruling), with a test that every kind a tool declares has a category in it.

**Format and tests.** A tool's commands are described in its own section file (`crates/construct-api/format/<tool>.md`), which construct-model's `FORMAT.md` links in the list's order; a test checks that every command in a tool's `COMMANDS` is described in its file. A tool's server tests are their own file (`server/tests/<tool>.rs`, a test binary Cargo finds by itself) on a shared harness (`server/tests/common/`), not more lines in `tests/service.rs`.

**Order of work.** One tool at a time, each its own pull request that changes no answer: (1) the trait, the macro and the list holding every tool, with the hand-written chains replaced by the expansion and every existing test passing unchanged; (2) warnings' rules and schedules' fields from the tools; (3) the server's reads and routes per tool; (4) the format files and the per-tool test files.

### The shapes of commands (the job after)

The page and the server can still disagree on what a command's arguments are: the page writes them by hand, the server reads them with `serde` into a struct per command. The server's structs can give their shapes as JSON Schema with `schemars` (MIT; version 1.2.3 is already in the server's lock, through `serde_with`): derive `JsonSchema` beside `Deserialize`, a tool's trait gains `fn schemas()`, the server answers `GET /api/commands` with every command's schema, and the page, in the test, checks every edit it sends against it.

It is larger than a small change, so A96 does not do it: about ninety argument structs across twenty-one modules gain the derive; `kit_core::Id` (kit-core has no dependencies, by rule) and the length texts need a schema by hand (`#[schemars(with = "String")]`); and the page needs a JSON Schema checker, which it would have to take as a pinned, permissive package rather than write. It is the job after the trait, since the trait is where `schemas()` goes. construct-api's `schema.rs` (hand-written schemas for Transport's commands) is the first thing it can replace.

## Why

- A core that names no tool is the only way two trains stop meeting in the same lines: two tools added at once each add a file and a line in a list, and a merge of two new lines in a list is at most a trivial conflict.
- The registry is the shape Spareware's Transport page already proved (tools, panels, packs replacing core parts by id), so there is one way to extend a Spareware page.
- An ordered list on both sides keeps the golden rule: the same input gives the same answer, in the same words, on every run.
- Splitting by a program that is re-run, not by hand, is the one way to cut a file twenty lanes are changing without losing a change.

## Not decided here

- Whether the core contains a tool's exception (Transport's `Registry.call`) or lets it go where it goes now.
- A shared registry file for Transport and Construct under `design/`.
- Discovery without the list (the server listing `app/tools/` itself) once no tool depends on its load order.
