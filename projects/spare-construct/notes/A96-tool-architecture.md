# A96: A tool is one module on each side, and the core never names it

**Status, 2026-11-28:** Decided for the build (piece A96). The page half is built by A96; the Rust half is the next job and is written here so that it can start at once. Part: CON-A8 (the page, all of it), and the command layer under it.

## The question

The owner, 28 Nov 2026: the page, and the whole tool architecture under it, is to be "designed in a smart, expandable way", expandable for ever and easy to maintain.

It is not. Every tool is written into the middle of shared lists:

- **The page.** `app/connect.js` is one file (5,564 lines on main, 7,303 with train 11) holding every tool. The core's functions name each tool: `drawPlan` has an `if` per kind, `propertiesPalette` a branch per tool, `hint` a line per tool, the plan's click handler a branch per placing tool, `connectRibbon` a call per tool, and Delete and the tool switch are chains of wrappers each tool adds around the last. Every train fights conflicts in this one file, and adding a tool means editing the middle of it.
- **The command layer.** A tool's commands live in its own module (`construct-api/src/cloud/<tool>.rs`), but it is also written into: `cloud/commands.rs` three times (`permit`, `permit_group` and `apply`, the last with nested chains of `generic_rules`), `cloud/undo.rs` twice (`is_step` and the `undo_rules` chain), `cloud/mod.rs`, construct-categories' `kinds.rs`, construct-warnings' `RULES`, the schedule fields, the server's `service.rs` reads and `http.rs` routes, construct-model's `FORMAT.md` command list, and `server/tests/service.rs`. Train 11 met conflicts in most of these in a single merge.

The rest of Spareware already has the shape the owner wants: the shared ribbon (`design/ribbon/`), one ribbon per discipline, and the Transport page's registry with country packs (`demo/transport/js/registry.js`, `packs/nz/`, `packs/README.md`, note [CON-A22-F5-P1](CON-A22-F5-P1-transport-country-packs.md)). This note follows that shape and does not invent a second one.

## The rule

**A tool is three things, each in its own files: its Rust part, its page module, and its section of the two-browser test. The core never names a tool. Adding a tool adds files and one line in one list per side, the line a module system needs anyway, and touches no logic outside the tool.**

The one list per side is the same idea as Transport's pack loading (its `index.html` lists a pack's files in order): a list of names, in a fixed order, holding no logic. Its order is the order the core asks the tools in, so it is what makes the page and the server deterministic.

## The page

### Files

```
app/
  main.js                       loads the core, then the tools in order, then starts
  core/                         the core (below); never imports from tools/
  registry.js                   the registry: disciplines, tools, packs
  tools/
    index.js                    the ordered list of tool modules, one line each
    project/<tool>.js           tools of the whole project (levels, views, schedules, warnings, types and parameters, graphics, crop, the families dialog)
    architecture/<tool>.js      wall, door, floor, roof, room, grid, dimension, curtain wall, family, opening, stair
    workshop/<tool>.js          Workshop's parts and assemblies
  packs/
    README.md                   how a country or content pack plugs in
```

Plain ES modules: no framework and no build step. `body.html` loads `app/main.js` as a module where it loaded `connect.js`. The server serves the whole `app/` folder from one embedded tree (the `include_dir` crate, MIT or Apache-2.0, pinned), so a new file needs no route; today every page file is its own `include_str!` and route.

### The core

The core is what every tool needs and no tool owns. Each part is a module of `app/core/`:

| Part | What it holds |
| --- | --- |
| State | The connection's state (`C`: me, the project, the open model, its elements, claims, links), the constants, small helpers |
| Connection and the ordered log | `api` and `post`, the socket, `send`, `receive`, `apply` (an entry from the log into the elements), `edit` (an edit up the socket, drawn at once, confirmed or turned back), `turnBack` |
| Claims | `heldClaim`, `claimedByOther`, `letGo`, the claim when a drag starts |
| Undo and redo | the person's own steps from the server, `undoOrRedo`, the quick access bar's two buttons |
| Selection | `select` and what changes with it |
| The drawing surface and redraw | the plan viewport (`plan`, `drawPlan`, `queueDraw`, the pointer handlers that pick and drag), the 3D viewport host, shapes and their checks against the server's checkpoints |
| The side panel host | the properties palette: the core's own rows for any element, then each tool's part |
| The ribbon host | the demo's ribbon config, changed where the connection gives a button meaning, then each tool's buttons |
| The palette | the command palette and keyboard keys a tool registers |
| Problems and dialogs | `problem`, `manage`, `refused`, `done`, the dialog frame |
| Hub, projects and models | signing in, the project browser, opening a model or a package |

### The contract a tool module meets

A tool module imports what it needs from the core and registers itself once, at load:

```js
import { register } from '../../registry.js';
// ...the tool's own functions, moved unchanged...
register.tool({
  id: 'floor',                       // kebab-case, unique
  discipline: 'architecture',        // the discipline whose ribbon shows it
  commands: ['floor.create', 'floor.type', 'floor.slope', 'floor.edge'],  // what it sends up the log
  owns: isFloor,                     // which elements are its own, to draw, pick, show and delete
  ribbon: floorRibbon,               // (config) => adds its buttons to the ribbon config
  inPlan: floorInPlan,               // (piece, data, scale) => the SVG of an element it owns
  sketch: floorSketch,               // () => SVG drawn over every element while it is in use
  palette: floorPalette,             // (piece) => its part of the side panel, or '' when it has none
  hint: floorHint,                   // () => the line under the ribbon while it is in use
  place: null,                       // (at, event) => a click in the plan while it is the tool
  remove: null,                      // (piece) => Delete of a selected element it owns
  test: 'Floors, from a sketch',     // the label of its section of the two-browser test
});
```

Every field but `id` and `discipline` is optional. The registry checks the fields when a tool registers (Transport's way: a field of the wrong type is a `TypeError` naming the tool and the field), so a mistake shows at load, not on the first click.

The core asks the tools, in the list's order, at each place it used to name them:

| Where in the core | What it asks |
| --- | --- |
| The ribbon host | every tool's `ribbon(config)` |
| `drawPlan` | for each element, the first tool that `owns` it draws it (`inPlan`); then every tool's `sketch()` |
| The side panel host | the selected element's owner's `palette(piece)`, or the active tool's panel |
| The hint line | the active tool's `hint()` |
| A click in the plan with a tool | the active tool's `place(at, event)` |
| Delete | the selected element's owner's `remove(piece)`, else the core's own delete |
| Keys and the command palette | the tool's `keys` and `commands` entries, each with its words and its key |
| The log | a tool's `open(model)`, `forget()` and `entry(model, into, entry)`, for what it keeps beside the elements |

A tool's own buttons and fields keep the demo's tables (`ACT.<name>` for a click, `CHG.<name>` for a change): they are already a registry by name, and the ribbon and the side panel already call through them.

### Disciplines and packs

A discipline registers as a tool does (`register.discipline({ id, label, tab, order })`): the ribbon tab it owns and whether it is connected to the project. A tool names its discipline, so one ribbon per discipline (the shared ribbon's rule) is built from what is registered.

Packs follow Transport exactly: a pack's files load between `beginPack({ id, name })` and `endPack()`; everything they register carries the pack's id and takes part only on a project of that pack; a pack that registers the same id as a core tool replaces it for that pack only. A country pack (a country's wall and floor types, its sheet borders, its rules) and a content pack (a firm's or a supplier's tools) plug in the same way. No Construct pack exists yet; `app/packs/README.md` says how one is added, after Transport's `packs/README.md`.

One registry shape across Spareware: Construct's registry has Transport's API (`register.<kind>(def)`, `list`, `get`, `beginPack`, `endPack`, `country`), its field checks and its ordering (`order`, then the order of registration). It is an ES module where Transport's is a classic script. A later job may lift the two into one shared file under `design/`.

### What does not change at the cut

The cut moves code; it does not change what any line does:

- Each tool's functions move to its module unchanged, as text.
- The core calls a tool's hooks directly, as it called the functions they were, so an exception still goes where it went. (Transport's `Registry.call` catches a plug-in's exception; whether Construct does is decided later, separately.)
- Every top-level statement that does something when the page loads (a listener added, an `ACT` entry wrapped) runs in the same order as before: the tools' list is the old file's order, and the split checks the order (below).
- The two-browser test runs unedited.

### The split, repeatable on any newer main

Other lanes keep changing `connect.js` while this piece is built, so the split is a program, kept beside this piece's lane scripts, and the cut is made by running it on the newest main at the end, never by hand-merging. It:

1. Parses `connect.js` with a JavaScript parser (the `oxc` crates, MIT, pinned in the script's own Cargo.toml) and resolves every name to its declaration, so a name a function declares for itself is never mistaken for the core's.
2. Gives every top-level statement a file, by its section heading (the `// --- ... ---` lines) and, where a section holds two tools, by name. A heading or a name it does not know stops it, naming the line: the table gets a line, and it runs again.
3. Writes each file: the statements, de-indented, then an `export` of what other files use, and at the top an `import` of what it uses from others.
4. Refuses what ES modules cannot do as the one file did: a binding assigned from another file (an import is read-only), a name read while the page loads from a file that has not loaded yet, and any change in the order of top-level statements that do something.
5. Applies the seams: the core's dispatch functions (above) are replaced by their registry versions, kept as files beside the script. Each seam records a hash of the original it replaces; when another lane has changed that original, the split stops and prints the difference, so the seam is brought up to date before the cut, never merged blind.

### Carrying open branches across

A branch made before the cut changes `connect.js`. The port helper (beside the split, with `port.md`) carries it over: it takes the branch's change to `connect.js` against its merge base, runs the split on the base and on the branch's version, and applies the difference file by file to the new files. A function the branch added goes to the file the split's table names for its section; one it changed goes to wherever that function now lives. What cannot be placed (a change to a seam function, or a new section) is printed for the branch's owner to place by hand.

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

**The one list.** A small declarative macro in `cloud/mod.rs` takes the tools in order and declares their modules: `tools! { walls, floors, roofs, curtains, rooms, stairs, ... }`. It expands to the `pub mod` lines (which Rust needs anyway) and to the dispatch that today is written out by hand:

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
