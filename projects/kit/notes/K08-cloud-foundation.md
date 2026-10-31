# K08: Cloud foundation for browser programs

*Piece K08, Spare Kit. Written 2026-10-31. Records what the owner decided in the Spare Construct storage brainstorm of 30 and 31 October 2026, and how the kit builds it. The owner's direction: "the stack will turn into the real thing so should be built properly while we test".*

## The question

How does a browser program such as Spare Construct store its data on the one co-op cloud, so that many firms' people can edit one model at once, access is enforced everywhere, and nothing is lost? It has to run on one free server now and on three paid ones later without code changes.

## What the owner decided

| Decision | Choice | Why |
| --- | --- | --- |
| Where people work | Browser only, like Forma or ACC. No local model file: the browser holds what's in view and a cache of pieces already viewed | One place for the truth |
| Hosting | One co-op cloud holds every firm's hub and project | No per-firm servers to run |
| Database | TiKV | Rust, Apache-2.0, many concurrent writers, three copies of every write. SQLite is ruled out: it has one writer at a time |
| The source of truth | The edit log. A model is pieces under keys, changed only through an ordered log, per project | Note A05's design |
| Access | Rows, checked by Cedar on every request | Rules in one readable file, never scattered through code |
| Files | S3-compatible storage: RustFS for now | Moves to S3 or Google Cloud Storage with a setting |
| Cost | Start free: Oracle Always Free for the test server | Pay only when firms pay |

## Options for how to build it

**1. One big server program.** Fast to write, and how the first test server began. It can't be reused by another program, can't be tested without TiKV running, and the format ends up defined by whatever the code happens to do.

**2. The kit's crates, with the database behind a trait.** The log, the key format and the orderer sit in `kit-ops`, `kit-store` and `kit-sync`, which depend on no database. TiKV is one implementation of a small trait. Failure tests and fuzzing then run in plain `cargo test`, on any machine, with faults injected on demand.

**3. Run TiDB, the SQL layer over TiKV, and write SQL.** Well travelled, and it brings GC, backup tooling and online schema changes. It adds a second large Go service to a 12 GB server and hides the key layout the design depends on, which the sketch shows as a tree of pieces.

## Recommendation

Option 2. The layers, top to bottom:

| Part | Where | Holds |
| --- | --- | --- |
| The program | `projects/spare-construct/server` | The web server, the live channel, seed data and deployment |
| The program's rules | `projects/spare-construct/crates` | The element model, commands, hubs, teams, folders, packages, Cedar's rules and the regenerator |
| Accounts | `projects/kit/services/accounts` | People and their commands, Cedar as an answer to `Decider`, sessions, OpenID Connect |
| TiKV and file backend | `projects/kit/services/store` | The `kit-store` traits on TiKV and `object_store` |
| `kit-sync` | root workspace | The orderer, claims, replay and the live channel's rules |
| `kit-store` | root workspace | The generic key layout, versioned rows, the storage traits, an in-memory store for tests |
| `kit-ops` | root workspace | The log entry and its encoding |
| `kit-core` | root workspace | Permanent IDs |

### Why the backend sits outside the root workspace

- **`kit-core` stays free of dependencies.** The geometry kernel's facade depends on it and the kernel's merge gate reads its whole graph, so a dependency there, even an optional one, breaks the kernel's rules. It holds the ID rules and no `serde`.
- **`tikv-client`, `cedar-policy` and `object_store` can't join the root workspace.** Cargo resolves optional requirements too, and the kernel's fuzz tooling pins `indexmap`, so these would fight over a lockfile the kernel depends on. They live in a workspace of their own at `projects/kit/services`, and the server in another, each with its own `Cargo.lock`.

### What the kit knows, and what the program knows

The kit knows nothing about buildings. Its logs, keys and live channel talk about documents and pieces; a program adds its own tables under the same roots, and publishes them. So:

- **The server belongs to the program.** A kit crate may not depend on a program, and a server wires a program's commands to the kit, so it lives at `projects/spare-construct/server`.
- **Commands and access rules are in `construct-api`, behind a `cloud` feature.** That crate is compiled into the browser, and `kit-accounts` is too heavy to go with it, so Construct asks `kit_ops::Decider` and the server hands it Cedar.
- **The directory log runs two sets of commands.** People are the kit's, hubs and teams are Construct's. Only the server sees both crates, so that is where they meet.

### Shapes are derived

Chapters 20 and 24 of the kernel's architecture and notes A02, A05 and A06 set the rules the server follows while boxes stand in for the kernel:

- One interface, `construct_model::geometry::Geometry`. A recipe goes in; a body, bounds, named faces and display meshes come out. Nothing above it reads a shape.
- A recipe is an input and a built shape is a derived record named by its content ID. Edits write inputs only and never build, so an edit costs the same however hard the shape is.
- Regeneration runs after the orderer commits, and is safe on any server: one cursor row per project lets one pass win. A failed build is model state, the same on every machine.
- The server and the browser both build through the interface, and A05's checkpoint compares body content IDs. A checkpoint is a log entry, so every browser gets it in order with the edits it covers, whichever server made it.
- A06's record layout and rules hold (one record per shape, every index rebuildable, large meshes apart), on TiKV and file storage rather than its SQLite.
- A document records its edition. A server or browser without that edition doesn't build it, because it would give different bytes under the same recipe.

### Why every write goes through a log

The kit map says "every admin action is an operation in the log", and A05 says the same of model changes. So there are two kinds of log and no third way in:

- each project has one, for model edits, packages, bridges and links;
- the directory has one, for people, hubs, membership and bridge permissions.

The first entry of a new directory creates its first administrator, and it is refused once the log isn't empty. Everything after that needs Cedar's yes. The seed data goes through the same doors.

Three things are deliberately outside the logs:

- **Sign-in state** (`auth/`): sessions are secrets and expire. A log kept forever is the wrong home for them.
- **Claims' expiry:** a claim ends by the clock, with no entry. Taking and releasing claims are entries.
- **Derived shapes:** bodies, displays, built rows and the area index are rebuilt from the log, so they aren't in it. Deleting them loses nothing.
- **Operator tools:** `backup`, `restore`, `digest`, `verify`, `regenerate` and `collect` run on the server with the database's own credentials. They are maintenance of the store, not a path for users; `restore` refuses a store that holds anything.

### The store format is published

From this piece on, the key layout and log entries are a Spareware format under [EXTENDING.md](../../../EXTENDING.md), section 3: a version in the data, a written specification ([kit-store's FORMAT.md](../crates/kit-store/FORMAT.md), with [Construct's tables](../../spare-construct/crates/construct-model/FORMAT.md) on top), a reader library, and recorded samples that every later version must still read. We may stop writing an old version. We never stop reading it.

## Evidence

Everything here was measured or read, on 31 October 2026:

- **`tikv-client` 0.4.0** (Apache-2.0): the [source](https://github.com/tikv/client-rust), read for transactions, snapshots, `gc` and errors. Its `CheckLevel` panics by default when a transaction is dropped, so a store must set it. An error from `commit` can mean the commit happened (`UndeterminedError`), so a retry has to be safe. That is why each browser tab's edits carry a counter.
- **TiKV has no garbage collector of its own.** TiDB normally tells it when old versions can go. Without a call to `gc`, every rewrite of a key stays on disk forever. The stack makes that call itself and tests it.
- **PD and TiKV stop on a hang-up signal, even under `nohup`.** Found running them from a script; `setsid` or a service manager is needed.
- **Optimistic commits on one `head` key don't scale across servers.** Two servers writing one project conflict on nearly every commit. Retrying at once with a growing random wait helped, and the worst cases are still seconds. One server has no such problem. The figures are in the [server's README](../../spare-construct/server/README.md).
- **The whole stack passes the owner's checklist,** on one node and on three with one killed mid-edit.
- **Method followed:** Replicache's commands up, changes down, and Figma's server order, as read for [note A05](https://github.com/spareware-co-op/spareware/pull/168).

## Open risks

- **The direct `tikv-client` route is less travelled than TiDB.** The client's last release was February 2026, and it pulls `tonic` 0.10, which is old. The load test in CI exists to catch trouble here. If the client stalls, TiKV's gRPC interface can be reached by another client without changing the format.
- **One `head` key per project caps a project's commit rate** at roughly one commit per round trip: about 200 edits a second measured, with batching. That is far above a real team's rate, and a fix wouldn't change the format.
- **Two servers writing one project stall for seconds.** Nothing is lost, but before several web servers run for real, each project must be served by one of them at a time: a lease row in the store and a load balancer that follows it. The test server runs one, so this waits.
- **Names stay in the log.** A log kept forever holds people's names, in `person.add` entries. Emails are stored only as hashes, but "members can delete their data" needs a way to redact a name from a log. It needs its own design, and it isn't done.
- **Backups are logical,** read into memory in one go. Past a few gigabytes, TiKV's own `br` tool should take over. Nothing about the format depends on this.
- **Oracle's free tier can reclaim an idle server,** and it isn't a promise for real work.
- **Regeneration rebuilds every changed element, in log order, in one transaction.** A03's dependency graph replaces that. The transaction grows with a batch, and a large rebuild could need smaller ones. The layout doesn't change.
- **Moving a document to another edition has no command yet.** Until it does, a new kernel edition can edit only new documents, and old ones are viewers of what they stored.
