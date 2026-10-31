# The Spare Construct test stack

A small, real server that proves the cloud design for Spare Construct end to end, before anything bigger is built on it. It runs on one free server now and moves to three by changing settings, not code ([deploy/README.md](deploy/README.md)).

What it proves:

- **Browser only.** No model file. The page loads only the pieces in view and keeps them by hash in the browser's cache.
- **The model is pieces in TiKV**, changed only through one ordered edit log per project ([note A05](https://github.com/spareware-co-op/spareware/pull/168)'s design, following Replicache and Figma's published method).
- **Access is rows, checked by Cedar on every request.**
- **The hierarchy:** people, hubs, projects, teams, WIP, Shared and Consumed folders, and bridges between hubs.

The boxes are placeholders. The geometry kernel's shapes take their place later; nothing here touches it.

**The page is the Spare Construct demo.** The server sends [the demo's own file](../../spare-construct/demo/index.html) unchanged and adds [web/connect.js](web/connect.js), so the ribbon, palettes, view tabs and status bar are the real ones and stay in step with the demo. Connected to live data: sign-in, the hub switcher, the project list, WIP, Shared and Consumed folders in the project browser, the plan and 3D views, properties, the Wall and Door tools, Share, Bridge, Take in, and links. Everything else in the demo says it isn't connected yet, rather than showing sample data.

## What's in it

| Part | What it is | Licence |
| --- | --- | --- |
| [src/](src) | The server, in Rust: web pages, JSON API, live channel | MPL-2.0 |
| [web/](web) | The connector that puts live data into the demo page | MPL-2.0 |
| [The demo page](../../spare-construct/demo/index.html), [the ribbon](../../../design/ribbon) | The whole interface, used as they are | MPL-2.0 |
| [policy/](policy) | Who may do what, in Cedar | MPL-2.0 |
| TiKV 8.5.8 | The database (Rust; RocksDB inside) | Apache-2.0 |
| PD 8.5.8 | TiKV's coordinator (Go) | Apache-2.0 |
| RustFS 1.0.4 | The file store, speaking S3 | Apache-2.0 |
| Caddy, on the server | https, with free certificates | Apache-2.0 |

### How data is stored

Every key starts with its scope, so a scan of `p:<project>/` finds a whole project. All keys are in [src/keys.rs](src/keys.rs).

| Key | Holds |
| --- | --- |
| `p:<project>/head` | The last log number. Every commit changes it, so only one writer wins each number |
| `p:<project>/log/<n>` | The edit log: who, when, the intent, the changes and their content ID |
| `p:<project>/record/<hash>` | An element's data, named by its hash |
| `p:<project>/shape/<hash>` | A shape, shared by every element of that size |
| `p:<project>/elem/<model>/<id>` | Which record is an element's current version |
| `p:<project>/area/<cell>/<id>` | Which elements touch a 5 m cell, so only what's in view loads |
| `p:<project>/claim/<model>/<id>` | Who is working on an element |
| `p:<project>/item/<team>/<folder>/<id>` | A package in a Shared or Consumed folder |
| `dir/...` | People, logins, hubs, projects |
| `acl/...` | Who belongs to which hub, project and team, and which hubs permit bridges |
| `auth/...` | Sessions, so a restart signs nobody out |

Package files go to RustFS through the `object_store` crate, so production points at S3 or Google Cloud Storage with one setting.

### The example data

Hubs Company 1, 2 and 3. Anna, Ben and Cara are home in Company 1, Dan in Company 2, Eve in Company 3; Dan and Eve are external users of Company 1. Example model 1 has Anna and Ben, with Dan external; Riverside Flats has Cara, with Eve external. Company 2 holds the bridged side of Example model 1 for Dan's Structure team. Company 1 and 2 permit bridges with each other; Company 3 doesn't, so a bridge there is refused.

Sign-in is one path: OpenID Connect. Locally it talks to [examples/mock_oidc.rs](examples/mock_oidc.rs), on the server to Google, so there's no test back door in the server. There is no sign-up: `STACK_PEOPLE` says which Google accounts sign in as which person.

## Running it locally

TiKV needs Linux: use WSL or any Linux machine, with Rust and a C compiler. Nothing needs root.

```bash
cd projects/kit/test-stack
bash scripts/install-binaries.sh    # TiKV, PD and RustFS into ~/test-stack
bash scripts/run-checklist.sh       # wipes local test data, seeds, runs the checklist
```

To use the page, run `bash scripts/run-browser-test.sh`, open <http://localhost:8080/auth/login> and pick a person. To be two people at once, use two browser profiles.

The real-browser test runs from any machine with Node and Edge or Chrome:

```bash
cd projects/kit/test-stack/tests/browser
npm ci
node two_browsers.mjs
```

## Checklist

CHECKLIST_RESULTS

## Licences

Every dependency is pinned, in [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock) and [tests/browser/package-lock.json](tests/browser/package-lock.json). `cargo deny check licenses` enforces [deny.toml](deny.toml).

| Direct dependency | Used for | Licence |
| --- | --- | --- |
| axum, axum-extra | Web server | MIT |
| openidconnect | Google sign-in | MIT |
| cedar-policy | Access checks | Apache-2.0 |
| tikv-client | TiKV | Apache-2.0 |
| object_store | Files on RustFS, S3 or Google Cloud Storage | MIT OR Apache-2.0 |
| tokio, tracing, tracing-subscriber, bytes | Async runtime, logs, buffers | MIT |
| anyhow, base64, clap, flate2, futures, hex, rand, reqwest, serde, serde_json, sha2, time, url | Everyday building blocks | MIT OR Apache-2.0 |
| chrono, rsa, tokio-tungstenite (tests only) | The stand-in sign-in and test clients | MIT OR Apache-2.0; tokio-tungstenite MIT |
| playwright-core (tests only) | Drives real browsers | Apache-2.0 |
| three.js 0.170.0, from a pinned CDN address, only when the 3D view opens | Draws the boxes | MIT |

No GPL, AGPL or LGPL code. Two licences outside the plain list are allowed, both permissive and both impossible to avoid in Rust:

- **Unicode-3.0:** Unicode's data licence, on `unicode-ident` (needed by every Rust macro) and the `icu_*` crates behind web addresses.
- **Apache-2.0 WITH LLVM-exception:** Apache-2.0 plus a wider permission, on one build tool (`ar_archive_writer`, under Cedar).
