# The Spare Construct cloud stack

The foundation of Spare Construct's cloud, running now as a test server. It is built to become the real thing: this folder is the thin program that wires the kit's crates into one server. It runs on one free server today and moves to three by changing settings, not code ([deploy/README.md](deploy/README.md)). Decisions are in note [K08](../notes/K08-cloud-foundation.md).

What it does:

- **Browser only.** No model file. The page loads only the pieces in view and keeps them by hash in the browser's cache.
- **The model is pieces in TiKV**, changed only through one ordered edit log per project ([note A05](https://github.com/spareware-co-op/spareware/pull/168)'s design, following Replicache and Figma's published method).
- **Access is rows, checked by Cedar on every request.** The web layer can't reach the store: every read and write goes through [`Service`](src/service.rs), which asks first.
- **Every write goes through a log.** The directory has one too, so seed data and administrators use the same doors as everyone. The exceptions are in K08, with why.
- **The hierarchy:** people, hubs, projects, teams, WIP, Shared and Consumed folders, and bridges between hubs.

The boxes are placeholders. The geometry kernel's shapes take their place later; nothing here touches it.

**The page is the Spare Construct demo.** The server sends [the demo's own file](../../spare-construct/demo/index.html) unchanged and adds [web/connect.js](web/connect.js), so the ribbon, palettes, view tabs and status bar are the real ones and stay in step with the demo. Connected to live data: sign-in, the hub switcher, the project list, WIP, Shared and Consumed folders in the project browser, the plan and 3D views, properties, the Wall and Door tools, Share, Bridge, Take in, and links. Everything else in the demo says it isn't connected yet, rather than showing sample data.

## Where everything is

| Part | Holds | Tested by |
| --- | --- | --- |
| [`kit-core`](../crates/kit-core) | Permanent IDs. No dependencies, because the geometry kernel reads this crate | Unit tests |
| [`kit-ops`](../crates/kit-ops) | The log entry, the format kept forever | Unit tests, recorded samples, fuzzing |
| [`kit-store`](../crates/kit-store) | The key layout ([FORMAT.md](../crates/kit-store/FORMAT.md)), versioned rows, the storage traits, a test store with injected faults | Unit tests, fuzzing |
| [`kit-sync`](../crates/kit-sync) | The orderer, claims, replay, the live channel's rules | [Failure tests](../crates/kit-sync/tests/failures.rs), fuzzing under random faults |
| [accounts](../services/accounts) | The directory and its log, Cedar checks, sign-in, sessions | [Directory tests](../services/accounts/tests/directory.rs) |
| [store](../services/store) | The traits on TiKV and `object_store` | [Promises](../services/store/tests/promises.rs) and [load](../services/store/tests/load.rs), on real TiKV |
| This folder | The web layer, the box commands, the folder commands, seed data, deployment | [Rules in-memory](tests/service.rs), [the checklist](tests/checklist.rs), [real browsers](tests/browser/two_browsers.mjs) |

| Outside program | What it is | Licence |
| --- | --- | --- |
| TiKV 8.5.8 | The database (Rust; RocksDB inside) | Apache-2.0 |
| PD 8.5.8 | TiKV's coordinator (Go) | Apache-2.0 |
| RustFS 1.0.4 | The file store, speaking S3 | Apache-2.0 |
| Caddy, on the server | https, with free certificates | Apache-2.0 |

The stored format is published and read forever: [FORMAT.md](../crates/kit-store/FORMAT.md).

### The example data

Hubs Company 1, 2 and 3. Anna, Ben and Cara are home in Company 1, Dan in Company 2, Eve in Company 3; Dan and Eve are external users of Company 1. Example model 1 has Anna and Ben, with Dan external; Riverside Flats has Cara, with Eve external. Company 2 holds the bridged side of Example model 1 for Dan's Structure team. Company 1 and 2 permit bridges with each other; Company 3 doesn't, so a bridge there is refused.

Sign-in is one path: OpenID Connect. Tests talk to a [stand-in provider](tests/support/mock_oidc.rs), which nothing but a test can reach; the server talks to Google. There's no test back door in the server. There is no sign-up: `STACK_PEOPLE` says which Google accounts sign in as which person.

## Running it

TiKV needs Linux: use WSL or any Linux machine, with Rust and a C compiler. Nothing needs root.

```bash
cd projects/kit/test-stack
cargo test --workspace              # everything that needs no database
bash scripts/install-binaries.sh    # TiKV, PD and RustFS into ~/test-stack
bash scripts/run-checklist.sh       # wipes local test data, then promises, load, checklist
```

`STACK_NODES=3 STACK_PD=127.0.0.1:2379,127.0.0.1:2389,127.0.0.1:2399 bash scripts/run-checklist.sh` runs the same on three copies of PD and TiKV, and kills one TiKV mid-edit.

To use the page, run `bash scripts/run-browser-test.sh`, open <http://localhost:8080/auth/login> and pick a person. To be two people at once, use two browser profiles.

The real-browser test runs from any machine with Node and Edge or Chrome:

```bash
cd projects/kit/test-stack/tests/browser
npm ci
node two_browsers.mjs
```

The kit's crates are tested from the repository root: `cargo test -p kit-core -p kit-ops -p kit-store -p kit-sync`.

Run `cargo fmt` here, never `cargo fmt --all`: it follows the path dependencies back into the root workspace and reformats every other project.

## Acceptance checklist

This is the owner's test of the whole stack, and it stays the test as the stack grows. Last run on 31 October 2026, in release builds, on a 12-core Linux machine with TiKV sized for the free server. [tests/checklist.rs](tests/checklist.rs) runs it against the real programs, and [tests/browser](tests/browser/two_browsers.mjs) runs it again in real Edge browsers with real mouse drags, on the demo page.

| # | Check | Result |
| --- | --- | --- |
| 1 | Anna logs in and lands in Company 1 | Passes. Someone not on the server is refused, and nothing opens without signing in |
| 2 | Dan switches between Company 2 and Company 1 | Passes. Each hub shows only the projects he is on |
| 3 | Two browsers editing Example model 1 each see the other's edit in under 200 ms | Passes. Real browsers: median 24 ms, worst 50 ms. Test clients, 200 edits each way: median 20 ms, worst 29 ms |
| 4 | Dan can't see the Architecture team's WIP; Arch v12 can be shared, bridged and consumed | Passes. A bridge to Company 3 is refused because only one hub permits it. What Dan consumes has the same hash as what Anna shared |
| 5 | Killing and restarting the server mid-edit loses nothing | Passes. 400 edits through 2 hard kills: exactly 400 log entries, and the log replays to exactly what is stored. The real page reconnects by itself mid-drag |
| 6 | A backup restores | Passes. After wiping TiKV and RustFS, every key and file came back byte for byte, the logs replay, and work carries on |

### Moving to three machines, by settings alone

The whole checklist passes on three copies of PD and TiKV, with three copies of every write, changing only `STACK_NODES`, `STACK_PD` and the replica count. One TiKV is killed with 175 of the 400 edits still to come: nothing is lost, and the run takes 4.9 s instead of 1.3 s while the other two take over.

### Load on the direct TiKV route

Each writer sends an edit and waits for it to be confirmed, as a busy browser does. These run the orderer directly on TiKV, with no web server. Every edit is checked afterwards: none lost, none doubled.

| Run | One node | Three nodes |
| --- | --- | --- |
| 1 project, 8 writers, 1 server | 174 edits/s, 99% in 97 ms | 211 edits/s, 99% in 43 ms |
| 1 project, 8 writers, 2 servers | 100 edits/s, 99% in 1,043 ms, worst 3.8 s | 101 edits/s, 99% in 780 ms, worst 4.3 s |
| 8 projects, 8 writers, 1 server | 236 edits/s, 99% in 91 ms | 174 edits/s, 99% in 131 ms |
| 32 projects, 64 writers, 2 servers | 277 edits/s, 99% in 576 ms | 420 edits/s, 99% in 347 ms |

What these say:

- **One project takes about 200 edits a second on one server,** with every edit confirmed in under 100 ms. That is far above what a model's team produces.
- **The figures move a good deal between runs.** The machine is shared, and TiKV here is capped at the free server's two cores' worth of threads. Treat them as a floor, not a measurement of TiKV.
- **Avoid two servers writing one project.** They fight over its `head` key, and the slowest edits take seconds. Nothing is lost, but it isn't good enough. Before running several web servers for real, each project needs pinning to one server (note K08, open risks). One server is unaffected.

### Not run yet

- **The Oracle server itself,** which needs the owner's account. CI runs the checklist and the install script on fresh ARM Ubuntu machines instead.
- **Real Google sign-in past the redirect,** which needs a Google client. Discovery and the redirect work against Google over real TLS; the rest of the path runs against the stand-in, which signs real tokens.
- **Latency across the internet.** The 200 ms above is on one machine. Over a network, add the round trip.

## Licences

Every dependency is pinned, in [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock) and [tests/browser/package-lock.json](tests/browser/package-lock.json). The kit's crates are pinned by the root Cargo.lock. `cargo deny check licenses` enforces [deny.toml](deny.toml) in CI.

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
| three.js 0.170.0, from a pinned CDN address, only when the 3D view opens | Draws the boxes | MIT |

No GPL, AGPL or LGPL code. Two licences outside the plain list are allowed, both permissive and both impossible to avoid in Rust:

- **Unicode-3.0:** Unicode's data licence, on `unicode-ident` (needed by every Rust macro) and the `icu_*` crates behind web addresses.
- **Apache-2.0 WITH LLVM-exception:** Apache-2.0 plus a wider permission, on one build tool (`ar_archive_writer`, under Cedar).
