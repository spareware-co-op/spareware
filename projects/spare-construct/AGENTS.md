# Working on Spare Construct

This file is for every agent and person who changes anything in `projects/spare-construct/`. The repository's own [AGENTS.md](../../AGENTS.md) still applies in full: check out a piece before touching anything, work only on its branch, keep the legal rules, sign off every commit, and hand over properly. This file adds what is particular to Spare Construct, and never weakens the root file; where the two seem to differ, the root file wins.

1. [Before you start](#1-before-you-start): what to read.
2. [Where things are](#2-where-things-are): the crates, the server and the app, and what each owns.
3. [The rules](#3-the-rules): what is particular to this program.
4. [Notes](#4-notes): how research and decision notes are written.
5. [The commands](#5-the-commands): what CI runs, and what it does not.
6. [Dependencies](#6-dependencies).
7. [When something does not fit](#7-when-something-does-not-fit).

A path in backticks is written from the repository root when it starts with `projects/`, `tools/` or `.github/`, and from this folder otherwise.

## 1. Before you start

- **Your piece.** Read its row on the work board in [STATUS.md](STATUS.md), its issue (`gh issue view <number> --repo spareware-co-op/spareware`), the section of [ARCHITECTURE.md](ARCHITECTURE.md) it belongs to, and the part in [FEATURES.md](FEATURES.md) that links it. "Done when" on the board is the contract.
- **The stage is Mapping.** Most pieces are notes. Product code exists only where section 2 says so: the Transport discipline, the cloud server and the app that runs on it, all on stand-ins where the real parts are not built yet. A crate with no code waits for the Design stage, and its `lib.rs` says what it waits on. Do not start one before an owner has accepted the decision notes it waits on.
- **Read the notes before you research.** Merged notes are in [notes/](notes/); notes in review are open pull requests from `piece/A..` branches (`gh pr list --repo spareware-co-op/spareware --search "head:piece/A"`). Later pieces build on them.
- **Read the geometry kernel's map before anything geometric.** Many of Spare Construct's questions are answered in `projects/engines/geometry/architecture/`: the sketch solver in chapter 15 (A11), hidden lines and drawings in chapter 18 (A12), the build order in chapter 30. Spare Construct decides what an element, a view or a lock is; the kernel builds and draws the geometry.
- **Interface work** reads [design/DESIGN.md](../../design/DESIGN.md) and uses [design/tokens.css](../../design/tokens.css) first.

## 2. Where things are

| Part | Holds | Tested by |
| --- | --- | --- |
| [`construct-model`](crates/construct-model) | What an element is (inputs only, with its recipe), the `Geometry` interface that is the one join to whatever builds shapes, hubs, projects and folders as rows, and Spare Construct's tables in the store ([FORMAT.md](crates/construct-model/FORMAT.md), a published format) | Unit tests, fuzzing (`kit.yml`) |
| [`construct-regen`](crates/construct-regen) | The stand-in geometry (`stand_in.rs`, boxes until the kernel can build) and, behind its `server` feature, the regenerator that keeps derived shapes up to date | Unit tests, fuzzing, and `construct-api`'s `tests/shapes.rs` on two generators |
| [`construct-shapes`](crates/construct-shapes) | `construct-regen`'s geometry compiled for the browser, built into [app/pkg](app/pkg) by `server/scripts/build-shapes.sh` | Unit tests, and every browser's checkpoints against the server's |
| [`construct-api`](crates/construct-api) | The command and query API, JSON in and out: the Transport commands, and with its `cloud` feature the server's element, folder, package, bridge, link, hub, project and team commands | Unit and integration tests |
| [`construct-cli`](crates/construct-cli) | The command line and the MCP server over `construct-api` | `tests/cli.rs`, `tests/mcp.rs` |
| [`construct-wasm`](crates/construct-wasm) | The Transport command layer for the browser, built into `demo/transport/pkg` by `tools/transport-truth/build-wasm.ps1` | `node tools/transport-truth/wasm-check.mjs` |
| [`construct-transport`](crates/construct-transport) | Transport's calculation core: a faithful port of the browser prototype's scripts in [demo/transport](demo/transport) | Truth tests against answers the prototype recorded (`tools/transport-truth`) |
| `construct-clash`, `construct-elements`, `construct-families`, `construct-interop`, `construct-solver`, `construct-versions`, `construct-views` | No code yet. Each `lib.rs` names its part of the map and what it waits on | |
| [server](server) | The cloud server: its own Cargo workspace, `Cargo.lock` and licence gate (`deny.toml`). Its [README](server/README.md) is the map of how it fits the kit's crates | `tests/`, the checklist on TiKV, two real browsers |
| [app](app) | The browser app: `connect.js`, `head.html` and `body.html`, which the server adds to the demo page | The server's real-browser test |
| [demo](demo) | The demo page, which is also the app's layout, and the Transport prototype | |
| [notes](notes) | Research and decision notes | |

## 3. The rules

1. **Every change is an operation** (the map's ground rule 4). An edit goes through the project's ordered log, for people, scripts and AI alike; there is no write path around it.
2. **Inputs are the truth; shapes are derived.** An element's record holds its recipe. A built shape is a derived record named by the content ID of its canonical bytes, so deleting every shape and replaying the log gives the same bytes.
3. **One line picks the geometry:** `construct_regen::geometry()`. `stand_in.rs` is the only file that knows today's shapes are boxes, and nothing else may rely on it. When the kernel can build an element, that line changes and nothing else does. Geometry reaches Spare Construct only through the facade, `projects/engines/crates/engine-geometry`, never a `geom-*` crate.
4. **The server and the browser build with the same code.** If a change touches `construct-regen`'s geometry, run `server/scripts/build-shapes.sh` and commit `app/pkg` in the same pull request.
5. **The demo page is the app's layout, served unchanged.** The server sends `demo/index.html` as it is and adds the files in `app/`. Change the layout in the demo, never by copying it into the app. Anything the demo shows that is not connected says so, rather than showing sample data.
6. **Transport's porting rule:** the same formulas, operation order and constants as the prototype, and JavaScript's number semantics where it relies on them (`construct-transport`'s `jsnum`). Parity is proven by truth tests. When the prototype changes, record its answers again with `tools/transport-truth` and port the change.
7. **Stored formats are read forever.** A change to a key or record in `construct-model` adds and never rewrites, and updates its FORMAT.md in the same pull request.
8. **Never crash.** No `unwrap`, `expect`, `panic!`, unchecked indexing, `todo!`, `unimplemented!` or `unreachable!` outside tests: return an error the caller can show. Piece A19 brings the geometry kernel's lint tables to these crates and the server; write to them now.
9. **Plain words on screen.** Short sentences, no jargon ([design/DESIGN.md](../../design/DESIGN.md)). Every colour, font and spacing comes from [design/tokens.css](../../design/tokens.css).
10. **Revit's structure, our look.** Tools, panels, commands and workflow may work the way Revit users know (root AGENTS.md, 5). Icons, artwork, colours, fonts, help text and the product's names are our own, and nothing is copied from Autodesk's software or documentation, from memory included.
11. **The discipline list changes only in a piece of its own.** Adding or renaming a discipline is never part of another piece.
12. **No client data.** Fixtures and test projects are generated or anonymised; the example data in `server/src/seed.rs` names made-up people and companies only.

## 4. Notes

- **Name and place:** `notes/<ID>-<slug>.md`, with any spike's files in `notes/<ID>-<name>/` (as `notes/A13-bench/`).
- **Sections:** the question, the options, the evidence, a recommendation and the open risks, then a sources table (`Source | Licence | Used for`).
- **Evidence is source.** Read the code of what you cite, at a pinned commit, and link to the lines. Say on which day sources were read. Mark anything not confirmed in source or by measurement *unverified*.
- **Recommend; never decide.** An owner decides. A note never changes an `ARCHITECTURE.md` decision in the same pull request (root AGENTS.md, 7). If a note reverses the map's lean, say so in its first lines.
- **Spike numbers** give the machine class (for example "an 8-core laptop"), never a machine's or a person's name.
- **LGPL sources** (planegcs, OpenCascade) are read for their method only, described in our own words.

## 5. The commands

What CI runs (`.github/workflows/construct-server.yml`, which runs on changes under `server/`, `app/`, `crates/` and `demo/index.html`), to run yourself before you push:

| Command | Checks |
| --- | --- |
| `cargo clippy --locked --no-deps -p construct-model -p construct-regen -p construct-shapes -p construct-api --all-features --all-targets -- -D warnings` | Lints on the crates the server uses |
| `cargo test --locked -p construct-model -p construct-regen -p construct-shapes -p construct-api --all-features` | Their tests, with the shapes run on two generators |
| `cargo check --locked --target wasm32-unknown-unknown -p construct-shapes -p construct-wasm` | The browser's builds stay free of the store |
| In `server/`: `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked` | The server's lints and every test that needs no database |
| In `server/`: `cargo deny --locked check licenses` (cargo-deny 0.20.2) | Every licence is on `deny.toml`'s list |

`.github/workflows/kit.yml` also runs the fuzz targets of `construct-model` and `construct-regen` with cargo-bolero.

Not in CI: run these yourself when your change touches them.

| Command | When |
| --- | --- |
| `cargo test --locked -p construct-transport` | Any change to Transport's core |
| `cargo test --locked -p construct-cli` | Any change to the command line, the MCP server or a command's schema |
| `powershell -File tools/transport-truth/build-wasm.ps1`, then `node tools/transport-truth/wasm-check.mjs` | Any change that reaches the Transport page in the browser |
| In `server/`: `bash scripts/run-checklist.sh` (Linux; it wipes local test data first) | Changes to the store, the log or the live channel |
| In `server/tests/browser`: `npm ci` then `node two_browsers.mjs` (Node with Edge or Chrome) | Changes to the app, or to what the server sends it |

TiKV needs Linux; the server's README says how to run it locally.

## 6. Dependencies

- **Use good crates** with permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib) rather than writing your own; never hand-roll around a dependency. Write your own only where the map's ground rule 1 says (data model, kernel, regeneration, drawing engine, file format), and say why in the code.
- **Pin it and name its licence.** The server pins every version exactly (`=1.2.3`) with its licence in a comment beside it, as `server/Cargo.toml` does, and its `deny.toml` checks every licence. Crates in the root workspace are pinned by `Cargo.lock`; note a new crate's licence beside it in `Cargo.toml`.
- **Browser builds stay apart.** `construct-shapes` and `construct-wasm` never take the store or the server's crates (the WebAssembly check in section 5).
- **No GPL or AGPL code** anywhere, and no program depends on another program (root ARCHITECTURE.md's layering rule): shared needs move into `projects/kit` or `projects/engines`.

## 7. When something does not fit

- **It waits on the kernel.** Say which kernel piece it waits for (its G number) where the part is described and in the interface, and do not build a substitute beyond the stand-in.
- **Something outside your piece looks broken.** Note it on the pull request; do not fix it.
- **A rule here gets in the way.** Say so on the pull request and ask an owner; do not work around it.
