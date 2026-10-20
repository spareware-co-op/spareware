# A05: Multi-user: server-ordered operation log or CRDT

## The question

How do several people edit one Spare Construct model at once, and offline? The map leans to a server that holds the authoritative operation log and decides the order. Clients send operations and the server broadcasts them. A text CRDT is used only for free text ([ARCHITECTURE.md](../ARCHITECTURE.md), Multi-user and history). It leaves one question to discuss: "Server first, or peer-to-peer with a relay?" The piece asks for a recommendation covering offline work, conflicts and determinism.

Sources were read on 20 October 2026:

| Source | Licence | Version read |
| --- | --- | --- |
| [Replicache and Zero](https://github.com/rocicorp/mono) (Rocicorp) | Apache-2.0 | commit `80f6a7a` |
| [Automerge](https://github.com/automerge/automerge) | MIT | commit `0ee5e2c` |
| [Loro](https://github.com/loro-dev/loro) | MIT | commit `136b944` |
| Bentley's [imodel-native](https://github.com/iTwin/imodel-native) | Apache-2.0 | commit `efdc2b3` |
| Bentley's [iTwin.js](https://github.com/iTwin/itwinjs-core) | MIT | commit `487cb99` |
| [Dolt](https://github.com/dolthub/dolt) | Apache-2.0 | commit `d1f8f8e` |

Two papers and one article were also read. Figma's server is closed source, so its method comes from Figma's own [article](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/) and is marked as such. "Unverified" marks anything not confirmed in source code or on an official page. No Rust toolchain was available, so nothing here is measured. The timings are worked out from the map's own budgets.

## What multi-user has to do

From the map and the notes before this one:

- **Every change is an operation** (ground rule 4). The interface, AI, plugins, undo and sync all take that one path.
- **Another user sees an edit in under 200 ms on a LAN** (Performance targets).
- **Claims, not files:** editing an element takes a light, automatic claim on it, and conflicts are caught per element (Multi-user and history).
- **Offline:** work offline on a branch, then merge back with an element-by-element conflict review (Multi-user and history). Phones and tablets on site keep only the chunks for their area and send their operations when they are back online (Construction).
- **History and branches:** who, when and why for every change; any past state; design options tried as branches (Multi-user and history).
- **Deterministic:** the same operations give the same model on every machine (ground rule 6). Only operations travel between users, never geometry.
- **Permissions** per project, discipline and element category. The kit owns the permission checks (Sharing with consultants).
- **What the store gives us:** each transaction returns the exact elements and columns it touched. Permanent IDs are saved and sent; in-memory handles never leave the process ([A02 note](https://github.com/spareware-co-op/spareware/blob/piece/A02/projects/spare-construct/notes/A02-entity-store.md), in review).
- **What the file gives us:** an `oplog` table holding sequence, operation ID, author, time and operation bytes, always replayed in sequence order ([A06 note](https://github.com/spareware-co-op/spareware/blob/piece/A06/projects/spare-construct/notes/A06-project-file.md), in review).

Out of scope: sharing between firms. That uses packages, which are signed pointers into a team's log plus content-addressed chunks (Sharing with consultants). This note covers live work inside one team or project.

## Options

| | A. Server-ordered operation log | B. CRDT for the whole model, peer-to-peer with a relay |
| --- | --- | --- |
| Who decides the order | One orderer per project gives each operation a sequence number | Every replica, by operation ID (counter, then actor) |
| Two people write the same value | Detected; the later write is re-run on the new state or turned back | Both kept; one is picked "arbitrarily" but the same way everywhere (Automerge) |
| Model rules (one host, no cycles, unique numbers) | Checked by the orderer inside each command | Each rule needs its own algorithm, and some can't be kept without coordination |
| Offline work | Pending commands rebased on reconnect; branches for long work | Built in: merge at any time |
| Cost of a late operation | A client re-runs only its own unconfirmed commands | Undo every later operation, apply, redo (move-op) |
| Permissions | Checked before an operation enters the log | Every peer must check, after the operation already exists |
| Needs a server | An orderer, which any desktop can run | A relay only |
| Rust code available | Our own, following Replicache's method | Automerge, Loro and Yrs, all MIT |

Inside option A there is a second choice: what does a log entry carry? It can carry the command, meaning the intent (`MoveWall` by 1 m). Or it can carry the changes the command made, meaning the state (wall 7's start point is now this). The open precedents split on this, so the evidence covers both.

## Evidence

### Figma: one server decides, CRDT ideas inside

Figma's multiplayer is the best-known server-ordered design editor. Its source is closed, so everything here comes from Evan Wallace's [article](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/) (16 October 2019). None of it is checked in source.

- The server is the central authority, so Figma doesn't use true CRDTs, only ideas from them.
- Values are per property, and the last value the server receives wins. The server's order replaces timestamps.
- A client applies its own change at once. While that change is unconfirmed, the client ignores incoming server values for the same property, so the screen doesn't flicker.
- A parent link is a property of the child. The server rejects a parent change that would make a cycle.
- Offline: on reconnect, the client downloads a fresh copy and reapplies its offline edits. Objects are created offline, so IDs are made on the client, never by the server.

### Replicache and Zero: predict on the client, decide on the server

Rocicorp's Replicache (client) and Zero (server) are open source under Apache-2.0. They implement the "predict, then rebase" method in full.

- **Up go commands.** A mutation is a mutator name plus arguments. The client runs it at once against local state as a pending commit. The server runs the same mutator against its own database.
- **Exactly once.** The server keeps a `lastMutationID` per client and increments it in the same transaction as the mutation. A lower ID is ignored as already processed, and a higher one is refused as out of order ([mutagen.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/zero-cache/src/services/mutagen/mutagen.ts#L462-L490)). A retried send after a dropped connection therefore never applies twice.
- **A refused command is still consumed.** If a mutator throws on the server, Zero runs it again in "error mode", which only advances `lastMutationID`, and returns the error to the app ([mutagen.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/zero-cache/src/services/mutagen/mutagen.ts#L215-L250)). A bad command can't block the queue.
- **Down comes state.** A pull returns a patch of `put`, `del` and `clear` operations on keys ([patch-operation.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/replicache/src/patch-operation.ts#L37-L49)), plus each client's new `lastMutationID`. The client checks that neither goes backwards and applies the patch to its last server snapshot ([pull.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/replicache/src/sync/pull.ts#L203-L300)).
- **Rebase only your own work.** The client then replays only its own pending mutations whose IDs are above what the server has confirmed ([pull.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/replicache/src/sync/pull.ts#L354-L395)). It never runs another client's mutators.
- **Replays can fail.** A check that passed the first time can fail against newer state. The client then drops only its prediction. The mutation stays pending, and the server decides ([rebase.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/replicache/src/db/rebase.ts#L120-L140)).
- **Commands are code, so versions matter.** A mutator name must not be removed within one schema version, because pending mutations must still replay ([rebase.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/replicache/src/db/rebase.ts#L56-L66)). Zero checks the schema version in the same step as the mutation ID (`checkSchemaVersionAndIncrementLastMutationID`, [mutagen.ts](https://github.com/rocicorp/mono/blob/80f6a7aafbaf9efc70aefba12f3dcd7abc737bc2/packages/zero-cache/src/services/mutagen/mutagen.ts#L462)).

So intent goes up and state comes down. Only the server's run of a command counts, and a client's own prediction is thrown away once the server's answer arrives. A slightly different build on another client can't corrupt anyone's model, because it never re-runs their commands.

### iModels: the BIM-scale precedent

Bentley's iModels are the one open engine found that syncs a BIM-scale model between many editors. A02 also found them to be the one open BIM-scale element store. Changesets carry a single index per iModel (`ChangesetIndexAndId`, [TxnManager.ts](https://github.com/iTwin/itwinjs-core/blob/487cb99636d31b6424dacd865755516ab07b6dfe/core/backend/src/TxnManager.ts#L13)), so the order is decided centrally, by Bentley's hub service.

- **State, not intent.** Local transactions are stored as SQLite row changesets. A pull reverses every local transaction ([TxnManager.cpp](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/iModelPlatform/DgnCore/TxnManager.cpp#L4439)), applies the incoming changesets, then reinstates each local transaction in turn as row changes ([L4575](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/iModelPlatform/DgnCore/TxnManager.cpp#L4575-L4618), [L4699](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/iModelPlatform/DgnCore/TxnManager.cpp#L4699-L4730)). The stages are named Merging and Rebasing ([TxnManager.h](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/iModelPlatform/PublicAPI/DgnPlatform/TxnManager.h#L408-L412)).
- **Row-level rules, many special cases.** iTwin.js's default conflict rules ([TxnManager.ts](https://github.com/iTwin/itwinjs-core/blob/487cb99636d31b6424dacd865755516ab07b6dfe/core/backend/src/TxnManager.ts#L1143-L1220)):
  - A local update whose "before" values no longer match: the local change replaces the row, so the last writer wins for the whole row.
  - A row deleted by someone else: the local change is skipped.
  - A unique-constraint clash: the local change is skipped.
  - A primary-key insert clash or a foreign-key clash: the whole rebase is aborted.
  - Before those rules run, several table-specific exceptions apply.
- **Intent added later, as an escape hatch.** Replaying rows can leave derived data wrong. So iTwin.js lets an app skip reinstating a transaction and `recompute` it instead (`RebaseHandler`, marked alpha, [TxnManager.ts](https://github.com/iTwin/itwinjs-core/blob/487cb99636d31b6424dacd865755516ab07b6dfe/core/backend/src/TxnManager.ts#L1736-L1758), used at [L576-L583](https://github.com/iTwin/itwinjs-core/blob/487cb99636d31b6424dacd865755516ab07b6dfe/core/backend/src/TxnManager.ts#L576-L583)).
- **Locks.** Editing an element needs its exclusive lock, which needs shared locks on everything that owns it. Locks come from a lock server, all or nothing, and are released on push ([LockControl.ts](https://github.com/iTwin/itwinjs-core/blob/487cb99636d31b6424dacd865755516ab07b6dfe/core/backend/src/LockControl.ts#L60-L77)). They must be taken while online.

The lesson: replaying state with the last writer winning per row needs special cases, and iModels are now adding intent replay as an option. Spare Construct's operations are commands from day one, so we can start where iModels are heading.

### CRDTs: Automerge, Loro and the tree move

- **Automerge.** An operation ID is a counter plus an actor, compared counter first, then actor ([types.rs](https://github.com/automerge/automerge/blob/0ee5e2c5395adfe29ed62d5d73496410183f2a85/rust/automerge/src/types.rs#L431-L449)). When two people write the same key, both values are kept. `get` returns one "arbitrarily chosen" value, chosen the same way on every node, and `get_all` returns all of them ([read.rs](https://github.com/automerge/automerge/blob/0ee5e2c5395adfe29ed62d5d73496410183f2a85/rust/automerge/src/read.rs#L255-L298)). Nothing is lost, but the app has to go looking. By default, one wall height silently wins.
- **Loro.** In its movable tree, a move that would create a cycle is skipped when updates merge ([tree_state.rs](https://github.com/loro-dev/loro/blob/136b944f2501b16eb82ef1c51541627958199d75/crates/loro-internal/src/state/tree_state.rs#L1076)). The person who made that move finds it undone.
- **The tree move.** Kleppmann, Mulligan, Gomes and Beresford give a formally verified CRDT that keeps a tree valid without coordination ([A highly-available move operation for replicated trees](https://martin.kleppmann.com/papers/move-op.pdf), IEEE TPDS 33(7), 2021). A replica that receives an operation older than some it has already applied undoes every later operation, applies the old one, then redoes the rest. The result is as if every operation had arrived in timestamp order. The authors' fast implementation saturated one core at 5,700 operations a second, averaging 200 undos and redos per remote operation at peak. The log can only be trimmed once every known replica has passed a timestamp ("causal stability").
- **Which rules need coordination.** Bailis and co-authors give a test for which database rules can be kept with no coordination ([Coordination Avoidance in Database Systems](https://arxiv.org/abs/1402.2237), PVLDB 8(3), 2014, Table 2):

  | Rule | Kept without coordination? |
  | --- | --- |
  | Uniqueness, choosing a specific value | No |
  | Uniqueness, choosing some value | Yes |
  | Foreign key, on insert | Yes |
  | Foreign key, on delete | No |
  | Foreign key, on cascading delete | Yes |

  In Spare Construct terms:
  - A door or sheet number someone picks is "a specific value", so it needs coordination.
  - Deleting a wall while someone else adds a door to it is "delete under a foreign key". It needs coordination, unless the delete cascades, which would silently delete the other person's door.

What this means for a model: a CRDT keeps each rule only with an algorithm built for that rule. Spare Construct has many rules. A door has one host, and that host must exist and be a wall. Room boundaries close. Joins are mutual. Numbers are unique. Sketches are solved by the constraint solver. Undo and redo in the move-op style would also rerun regeneration for every undone and redone operation. And with no orderer, every peer must check permissions on operations that already exist on other peers.

**Where CRDTs fit:** free text inside a field, such as notes, comments and specification text, where merging characters is the right answer. Spare Office needs the same for live co-editing (piece OF03), so the kit should choose one text CRDT for both. Automerge, Loro and Yrs are all MIT-licensed Rust.

### Dolt: merging branches cell by cell

Dolt is a SQL database with branches. Its merge is the method for the map's "merge back with an element-by-element conflict review".

- **Three-way, by column.** Each changed row is compared against the common base. If the two sides changed different columns of a row, both changes are kept. If they changed the same column to different values, that is a conflict ([merge_prolly_rows.go](https://github.com/dolthub/dolt/blob/d1f8f8e763864d26e4b0753647c97f283f06e404/go/libraries/doltcore/merge/merge_prolly_rows.go#L2044-L2048), [L2200-L2262](https://github.com/dolthub/dolt/blob/d1f8f8e763864d26e4b0753647c97f283f06e404/go/libraries/doltcore/merge/merge_prolly_rows.go#L2200-L2262)).
- **The same result in either direction.** When both sides wrote equal values in different bytes, the higher byte string is kept, "deterministic regardless of the merge direction" ([L2250-L2256](https://github.com/dolthub/dolt/blob/d1f8f8e763864d26e4b0753647c97f283f06e404/go/libraries/doltcore/merge/merge_prolly_rows.go#L2250-L2256)).
- **Derived columns are recomputed, not merged.** Generated columns are updated after the merge ([L2260](https://github.com/dolthub/dolt/blob/d1f8f8e763864d26e4b0753647c97f283f06e404/go/libraries/doltcore/merge/merge_prolly_rows.go#L2260)). Our geometry, areas and tag text are the same kind of value.
- **Broken rules become data.** Foreign-key violations found after a merge are recorded for a person to resolve ([violations_fk_prolly.go](https://github.com/dolthub/dolt/blob/d1f8f8e763864d26e4b0753647c97f283f06e404/go/libraries/doltcore/merge/violations_fk_prolly.go#L35-L44)). That matches the map's warnings stored as data.

### Determinism

- **With an orderer**, one total order exists and the orderer's state is the truth. The map's "replay and get an identical result" then becomes a check, not something correctness depends on. Each log entry can carry the content hashes of the chunks it changed. Chunks are already content-addressed (Sharing with consultants, Speckle's method). A client compares hashes, and on a mismatch it fetches the orderer's chunk and files a bug report through the kit's bug reports.
- **With a CRDT**, every replica must run identical merge code on every operation forever. There is no reference copy to heal from, and no one to say which replica is right.

### The 200 ms budget

| Step | Budget |
| --- | --- |
| Run the command on the orderer (a local edit) | 16 ms (the map's one-frame budget) |
| LAN round trip (*typical, not measured*) | about 1 ms |
| Apply the changes and regenerate on the other client | 16 ms |
| **Total** | **about 35 ms**, against a 200 ms target |

The risk is a global edit. If the orderer had to finish a 1 s level-height change before ordering the next command, everyone would wait a second. The orderer should commit and order the changed inputs at once, and leave regeneration to the incremental engine. The next command then waits only for the derived values it actually reads (piece A03).

## Recommendation

1. **Use a server-ordered operation log (option A).** This confirms the map's lean. It is the only option that checks Spare Construct's model rules and permissions before an operation exists. It also keeps a reference copy for determinism, and it costs one orderer.

2. **Can we run without a central server? Yes.** The orderer is a role, not a machine: one process per project that numbers operations. It can run on co-op servers, a firm's own server, or inside the desktop app for a LAN session or a sole practitioner. "Peer-to-peer with a relay" is then the same protocol, with one peer elected as orderer. That answers the map's open question without a second sync design.

3. **Follow Replicache's method: commands up, changes down.**
   - **What a log entry holds:**
     - the sequence number, set by the orderer
     - the client ID and the client's own counter (Zero's `lastMutationID`, for exactly-once)
     - author, time, and the sequence the client had seen (its base)
     - the command name and arguments: the intent, kept for history, "why", undo and AI audit
     - the input changes the command made: element, column, before and after (the A02 change list)
     - the hashes of the chunks it changed
     - the core version

     This extends A06's `oplog` columns.
   - **The orderer runs every command** against the latest state, inside a transaction, with the kit's permission check first. A command that fails is still consumed (Zero's error mode). The author sees why.
   - **Other clients apply the input changes as data**, then regenerate derived values locally. They never run anyone else's command. So geometry still never travels, and a client with a slightly different build of a command still applies everyone's work correctly.
   - **A client re-runs only its own unconfirmed commands** on top of each new state (rebase). A command that now fails loses its prediction and joins a short "turned back" list with the orderer's reason.
   - **The core version is pinned per project.** A client on another version can read, but can only send after it updates (Zero's schema-version check).

4. **Catch conflicts in three layers, and never let the last writer silently win the same value.**
   - **Claims.** Editing an element takes a short, leased claim on it. Others see who holds it, and the orderer turns back their writes to it until the claim is released or expires. Before going offline, a person can take long claims on a level or an area (the iModel lock method), so their offline work merges cleanly. Lease lengths are set in Design.
   - **Stale writes, column by column.** Each input change carries its before-value. If another author changed the same element and column after the command's base, that command is turned back and shown as a conflict. Changes to different columns of one element both stand (Dolt's cell-wise merge). Commands declared as relative, such as "move by" or "add to list", are re-run instead of checked, because they commute. The command schema is generated from Rust types (Headless API), so this declaration lives there.
   - **Model rules inside commands.** If a host was deleted first, adding a door to it fails with a clear reason; it doesn't cascade. Unique numbers chosen by a person are checked by the orderer, because Bailis shows they need coordination. Soft problems, such as overlaps or an unenclosed room, become warnings stored as data, as Dolt records violations. Derived values never conflict: they are regenerated.

5. **Two kinds of offline work.**
   - **Short (minutes to a day):** queue the commands and rebase them on reconnect, with the turned-back list for review.
   - **Long or deliberate:** a branch, meaning a fork of the log at one sequence number. Merging is three-way by element and column, following Dolt. Derived values are regenerated after the merge, and conflicts and violations are reviewed element by element. The merge lands on the main log as one entry, and the branch's entries stay in history.
   - **Design-option branches** use the same machinery.
   - **Phones on site** hold only their area's chunks and pre-claim that area.
   - The app suggests switching from a queue to a branch when the queue grows long or the first conflict appears. The thresholds are set in Design.
   - IDs for elements created offline are permanent IDs made on the client, as Figma does. Their width is `kit-core`'s decision (A02's open risk on 64 against 128 bits).

6. **Free text only:** a text CRDT inside long text fields. Its operations ride inside commands in the same log, so order and history stay one. Choose it once for the kit together with Spare Office (OF03), from Automerge, Loro or Yrs.

7. **Determinism is checked, and it heals.** The orderer's state is the truth. Clients verify chunk hashes after applying each entry. On a mismatch, the client fetches the chunk and reports a bug. The map's determinism test, the same log replayed native and in the browser to bit-identical models, stays in CI, now using recorded server logs as fixtures.

8. **Put the generic protocol in the kit.** The log, ordering, counters, rebase, claims, branches, three-way merge and hash checks are all generic, so they belong in `kit-sync` and `kit-ops`. The kit map already says "Server-ordered operation log; deterministic replay". Spare Construct supplies its commands, its column metadata (absolute or relative) and its model rules. An owner decides, because this moves work between crates.

9. **Benchmarks for the harness** (Phase 4, multi-user sync):
   - edit-to-other-user time on a LAN, against the 200 ms target
   - orderer throughput with 50 simulated users
   - rebase time for 1,000 pending commands
   - merging a branch with 10,000 changed elements
   - hash mismatches in a long native-and-browser fuzz run (target: zero)

## Open risks

- **Long commands can block the orderer.** This depends on A03 letting the orderer commit inputs and leave regeneration lazy. Measure it with the level-height benchmark, with other users editing during the change.
- **Rebase cost grows with the queue.** Re-running a day's offline commands could be slow, which is why long work goes to a branch. The threshold needs measuring.
- **Every command must declare its before-values, and whether it is relative.** Generating this from the command schema keeps it honest, but a wrong declaration either hides a conflict or invents one. Fuzz conflicting command pairs.
- **Claims can strand work.** Someone who goes offline holding claims blocks others. Claims need expiry, a visible owner, and an override for a project lead, recorded in the log.
- **One orderer per project is a single point of failure.** People can keep working offline while it is down. Handing the orderer role to another machine from the latest log needs a takeover protocol, to be designed (*not researched here*).
- **Big imports.** An IFC import of 100,000 elements makes a huge input-change list. Large results should travel as content-addressed chunks referenced from the log entry, not inline.
- **Core upgrades mid-project.** Old log entries must still replay for history after the core changes. Version every command and keep old command readers, as Replicache warns about removed mutators.
- **Figma's method comes from an article, not source.** The Bailis rules come from a paper's table, not code.
- **Nothing is measured.** No Rust toolchain was available. Every timing above is a budget, to be proved in the Phase 0 and Phase 4 harness.

## Sources

Every source is linked where it is used above. All were read on 20 October 2026, at the commits named in the question. Licences: Replicache and Zero (rocicorp/mono), imodel-native and Dolt are Apache-2.0; iTwin.js, Automerge and Loro are MIT. Papers:

- Kleppmann, Mulligan, Gomes, Beresford, "A highly-available move operation for replicated trees", IEEE Transactions on Parallel and Distributed Systems 33(7), 2021 ([PDF](https://martin.kleppmann.com/papers/move-op.pdf)).
- Bailis, Fekete, Franklin, Ghodsi, Hellerstein, Stoica, "Coordination Avoidance in Database Systems", PVLDB 8(3), 2014 ([arXiv](https://arxiv.org/abs/1402.2237)).

Article: Evan Wallace, "How Figma's multiplayer technology works", Figma blog, 16 October 2019 ([link](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)).

No code from any source is used in this note.
