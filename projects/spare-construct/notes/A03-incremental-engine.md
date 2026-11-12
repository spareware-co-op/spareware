# A03: Incremental engine: the Salsa crate or our own

## The question

Regeneration rebuilds only what an edit changed, across every CPU core ([ARCHITECTURE.md](../ARCHITECTURE.md), Regeneration and constraints). The map says how: "incremental computation in the style of Salsa (the engine inside rust-analyzer): cached queries, revision numbers, and early cut-off when a recomputed value comes out unchanged", and its decisions table leans to the Salsa crate itself. The piece asks for a recommendation, Salsa or our own, covering parallel rebuilds and cancelling.

Sources were read on 12 November 2026: Salsa 0.28.5 (tag `salsa-v0.28.5`, commit [`d434f88`](https://github.com/salsa-rs/salsa/tree/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86), released 24 September 2026), comemo 0.5.0 at commit [`5944487`](https://github.com/typst/comemo/tree/5944487f5e9d11949038662df4b307ec56f153b3), the kernel's map (chapters 10, 20, 21 and 23) and this repo's `construct-regen`. A small spike was built and run against Salsa 0.28.5 (below, "The spike"). "Unverified" marks anything not confirmed in source or by measurement.

## What the engine has to do

From Spare Construct's map and the notes already written:

- **Only what changed.** An edit marks its inputs dirty and only what sits downstream is recomputed (Regeneration). The entity store returns "the exact elements and columns it touched" from every transaction, for this piece ([A02](A02-entity-store.md), Recommendation, change tracking).
- **Every core, and the same answer.** A level change touching 2,000 walls uses every core and finishes in under 1 s; a door move redraws within one frame, 16 ms (Performance targets). The answer is bit-identical on any number of threads, natively and in the browser (ground rule 6).
- **Cancellable.** A global edit shows progress and can be cancelled (Regeneration, low level).
- **Server and browser.** People work in the browser ([K08](../../kit/notes/K08-cloud-foundation.md)). The server's regenerator keeps a project's derived shapes in TiKV behind the orderer, and each browser predicts its own edits and compares content IDs with the server's checkpoints ([A05](A05-multi-user.md); `construct-regen`'s `regenerator.rs`). Today that regenerator rebuilds every changed element in log order and says the order and caching wait on this piece ([lib.rs](../crates/construct-regen/src/lib.rs)).
- **Memory.** Under 256 bytes per placed instance, of which the kernel takes about 140 and the store most of the rest (A02, item 5).

From the kernel's map:

- **The program owns the graph.** "The regeneration graph belongs to the program (Spare Construct's engine, in the style of Salsa)." The kernel supplies pure operations, waves inside which items are independent, results published in element order, and early cut-off on equal content hashes ([23.5](../../engines/geometry/architecture/23-performance.md#235-regenerating-many-elements-at-once)).
- **One pool, fixed chunks, ordered results.** The program's per-element work and the kernel's inner work share one re-entrant pool, through `geom-run`'s `Exec` with fixed chunk plans ([23.4](../../engines/geometry/architecture/23-performance.md#234-deterministic-parallelism-one-helper)).
- **Cancelling never unwinds.** Operations poll the caller's `Cancel` at fixed steps and return `Cancelled`, "because WebAssembly builds cannot catch an unwinding panic". A cancelled result is never logged or cached ([23.10](../../engines/geometry/architecture/23-performance.md#2310-budgets-and-cancellation), [10.1](../../engines/geometry/architecture/10-operations.md)).
- **Cache keys are content.** A result is keyed by kernel edition, operation and version, `OpPath` and the canonical bytes of every input ([23.8](../../engines/geometry/architecture/23-performance.md#238-caching-by-content)). The regenerator already stores bodies under their content IDs.
- **In a browser,** a person's cancel button cannot reach a running worker on a page without cross-origin isolation, so today's plan is to end the worker and reload its chunks (23.10).

## Options

| | A. The Salsa crate | B. Our own engine, on Salsa's method | C. comemo |
| --- | --- | --- | --- |
| What it is | Demand-driven queries with memos, revisions, durability and backdating; rust-analyzer and ty use it | Rules over the entity store's columns, dirtied from each transaction's change list and run in waves through `geom-run` | Memoised functions that record the calls they made on tracked inputs and re-check them on the next call |
| Licence | Apache-2.0 or MIT | Ours (Apache-2.0) | Apache-2.0 or MIT |
| Direction | Pull: a query validates its inputs when it is asked for | Push from the change list, then pull for what is in view | Pull, per call |
| Parallel | Clone the database per thread; a thread asking for a query another is computing blocks on it | `Exec` waves, fixed chunks, results in element order (23.4, 23.5) | None of its own |
| Cancelling | Unwinds the stack with a panic payload | The kernel's `Cancel`, polled; a stopped item returns `Cancelled` | None |
| Persisting | Optional serde dump of the whole database | Derived rows in TiKV under content IDs, as the regenerator does now | None |
| Size | 18,766 lines in `src/` | What we write | 1,596 lines in `src/` |

Adapton (demanded computation graphs, Hammer and others, PLDI 2014) and differential dataflow were considered by method only and not read in source: the first is the push-then-pull dirtying that option B uses, and the second is a streaming engine for relational data rather than for rebuilding shapes.

## Evidence

### How Salsa works, in brief

A tracked function's result is a memo: the value, the revision it last changed in (`changed_at`), the lowest durability of its inputs, and the inputs it read ([zalsa_local.rs L502-L526](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/zalsa_local.rs#L502-L526)). Setting an input starts a new revision. Asking for a query in a new revision walks its recorded inputs: if none changed after the memo was verified, the memo stands; otherwise the function reruns, and if its new value equals the old one the memo keeps its old `changed_at`, so nothing that read it reruns (backdating, [backdate.rs L15-L29](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/function/backdate.rs#L15-L29)). Durability levels let a query that read only rarely changing inputs skip the walk when only frequently changing inputs moved ([durability.rs L3-L20](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/durability.rs#L3-L20)). This is the method the map names, and the method option B follows; the question is only whether to take the crate.

### Cancelling

**Salsa cancels by unwinding.** `Cancelled::throw` calls `panic::resume_unwind`, and `Cancelled::catch` is `catch_unwind` ([cancelled.rs L25-L43](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/cancelled.rs#L25-L43)). A write first sets a flag and then waits until every other handle to the database has been dropped ([storage.rs L160-L190](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/storage.rs#L160-L190)); running queries see the flag at their next query call, or when they call `unwind_if_revision_cancelled` themselves, and unwind ([zalsa.rs L301-L309](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/zalsa.rs#L301-L309)). There is also a per-handle `CancellationToken` that ends the same way ([zalsa_local.rs L51-L67](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/zalsa_local.rs#L51-L67)). No public call tells a query "a write is waiting" without unwinding; the nearest is the `DidSetCancellationFlag` event, which a database's event callback could turn into a flag of its own.

**Natively, that works and is quick.** In the spike, three workers building bodies were stopped by a write on the main thread, which waited 0.13 to 0.25 ms for them to drop their handles, over four runs.

**In WebAssembly it ends the instance.** The pinned toolchain, Rust 1.99.0, reports `panic="abort"` for both `wasm32-unknown-unknown` and `wasm32-wasip1` (`rustc --print cfg --target ...`), so `catch_unwind` cannot catch and `resume_unwind` aborts. Salsa 0.28.5 builds for both targets (the spike built each, 411 KB for wasip1), but a cancellation in a browser would kill the worker, which is what 23.10 already plans for the cancel button, and with no shared memory nothing can set the flag mid-query anyway. Whether a stable unwinding WebAssembly build arrives, and when, is unverified (G01 evidence [07](../../kit/notes/G01-evidence/07-persistent-naming.md), question 6, asks the same).

**The kernel's rule meets Salsa's at a seam.** A kernel operation inside a Salsa query that sees its `Cancel` returns `Err(Cancelled)` cooperatively. Salsa would memoise that value unless the query turns it back into an unwind, so every query that calls the kernel would need that conversion, and it only exists natively. 23.10 says a cancelled result is never cached; under Salsa that holds only if every one of those conversions is right.

### Parallel rebuilds

**Salsa parallelises by cloning handles.** A database handle is `Send` but not `Sync` (its query stack is a `RefCell`, [zalsa_local.rs L37-L49](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/zalsa_local.rs#L37-L49)), so each task needs its own clone, made on a thread that holds one; the spike first failed to compile with rayon's `map_init` for that reason. A thread asking for a query another thread is computing blocks until it finishes, or reports a cycle ([runtime.rs L305-L330](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/runtime.rs#L305-L330)). The 0.2x `par_map` helper is gone from 0.28.5; parallelism is the caller's.

**The speed-up is good.** 2,000 bodies of 200,000 integer steps each took 645 to 669 ms on one thread and 227 to 238 ms on three, 2.8 times, over four runs.

**Salsa's IDs depend on scheduling.** Each thread allocates from "the most recent page from this thread" ([zalsa_local.rs L104-L107](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/zalsa_local.rs#L104-L107)). In the spike, 20,000 names interned inside queries on one thread and on three got different `salsa::Id`s for 19,744 of them, and two runs on three threads differed for 19,168 to 19,872. Values stay right, but nothing that feeds the model may be ordered or tie-broken by a Salsa ID, which G01 evidence 07 already says for persistence. The kernel's helper gives ordered results by construction (23.4); with Salsa, ordering would be a review rule over every query.

**Our own engine can use the kernel's helper directly.** 23.5 already defines regeneration as waves (wall bodies, then joins, then openings), each wave's items independent and run through `Exec`, results published in element order. That is a scheduler, not a demand-driven one, and it shares the pool with the kernel's inner work without a second threading model.

### What one edit costs

Salsa is pull-based: a query that read n inputs walks n edges when it is next asked for after any change it might depend on. In the spike, a summary over 1,000,000 walls took 337 to 386 ms the first time and 306 to 399 ms after one wall changed (about 0.3 to 0.4 µs a read), and 1 to 2 µs when nothing changed. For the 230,000-element reference model that is roughly 70 to 90 ms for any query that reads every element (derived), past a 16 ms frame. Durability does not help when the changed input and the summary's inputs have the same durability. The usual answer under Salsa is summaries per chunk, so a summary reads chunk summaries, not elements; option B needs the same structure for its own reasons.

The store already knows exactly what changed (A02). A push step from that change list, walking reverse edges to mark what is downstream, costs what the change touches, not what the summary read. Early cut-off then stops the walk where a recomputed value's content ID is unchanged (23.5). This is Adapton's dirty-then-demand order, and in the terms of "Build systems à la carte" (Mokhov, Mitchell and Peyton Jones, ICFP 2018, [doi:10.1145/3236774](https://doi.org/10.1145/3236774)) a topological scheduler with verifying and constructive traces: verifying traces to decide what to rerun, content-addressed results so a rerun with known inputs is a lookup.

### Memory

The spike measured 56 bytes per input (two `u32` fields) and 128 bytes per memo (a `u64` result with two input reads), from the process's resident memory over 1,000,000 of each. Salsa pins its memo header at four words and a memo at five ([memo.rs L635-L641](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/function/memo.rs#L635-L641)); the rest is the recorded edges and table slots. An element with four derived values (body, cells, display record, tag text) would carry about 500 bytes of memos under Salsa (derived), twice the whole program's per-instance target. LRU eviction sets a memo's value to nothing and keeps the memo, its revisions and its recorded inputs ([memo.rs L90-L97](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/function/memo.rs#L90-L97)), so evicting saves the value's bytes only.

Option B keeps derived state as columns in the store's chunk tables: a revision stamp and a content-ID index per derived value per element, about 8 bytes each, with edges in the relationship tables A02 already plans (estimated; to be measured by the benchmark below). Most edges are implied by rules ("a wall body reads its recipe, type and level") and need not be stored per element; only dynamic reads, such as which walls meet at a junction, are.

### Persisting and paging

Salsa's `persistence` feature serialises and restores the whole database through serde (`as_serialize` and `deserialize`, [database.rs L178-L190](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/src/database.rs#L178-L190); [tests/persistence.rs](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/tests/persistence.rs)). Spare Construct's derived data lives in TiKV per element and per content ID, paged by chunk (A06, K08), and the regenerator keeps it there. Salsa can create new inputs with a shared reference (the spike's `Wall::new(&db, ..)`), so paging a chunk in does not cancel running work, but changing or dropping a paged input is a write that cancels everything and waits for every handle.

### comemo

Typst's comemo memoises a function together with the calls it made on its tracked arguments, and on the next call re-checks those calls' results ([lib.rs L1-L60](https://github.com/typst/comemo/blob/5944487f5e9d11949038662df4b307ec56f153b3/src/lib.rs#L1-L60)). Its caches are process-wide statics ([memoize.rs L14](https://github.com/typst/comemo/blob/5944487f5e9d11949038662df4b307ec56f153b3/src/memoize.rs#L14), [L114](https://github.com/typst/comemo/blob/5944487f5e9d11949038662df4b307ec56f153b3/src/memoize.rs#L114)), it has no revisions, dependency graph, parallel scheduler or cancelling, and validation re-checks every recorded call. That suits a compiler run in one process for one document, not a server holding many projects, so it is a reference for constraint-style validation, not a contender.

### Keeping up

Salsa calls itself "experimental" ([Cargo.toml L9](https://github.com/salsa-rs/salsa/blob/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86/Cargo.toml#L9)) and is pre-1.0 with a FIXME to remove `salsa_unstable` before 1.0 (L73-L74). Eleven breaking minor releases came out between February 2025 (0.18.0) and July 2026 (0.28.0), from the repo's tags. Its dependencies are permissive (hashbrown, parking_lot, crossbeam, boxcar, inventory and others; the spike's lock file), and its strengths are real: cycle handling by fixpoint, a long test suite of cancellation and cycle races across threads (`tests/parallel/`), and use at the scale of rust-analyzer and ty.

### The spike

A program of about 170 lines on Salsa 0.28.5 (default features off, `macros` and `inventory` on) and rayon 1.12.0, built with Rust 1.99.0 in release mode and run four times on a 6-core Intel i7-10750H under Linux, 3 rayon threads. It measured memory per input and memo over 1,000,000 walls, a summary over them before and after one edit, 2,000 costly bodies on 1 and 3 threads, the wait for a write to cancel three workers, and interned IDs on 1 and 3 threads; it also built for both WebAssembly targets. No WebAssembly runtime was installed, so the abort on cancellation there is from the target's panic strategy, not from a run. The spike is not committed; its figures are quoted above.

## Recommendation

1. **Write our own engine on Salsa's method (option B).** This changes the map's lean, which an owner decides. Salsa's method is the right one and the crate is excellent, but four things in this program go against using it: it cancels by unwinding, which aborts in our WebAssembly builds and crosses the kernel's rule that cancelling never unwinds; its IDs and its scheduling are not ordered, while the kernel's helper is; its pull-based revalidation costs what a query read rather than what changed, though the store already hands us the exact change list; and its memos alone would take about twice the per-instance memory target. Wrapping it would mean a conversion at every kernel call, a review rule on ordering, chunk summaries and a second threading model beside `Exec`, on a crate that breaks its API every few months.
2. **Port named methods; invent nothing:**
   - **Revisions, verified-at and changed-at** per derived value, and **backdating**: Salsa's memo header and `backdate_if_appropriate`, with equality by content ID (23.8, 23.5) instead of `PartialEq`.
   - **Durability:** Salsa's levels, mapped as G01 evidence 07 suggests: family and type definitions high, instance parameters low, so a type-only check skips instance edges.
   - **Rules, not ad hoc queries:** each derived value is a rule with a declared shape of inputs (wall body reads recipe, type, level; join reads the uncut bodies at a junction; opening reads host body and cutters), stratified into the waves of 23.5, so the graph has no cycles by construction and a rule that would close one is refused when it is registered. Reads a rule makes beyond its declared ones are recorded as dynamic edges in the store's relationship tables (Salsa's recorded inputs).
   - **Push, then pull:** each transaction's change list marks downstream values dirty through reverse edges (Adapton's dirtying). The server then runs every dirty value in wave order; a browser runs only what is in view and what a view asks for, so one engine serves both.
   - **Waves through `geom-run`:** each wave is one `Exec::map_each` over dirty items in permanent-ID order, with a fixed chunk plan and per-item budgets (23.10), results published in element order. No second pool, no handle clones.
   - **Constructive traces:** a rerun whose inputs' content IDs are already in the content store is a lookup, which the regenerator does today for bodies; the cache key is 23.8's.
3. **Cancelling, natively and in the browser:**
   - An edit that supersedes a running pass sets the pass's `Cancel`. Kernel operations stop at their next poll (measured median about 2 µs through `OpContext::run`, 23.10) and return `Cancelled`; nothing of that item is stored.
   - Items that finished before the cancel are kept when their inputs' content IDs are unchanged by the new edit, which the verifying trace checks. So a cancelled level change keeps the walls it already rebuilt and the edit is not repaid in full.
   - In a browser the engine returns to the worker's event loop between chunks of a wave (one chunk is a fixed number of items), so a cancel message from the page is read there with no shared memory. That avoids ending the worker for the cancel button in most cases; an item still running when the message arrives finishes first. This is a proposal, to be measured against the 16 ms frame.
4. **Use Salsa as the yardstick and the oracle.** The Phase 0 harness implements the same rule set twice, once in the engine and once as Salsa 0.28.5 queries (a dev-dependency only, Apache-2.0 or MIT), and runs seeded edit sequences through both, natively: every derived value must match, and the engine's revalidation cost after one edit must stay below Salsa's. It reports memory per element against the 128-byte memo measured here, a 2,000-wall level change on 1, 2, 3 and 8 threads with identical hashes, a door move against 16 ms, and cancel-to-stop latency. If the engine is not at least as fast as Salsa on that harness, revisit this note.
5. **Can it live in the kit?** Revisions, rules, edges, waves and the change list are not about buildings, and Spare Studio's documents may want the same (*unverified*). I recommend the generic engine sits beside the store A02 put in `kit-store`, and `construct-regen` keeps Spare Construct's rules and its regenerator. An owner decides, because it moves work between crates.

## Open risks

- **New code to get right.** Salsa's test suite covers cancellation races, cycles and eviction across threads that a new engine has to cover again. The differential harness in item 4, and the kernel's replay corpus at 1, 2, 3 and 8 threads, are the guard.
- **Fan-out.** Pushing dirtiness through reverse edges can mark much more than ends up changing, such as every tag on a level after a level edit. Early cut-off stops recomputation, not marking; marking cost is to be measured on the reference model.
- **The browser yield.** Returning to the event loop between chunks adds latency per chunk and may need a smaller chunk plan in the browser than natively. Chunk plans never change results (23.4), so this is a speed question only.
- **Unwinding WebAssembly may arrive.** If a stable unwinding WebAssembly build lands, reason one weakens; the ordering, revalidation and memory reasons stand. *Unverified*: whether and when.
- **Project-level constraints.** Locked dimensions and alignments across elements can form cycles that waves cannot order. They are the solver's clusters (A11), solved as one rule, not cycles in this engine; how big such a cluster may get is open in the map.
- **The spike is small.** Integer work stands in for kernel operations, one machine, four runs. The figures show orders of size, not targets.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [Salsa 0.28.5](https://github.com/salsa-rs/salsa/tree/d434f8805c60ac60dce5f367ca91c5a7cd3e4c86) | Apache-2.0 or MIT | Method, cancelling, parallelism, memory, persistence; spike |
| [comemo 0.5.0](https://github.com/typst/comemo/tree/5944487f5e9d11949038662df4b307ec56f153b3) | Apache-2.0 or MIT | Constrained memoisation, compared |
| [rayon 1.12.0](https://github.com/rayon-rs/rayon) | Apache-2.0 or MIT | Spike threads |
| Mokhov, Mitchell and Peyton Jones, "Build systems à la carte", ICFP 2018, [doi:10.1145/3236774](https://doi.org/10.1145/3236774) | Paper | Scheduler and trace terms |
| Hammer and others, "Adapton: composable, demand-driven incremental computation", PLDI 2014 | Paper (not read in source) | Dirty, then demand |
| Kernel map, chapters [10](../../engines/geometry/architecture/10-operations.md), [21](../../engines/geometry/architecture/21-determinism.md), [23](../../engines/geometry/architecture/23-performance.md) | This repo | Rules the engine must keep |
| [G01 evidence 07](../../kit/notes/G01-evidence/07-persistent-naming.md) | This repo | Earlier reading of Salsa's IDs, durability and cancelling |
| [A02](A02-entity-store.md), [A05](A05-multi-user.md), [A06](A06-project-file.md), [K08](../../kit/notes/K08-cloud-foundation.md) | This repo | Store, sync and storage the engine sits on |
