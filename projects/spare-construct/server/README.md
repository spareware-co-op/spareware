# Spare Construct's cloud server

The real server for the Spare Construct app, running now as a test server. It is the thin program that wires the kit's generic crates and Construct's own into one server. It runs on one free server today and moves to three by changing settings, not code ([deploy/README.md](deploy/README.md)). Decisions are in note [K08](../../kit/notes/K08-cloud-foundation.md).

What it does:

- **Browser only.** No model file. The page loads only the pieces in view and keeps them by hash in the browser's cache.
- **The model is pieces in TiKV**, changed only through one ordered edit log per project ([note A05](https://github.com/spareware-co-op/spareware/pull/168)'s design, following Replicache and Figma's published method).
- **Access is rows, checked by Cedar on every request.** The web layer can't reach the store: every read and write goes through [`Service`](src/service.rs), which asks first.
- **Every write goes through a log.** The directory has one too, so seed data and administrators use the same doors as everyone. The exceptions are in K08, with why.
- **The hierarchy:** people, hubs, projects, teams, WIP, Shared and Consumed folders, and bridges between hubs (CON-A25, CON-A26).

### Shapes

Elements are recipes, and shapes are built from them. Boxes are a stand-in behind the same shape the geometry kernel will use, so swapping them changes one module and not the data layout, the edit log or the app.

- **One interface.** [`Geometry`](../crates/construct-model/src/geometry.rs) is the one join: a recipe goes in, and a body, bounds, named faces and display meshes come out. The stand-in is [one implementation](../crates/construct-regen/src/stand_in.rs), and [`construct_regen::geometry`](../crates/construct-regen/src/lib.rs) is the one line that picks it.
- **Inputs are the truth.** An element's record holds its recipe, and edits carry inputs and before and after values. A built shape is a derived record named by the content ID of its canonical bytes: delete them all and the log makes the same bytes again.
- **The server and the browser both build through it.** The browser runs the same code compiled to WebAssembly ([`construct-shapes`](../crates/construct-shapes)). After each pass the server writes a checkpoint of the bodies it built into the log, and every browser compares them with its own, so a mismatch is caught from day one (note A05).
- **Shapes per body, display meshes by level of detail,** with large ones in file storage, linked by hash.
- **Room for what comes next:** a door is hosted on a face of a wall by persistent name, each document records its kernel edition, and links go by document reference. A browser that can't run a document's edition is a viewer of the stored shapes.

[tests/swap.rs](tests/swap.rs) runs the whole server on a second, different shape generator, then compares the stores: same keys, same log.

**The page is the Spare Construct demo.** The server sends [the demo's own file](../demo/index.html) unchanged and adds [app/connect.js](../app/connect.js), so the ribbon, palettes, view tabs and status bar are the real ones and stay in step with the demo. Connected to live data: sign-in, the hub switcher, the project list, WIP, Shared and Consumed folders in the project browser, the plan and 3D views, properties, the Wall and Door tools, Share, Bridge, Take in, and links. Everything else in the demo says it isn't connected yet, rather than showing sample data.

## Where everything is

| Part | Holds | Tested by |
| --- | --- | --- |
| [`kit-core`](../../kit/crates/kit-core) | Permanent IDs. No dependencies, because the geometry kernel reads this crate | Unit tests |
| [`kit-ops`](../../kit/crates/kit-ops) | The log entry, the format kept forever, the access question | Unit tests, recorded samples, fuzzing |
| [`kit-store`](../../kit/crates/kit-store) | The generic key layout ([FORMAT.md](../../kit/crates/kit-store/FORMAT.md)), versioned rows, the storage traits, a test store with injected faults | Unit tests, fuzzing |
| [`kit-sync`](../../kit/crates/kit-sync) | The orderer, claims, replay, the live channel's rules | [Failure tests](../../kit/crates/kit-sync/tests/failures.rs), fuzzing under random faults |
| [accounts](../../kit/services/accounts) | People, Cedar, sign-in, sessions | [People tests](../../kit/services/accounts/tests/people.rs) |
| [store](../../kit/services/store) | The traits on TiKV and `object_store` | [Promises](../../kit/services/store/tests/promises.rs) and [load](../../kit/services/store/tests/load.rs), on real TiKV |
| [`construct-model`](../crates/construct-model) | The geometry interface, elements, rows and Construct's tables ([FORMAT.md](../crates/construct-model/FORMAT.md)) | Unit tests, fuzzing |
| [`construct-regen`](../crates/construct-regen) | The stand-in, and the regenerator that keeps derived shapes up to date | Unit tests, and [shapes.rs](../crates/construct-api/tests/shapes.rs), on two generators |
| [`construct-api`](../crates/construct-api) | With its `cloud` feature: Cedar's rules, element commands, folders, packages, bridges, links, hubs, projects and teams | [shapes.rs](../crates/construct-api/tests/shapes.rs), [directory.rs](tests/directory.rs) |
| [`construct-shapes`](../crates/construct-shapes) | The geometry for the browser | Unit tests, and every browser's checkpoints |
| [app](../app) | The browser app on the demo's layout | [Real browsers](tests/browser/two_browsers.mjs) |
| This folder | The web layer, the live channel, seed data, deployment | [Rules in-memory](tests/service.rs), [the swap](tests/swap.rs), [the checklist](tests/checklist.rs) |

| Outside program | What it is | Licence |
| --- | --- | --- |
| TiKV 8.5.8 | The database (Rust; RocksDB inside) | Apache-2.0 |
| PD 8.5.8 | TiKV's coordinator (Go) | Apache-2.0 |
| RustFS 1.0.4 | The file store, speaking S3 | Apache-2.0 |
| Caddy, on the server | https, with free certificates | Apache-2.0 |

The stored format is published and read forever: the [kit's](../../kit/crates/kit-store/FORMAT.md) and [Construct's](../crates/construct-model/FORMAT.md).

### The example data

Hubs Company 1, 2 and 3. Anna, Ben and Cara are home in Company 1, Dan in Company 2, Eve in Company 3; Dan and Eve are external users of Company 1. Example model 1 has Anna and Ben, with Dan external; Riverside Flats has Cara, with Eve external. Company 2 holds the bridged side of Example model 1 for Dan's Structure team. Company 1 and 2 permit bridges with each other; Company 3 doesn't, so a bridge there is refused.

Sign-in is one path: OpenID Connect. Tests talk to a [stand-in provider](tests/support/mock_oidc.rs), which nothing but a test can reach; the server talks to Google. There's no test back door in the server. There is no sign-up: `STACK_PEOPLE` says which Google accounts sign in as which person.

## Running it

TiKV needs Linux: use WSL or any Linux machine, with Rust and a C compiler. Nothing needs root, unless your limit on open files is under TiKV's 123,880, which `stack-up.sh` checks.

```bash
cd projects/spare-construct/server
cargo test                          # everything that needs no database
bash scripts/install-binaries.sh    # TiKV, PD and RustFS into ~/spare-stack
bash scripts/run-checklist.sh       # wipes local test data, then promises, load, checklist
```

`STACK_NODES=3 STACK_PD=127.0.0.1:2379,127.0.0.1:2389,127.0.0.1:2399 bash scripts/run-checklist.sh` runs the same on three copies of PD and TiKV, and kills one TiKV mid-edit.

To use the page, run `bash scripts/run-browser-test.sh`, open <http://localhost:8080/auth/login> and pick a person. To be two people at once, use two browser profiles.

The real-browser test runs from any machine with Node and Edge or Chrome:

```bash
cd projects/spare-construct/server/tests/browser
npm ci
node two_browsers.mjs
```

The kit's and Construct's crates are tested from the repository root: `cargo test -p kit-core -p kit-ops -p kit-store -p kit-sync`, and `cargo test -p construct-model -p construct-regen -p construct-shapes -p construct-api --all-features`. The kit's services are their own workspace, `projects/kit/services`.

After changing what the geometry builds, run `bash scripts/build-shapes.sh` and commit `app/pkg`. It needs the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.118.

Run `cargo fmt` here, never `cargo fmt --all`: it follows the path dependencies back into the root workspace and reformats every other project.

## Acceptance checklist

This is the owner's test of the whole stack, and it stays the test as the stack grows. Last run on 31 October 2026, in release builds, on a 12-core Linux machine with TiKV sized for the free server. [tests/checklist.rs](tests/checklist.rs) runs it against the real programs, and [tests/browser](tests/browser/two_browsers.mjs) runs it again in real Edge browsers with real mouse drags, on the demo page.

| # | Check | Result |
| --- | --- | --- |
| 1 | Anna logs in and lands in Company 1 | Passes. Someone not on the server is refused, and nothing opens without signing in |
| 2 | Dan switches between Company 2 and Company 1 | Passes. Each hub shows only the projects he is on |
| 3 | Two browsers editing Example model 1 each see the other's edit in under 200 ms | Passes. Real browsers: median 24 ms, worst 67 ms. Test clients, 200 edits each way: median 22 ms, worst 130 ms |
| 4 | Dan can't see the Architecture team's WIP; Arch v12 can be shared, bridged and consumed | Passes. A bridge to Company 3 is refused because only one hub permits it. What Dan consumes has the same hash as what Anna shared |
| 5 | Killing and restarting the server mid-edit loses nothing | Passes. 400 edits through 2 hard kills: exactly 400 log entries, and the log replays to exactly what is stored. The real page reconnects by itself mid-drag |
| 6 | A backup restores | Passes. After wiping TiKV and RustFS, every key and file came back byte for byte, the logs replay, and work carries on |

Shapes run through all of it. Every `verify` above also rebuilds each stored shape from its recipe and compares the bytes. In real browsers, every body built on the page matched the server's checkpoint, and a browser that can't load the geometry still draws the stored shapes and sends no edits.

### Moving to three machines, by settings alone

The whole checklist passes on three copies of PD and TiKV, with three copies of every write, changing only `STACK_NODES`, `STACK_PD` and the replica count. Every Raft leader is first moved onto one TiKV, the worst one to lose, which is then killed with 175 of the 400 edits still to come. Nothing is lost, and the run takes 13 s instead of 1.6 s while the other two elect new leaders. A server restarted just as that TiKV dies was back in 8 to 14 s.

### Load on the direct TiKV route

Each writer sends an edit and waits for it to be confirmed, as a busy browser does. These run the orderer directly on TiKV, with no web server and no regeneration, which runs after each commit and holds nothing up. Every edit is checked afterwards: none lost, none doubled. Through the real server, with regeneration keeping up behind them, eight tabs on two servers reach 85 edits/s on one node and 58 on three.

| Run | One node | Three nodes |
| --- | --- | --- |
| 1 project, 8 writers, 1 server | 225 edits/s, 99% in 42 ms | 177 edits/s, 99% in 58 ms |
| 1 project, 8 writers, 2 servers | 125 edits/s, 99% in 409 ms, worst 4.9 s | 94 edits/s, 99% in 1,356 ms, worst 5.4 s |
| 8 projects, 8 writers, 1 server | 286 edits/s, 99% in 37 ms | 130 edits/s, 99% in 202 ms |
| 32 projects, 64 writers, 2 servers | 528 edits/s, 99% in 168 ms | 422 edits/s, 99% in 200 ms |

What these say:

- **One project takes about 200 edits a second on one server,** with every edit confirmed in under 100 ms. That is far above what a model's team produces.
- **The figures move a good deal between runs.** The machine is shared, and TiKV here is capped at the free server's two cores' worth of threads. Treat them as a floor, not a measurement of TiKV.
- **Avoid two servers writing one project.** They fight over its `head` key, and the slowest edits take seconds. Nothing is lost, but it isn't good enough. Before running several web servers for real, each project needs pinning to one server (note K08, open risks). One server is unaffected.

### Not run yet

- **The Oracle server itself,** which needs the owner's account. CI runs the checklist and the install script on fresh ARM Ubuntu machines instead.
- **Real Google sign-in past the redirect,** which needs a Google client. Discovery and the redirect work against Google over real TLS; the rest of the path runs against the stand-in, which signs real tokens.
- **Latency across the internet.** The 200 ms above is on one machine. Over a network, add the round trip.

## Licences

Every dependency is pinned, in [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock) and [tests/browser/package-lock.json](tests/browser/package-lock.json). The kit's and Construct's crates are pinned by the root Cargo.lock. `cargo deny check licenses` enforces [deny.toml](deny.toml) in CI.

| Direct dependency | Used for | Licence |
| --- | --- | --- |
| axum, axum-extra | Web server | MIT |
| openidconnect | Google sign-in | MIT |
| cedar-policy | Access checks | Apache-2.0 |
| tikv-client | TiKV | Apache-2.0 |
| object_store | Files on RustFS, S3 or Google Cloud Storage | MIT OR Apache-2.0 |
| tokio, tracing, tracing-subscriber | Async runtime, logs | MIT |
| base64, clap, flate2, futures, hex, rand, reqwest, serde, serde_json, sha2, thiserror, time, url | Everyday building blocks | MIT OR Apache-2.0 |
| bolero (tests only) | Fuzzing | MIT |
| chrono, rsa, tokio-tungstenite (tests only) | The stand-in sign-in and test clients | MIT OR Apache-2.0; tokio-tungstenite MIT |
| playwright-core (tests only) | Drives real browsers | Apache-2.0 |
| wasm-bindgen | The geometry in the browser | MIT OR Apache-2.0 |
| three.js 0.170.0, from a pinned CDN address, only when the 3D view opens | Draws the shapes | MIT |

No GPL, AGPL or LGPL code. Two licences outside the plain list are allowed, both permissive and both impossible to avoid in Rust:

- **Unicode-3.0:** Unicode's data licence, on `unicode-ident` (needed by every Rust macro) and the `icu_*` crates behind web addresses.
- **Apache-2.0 WITH LLVM-exception:** Apache-2.0 plus a wider permission, on one build tool (`ar_archive_writer`, under Cedar).
