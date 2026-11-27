# Spare Construct

Replaces the building-modelling programs people pay for. Stage: Mapping for most of the map; the first code is built and running as a test server.

## What a person can do in it today

Open the browser page and:

- Sign in with Google, pick a hub (a company) and a project.
- See the project's WIP, Shared and Consumed folders.
- Open a plan and a 3D view.
- Draw a wall (two points) and place a door (a click on a wall), and see its properties.
- Edit with a second person on the same model and see each other's edits live.
- Share a package, bridge it to another hub, take it in, and make links.

Everything else in the page says it is not connected yet. There is no model file: the model lives on the server. Walls, doors and blocks are boxes for now. There are no floors, roofs, stairs, rooms, families, sheets, schedules, or file import and export yet.

Traffic and public transport is the furthest along. Its calculation core is the crate `construct-transport`, and it has a browser demo in `demo/transport`. Workshop has a browser prototype in `demo/workshop`.

## What the code is

- `crates/`: one Rust library per part of the map.
  - Real: `construct-model` (the geometry interface, elements, rows and the stored format), `construct-regen` (the stand-in that makes boxes, and the regenerator that rebuilds shapes after each edit), `construct-shapes` (the same geometry for the browser), `construct-grids` (grid lines and reference planes: their names and the next-name rule, straight and arc geometry, crossings, and how they are stored), `construct-api` (the commands: elements, grids and reference planes, views, folders, packages, bridges, links, hubs, projects, teams, access rules; and the transport commands), `construct-cli` (command line and MCP server), `construct-wasm` and `construct-transport` (transport), `construct-views` (views as elements: the kinds, the range, what a view shows) and `construct-schedules` (schedules as stored queries: fields, filters, sorting, and their live rows).
  - Short placeholders, no code yet: `construct-elements`, `construct-families`, `construct-clash`, `construct-interop`, `construct-solver`, `construct-versions`.
- `server/`: the Rust web server on the shared kit crates, with TiKV as the database, PD as its coordinator, RustFS for large files, Caddy for https, and Cedar for access rules. See [server/README.md](server/README.md).
- `app/`: the browser page, on the demo's layout, with live data from the server. See [app/README.md](app/README.md).
- `demo/`: the full interface mock-up, and the transport and workshop prototypes.
- The acceptance checklist is in [server/README.md](server/README.md). It records all six checks passing, last run on 31 October 2026.

## How geometry is reached

One trait, `Geometry` in `construct-model`, takes a recipe and returns a body, bounds, named faces and display meshes. Today a stand-in that makes boxes answers it. The kernel's interface, with solidcore (the solid-modelling code behind the kernel) behind it, is being joined to that trait now.

## Where the plan is

[FEATURES.md](FEATURES.md) is the full list of what to build. The build order is being worked wave by wave, starting with the element core.

## Rules a builder keeps

- [AGENTS.md](../../AGENTS.md): how work is done, and the rules that never bend.
- [EXTENDING.md](../../EXTENDING.md): how a new piece is made.
- [ARCHITECTURE.md](ARCHITECTURE.md): the full map and its decisions.

Also here: [STATUS.md](STATUS.md) (the work board) and `notes/` (research and decision notes).
