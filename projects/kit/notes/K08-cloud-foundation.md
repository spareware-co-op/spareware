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

**1. One big server program.** Fast to write, and how the first test stack began. It can't be reused by another program, can't be tested without TiKV running, and the format ends up defined by whatever the code happens to do.

**2. The kit's crates, with the database behind a trait.** The log, the key format and the orderer sit in `kit-ops`, `kit-store` and `kit-sync`, which depend on no database. TiKV is one implementation of a small trait. Failure tests and fuzzing then run in plain `cargo test`, on any machine, with faults injected on demand.

**3. Run TiDB, the SQL layer over TiKV, and write SQL.** Well travelled, and it brings GC, backup tooling and online schema changes. It adds a second large Go service to a 12 GB server and hides the key layout the design depends on, which the sketch shows as a tree of pieces.

## Recommendation

Option 2. The layers, top to bottom:

| Part | Where | Holds |
| --- | --- | --- |
| The program | `projects/kit/test-stack` | The web server, the box commands, the folder commands, seed data and deployment |
| Accounts | `projects/kit/services/accounts` | Directory rows and commands, Cedar checks, sessions, OpenID Connect |
| TiKV and file backend | `projects/kit/services/store` | The `kit-store` traits on TiKV and `object_store` |
| `kit-sync` | root workspace | The orderer, claims, replay and the live channel's rules |
| `kit-store` | root workspace | The key layout, versioned rows, the storage traits, an in-memory store for tests |
| `kit-ops` | root workspace | The log entry and its encoding |
| `kit-core` | root workspace | Permanent IDs |

### Why the backend sits outside the root workspace

- **`kit-core` stays free of dependencies.** The geometry kernel's facade depends on it and the kernel's merge gate reads its whole graph, so a dependency there, even an optional one, breaks the kernel's rules. It holds the ID rules and no `serde`.
- **`tikv-client`, `cedar-policy` and `object_store` can't join the root workspace.** Cargo resolves optional requirements too, and the kernel's fuzz tooling pins `indexmap`, so these would fight over a lockfile the kernel depends on. They live in a separate workspace at `projects/kit/test-stack`, with its own `Cargo.lock`.

### Why every write goes through a log

The kit map says "every admin action is an operation in the log", and A05 says the same of model changes. So there are two kinds of log and no third way in:

- each project has one, for model edits, packages, bridges and links;
- the directory has one, for people, hubs, membership and bridge permissions.

The first entry of a new directory creates its first administrator, and it is refused once the log isn't empty. Everything after that needs Cedar's yes. The seed data goes through the same doors.

Three things are deliberately outside the logs:

- **Sign-in state** (`auth/`): sessions are secrets and expire. A log kept forever is the wrong home for them.
- **Claims' expiry:** a claim ends by the clock, with no entry. Taking and releasing claims are entries.
- **Operator tools:** `backup`, `restore`, `digest`, `verify` and `collect` run on the server with the database's own credentials. They are maintenance of the store, not a path for users; `restore` refuses a store that holds anything.

### The store format is published

From this piece on, the key layout and log entries are a Spareware format under [EXTENDING.md](../../../EXTENDING.md), section 3: a version in the data, a written specification ([kit-store's FORMAT.md](../crates/kit-store/FORMAT.md)), a reader library, and recorded samples that every later version must still read. We may stop writing an old version. We never stop reading it.

## Evidence

Everything here was measured or read, on 31 October 2026:

- **`tikv-client` 0.4.0** (Apache-2.0): the [source](https://github.com/tikv/client-rust), read for transactions, snapshots, `gc` and errors. Its `CheckLevel` panics by default when a transaction is dropped, so a store must set it. An error from `commit` can mean the commit happened (`UndeterminedError`), so a retry has to be safe. That is why each browser tab's edits carry a counter.
- **TiKV has no garbage collector of its own.** TiDB normally tells it when old versions can go. Without a call to `gc`, every rewrite of a key stays on disk forever. The stack makes that call itself and tests it.
- **PD and TiKV stop on a hang-up signal, even under `nohup`.** Found running them from a script; `setsid` or a service manager is needed.
- **Optimistic commits on one `head` key don't scale across servers.** Two servers writing one project conflicted on every commit: 117 edits a second, worst 1.75 s. See the load test in the [test stack's README](../test-stack/README.md) for the numbers after the fix.
- **Method followed:** Replicache's commands up, changes down, and Figma's server order, as read for [note A05](https://github.com/spareware-co-op/spareware/pull/168).

## Open risks

- **The direct `tikv-client` route is less travelled than TiDB.** The client's last release was February 2026, and it pulls `tonic` 0.10, which is old. The load test in CI exists to catch trouble here. If the client stalls, TiKV's gRPC interface can be reached by another client without changing the format.
- **One `head` key per project caps a project's commit rate** at roughly one commit per round trip. Batching lifts edits per commit. This fits a project's real edit rate, and a fix wouldn't change the format.
- **Backups are logical,** read into memory in one go. Past a few gigabytes, TiKV's own `br` tool should take over. Nothing about the format depends on this.
- **Oracle's free tier can reclaim an idle server,** and it isn't a promise for real work.
