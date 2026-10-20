# A02: Entity store: our own or bevy_ecs

## The question

The model is a database of elements stored column by column ([ARCHITECTURE.md](../ARCHITECTURE.md), Data model). Should that in-memory store be the bevy_ecs crate used on its own, or a store we write ourselves? The map leans to our own, "because we need chunked storage and permanent IDs that game ECS libraries do not give". The piece asks for a recommendation backed by both codebases, covering chunked storage and permanent IDs.

Sources were read on 20 October 2026: bevy_ecs 0.20.0 (released 8 October 2026), Flecs 4.1.6, hecs 0.11.2 and Bentley's imodel-native at commit `efdc2b3`. "Unverified" marks anything not confirmed in source code or on an official page. No Rust toolchain was available, so the memory figures below are worked out from the struct layouts in the source, not measured.

## What the store has to do

From the map:

- **Millions of elements in memory, scanned in milliseconds** (Data model). A city has to work by the same design (Goals).
- **Chunks:** elements group by building, level and spatial cell. The core keeps recently used chunks in memory and pages the rest from disk (Storage and scale). The project file holds one blob per chunk ([A06 note](https://github.com/spareware-co-op/spareware/blob/piece/A06/projects/spare-construct/notes/A06-project-file.md), in review).
- **Permanent IDs:** 128-bit and time-ordered on disk and over the wire, with a compact 32-bit index in memory (Data model). Permanent IDs belong to `kit-core`.
- **Types stored once; instances store only what differs.** 2,000 doors of one type share one geometry (Data model).
- **Relationships** (hosted by, joined to, bounded by, in room, on level, in group) are their own tables, queryable both ways (Data model).
- **Parameters are typed and many are user-defined:** shared parameters are keyed by GUID, and families define their own (Data model, Families). The set of parameters is only known at run time.
- **Under 256 bytes per placed instance**, plus shared type geometry (Performance targets).
- **Deterministic:** the same operations give the same model on every machine, native or browser (ground rule 6).
- **Parallel:** regeneration uses every core through rayon (Runtime shape).
- **One core, native and WebAssembly** (Runtime shape).

## Options

| | A. bevy_ecs on its own | B. Our own chunked column store | C. Flecs through Rust bindings |
| --- | --- | --- | --- |
| What it is | Bevy's ECS crate without the engine: one `World`, archetype tables, queries | Columns per element shape, inside each chunk; handles mapped to permanent IDs | A C ECS with first-class relationships and inheritance |
| Licence | MIT or Apache-2.0 | Ours (MPL-2.0) | MIT |
| Language | Rust | Rust | C, with third-party Rust bindings |
| Storage unit | One table per archetype for the whole world | One table per element shape per chunk | One table per archetype for the whole world |
| Change tracking | Two ticks per component per entity, always on | Per chunk, table and column, plus the exact change list from each transaction | One counter per table column |
| Relationships | One-to-many, kept in step by hooks | Our own tables, both directions, any cardinality | Pairs, many-to-many, symmetric, optionally non-fragmenting |
| Types and instances | Not built in | Port Flecs's inheritance method | Built in (`IsA` with `Inherit`) |
| Iteration order | Depends on history (swap-remove) | Sorted by permanent ID within each table | Depends on history (*unverified*) |
| Size of the code | bevy_ecs: 120,058 lines of Rust | What we write | 100,675 lines of C in `src/` |

*Line counts include comments and tests.*

[hecs](https://github.com/Ralith/hecs) (MIT or Apache-2.0, 9,552 lines) was also read as the smallest archetype ECS in Rust. It has no relationships, and its columns are keyed by Rust `TypeId` ([archetype.rs](https://github.com/Ralith/hecs/blob/v0.11.2/src/archetype.rs#L558-L564)), so families can't add columns at run time. It is a reference to read, not a contender.

Bentley's iModel engine ([imodel-native](https://github.com/iTwin/imodel-native), Apache-2.0) is the one open, BIM-scale element store found. It keeps elements as separate objects in a most-recently-used cache over SQLite, 2,000 elements by default ([DgnElements.cpp](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/iModelPlatform/DgnCore/DgnElements.cpp#L15-L30)). That's a different design from the map's column store, so it is used below only for permanent IDs.

## Evidence

### Memory per element

**bevy_ecs** keeps, for every live entity:

- `EntityMeta`: the generation, an `Option<EntityLocation>` (archetype, archetype row, table, table row) and the spawn tick, 24 bytes ([entity/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/mod.rs#L1181-L1194), [EntityLocation](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/mod.rs#L1212-L1233)).
- An `ArchetypeEntity` (entity plus table row) in its archetype, 16 bytes with 8-byte alignment ([archetype.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/archetype.rs#L326-L329), [L388-L395](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/archetype.rs#L388-L395)).
- Its `Entity` again in its table's entity list, 8 bytes ([table/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/table/mod.rs#L204-L207)).
- For every component, an `added_ticks` and a `changed_ticks` entry beside the data, 4 bytes each ([column.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/table/column.rs#L27-L33)). `Column::with_capacity` always allocates both, and I found no way to turn them off.

Take a door instance with eight columns and 128 bytes of data: permanent ID 16, type 8, host 8, level 8, placement (f64 position and rotation) 32, bounding box (f64) 48, category and flags 4, override list offset 4.

| | bevy_ecs | Our own (sketch) |
| --- | --- | --- |
| Data | 128 | 128 |
| Per-element bookkeeping | 48 | about 20 (generation and location 16, row-to-handle 4) |
| Change tracking | 64 (8 columns × 8 bytes) | 0 per element |
| **Total** | **240 bytes** | **about 148 bytes** |
| 230,000 elements (the reference model) | 55 MB | 34 MB |
| 10,000,000 elements (a city) | 2.4 GB | 1.5 GB |

*Computed from field sizes, not measured. Both also need the permanent-ID lookup map, which costs the same in both, and neither includes allocator slack.*

bevy_ecs fits the 256-byte target for this door, with 16 bytes to spare. Each extra column costs another 8 bytes in ticks. For the reference building either option fits easily. The gap matters for a city, and in a browser tab, where the map already warns about a hard memory ceiling (Runtime shape).

**Parameters can't be one component each in bevy_ecs.** A sparse-set component keeps a `SparseArray` indexed by entity index, which grows to the highest index that has ever held it, at 4 bytes a slot ([sparse_set.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/sparse_set.rs#L24-L27), [insert](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/sparse_set.rs#L100-L106)). 500 parameter kinds over 230,000 elements would cost 500 × 230,000 × 4 bytes = 460 MB before storing a single value. As table components instead, every new mix of overridden parameters makes a new archetype. Either way, parameters have to be data inside a column, not a component each. The same holds for our own store. bevy's usual one-component-per-property style just pushes the wrong way here.

### Chunks and paging

**bevy_ecs has one table per archetype for the whole world.** Relationships are ordinary components holding an `Entity`, so they don't split tables by target. Splitting by target isn't there yet: a field in the archetype code is "currently unused, but planned to be used to implement a component index to improve performance of fragmenting relations" ([archetype.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/archetype.rs#L792)). Elements from every chunk sit mixed in the same tables.

- **Paging a chunk out** means despawning each of its elements. Each despawn swap-removes a row from every column of its table, moves the last row into the gap and updates that element's location ([table/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/table/mod.rs#L228-L255)), then runs any relationship hooks. Columns grow by `realloc`, and I found no code that shrinks a table after despawns (searched `storage/`; *unverified* beyond that), so memory may not come back.
- **Paging a chunk in** means spawning each element (`spawn_batch`, [world/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/world/mod.rs#L1319)).
- **Can we make bevy_ecs chunk?** One `World` per chunk would do it: dropping a `World` frees its memory at once. But component IDs are given out per `World` as components register ([world/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/world/mod.rs#L615)), an `Entity` means nothing in another `World`, and relationships and hooks stop at the chunk edge. Joins and hosting cross cell edges all the time (a wall in one cell joins a wall in the next), so we would rebuild those parts ourselves, outside bevy.

**Flecs can split tables by a pair's target.** Adding `(InCell, cell_7)` puts the entity in a table of its own for that cell, because "relationship pairs are two elements encoded into a single 64-bit id" and go into the archetype like any component ([Relationships.md](https://github.com/SanderMertens/flecs/blob/v4.1.6/docs/Relationships.md#relationship-performance)). The cost is fragmentation: more tables to create and for queries to match. Flecs says it is "optimized for supporting large amounts (hundreds of thousands) of tables" ([Fragmentation](https://github.com/SanderMertens/flecs/blob/v4.1.6/docs/Relationships.md#fragmentation)). Those tables are still part of one world, so paging a cell out still deletes entities one by one.

**Our own store makes the chunk the storage unit.** Each chunk owns its tables, one per element shape present in it. Paging out frees the chunk's column buffers in one go. Paging in decodes the chunk blob straight into column buffers, with no per-element insertion.

### Permanent IDs and handles

All three ECSs use the same in-memory ID: a dense index plus a generation, so a stale handle is caught rather than pointing at the wrong element.

- bevy_ecs: 32-bit `EntityIndex` and 32-bit `EntityGeneration` packed into 8 bytes ([entity/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/mod.rs#L424-L433)).
- hecs: 32-bit id and 32-bit generation ([entities.rs](https://github.com/Ralith/hecs/blob/v0.11.2/src/entities.rs#L18-L21)).
- Flecs: 32-bit index, 16-bit generation and flag bits in a 64-bit id ([api_defines.h](https://github.com/SanderMertens/flecs/blob/v4.1.6/include/flecs/private/api_defines.h#L371-L375)).

None of them has permanent IDs. Each gives a way to work with them:

- bevy_ecs lets the caller choose entity indices: `spawn_at` and `despawn_no_free` "allow you to skip Bevy's default entity allocator" ([entity/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/mod.rs#L779-L780), [world/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/world/mod.rs#L1078)). Its `MapEntities` and `EntityMapper` traits rewrite every stored `Entity` when a scene loads ([map_entities.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/map_entities.rs#L54), [L231](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/entity/map_entities.rs#L231)).
- Flecs has entity ranges: "new entity identifiers will fall within the specified [min, max] interval, including recycled identifiers" ([flecs.h](https://github.com/SanderMertens/flecs/blob/v4.1.6/include/flecs.h#L2700-L2730)).
- iModels use 64-bit element IDs: "Top 24 bits are BeBriefcaseId. Lower 40 bits are local id". The source calls this "a more efficient strategy for id values than using true 128-bit GUIDs" ([BeSQLite.h](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/BeSQLite/PublicAPI/BeSQLite/BeSQLite.h#L289-L301)). Briefcase IDs are handed out centrally, below 2^24 ([L261](https://github.com/iTwin/imodel-native/blob/efdc2b30777c24802964b0fefaeb01d4f202d018/iModelCore/BeSQLite/PublicAPI/BeSQLite/BeSQLite.h#L261)). This is the map's "client ID plus counter" option, proven at BIM scale.

So permanent IDs work the same way in either option. The permanent ID is a column, one hash map gives permanent ID to handle, references are saved as permanent IDs and mapped to handles on load (bevy's `MapEntities` method), and handles never leave the process. This does not decide between A and B. The map's reason for its lean holds for chunks, not for IDs.

### Types and instances

The map wants type data stored once and instances to store only what differs. That is Flecs's `IsA` relationship with the `Inherit` trait: "Inherited components are not copied to the instance, and are only stored once in memory. Operations such as `get` and `has`, and queries will automatically lookup inheritable components by following the `IsA` relationship". Adding the component to the instance overrides it ([ComponentTraits.md](https://github.com/SanderMertens/flecs/blob/v4.1.6/docs/ComponentTraits.md#inherit)). bevy_ecs has no equivalent. We would build it on top in option A, and port it in option B.

### Relationships

- bevy_ecs: "A `Relationship` may only be one-to-many (or one-to-one)". The target keeps a list of its sources, updated by component hooks ([relationship/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/relationship/mod.rs#L32-L36)). That fits hosted by, on level and in room. It doesn't fit joined to and bounded by, which are many-to-many, so those would be our own tables anyway.
- Flecs covers all of them: any number of targets, `Exclusive` (at most one target), `Symmetric` ("when a relationship `(R, Y)` is added to entity `X`, the relationship `(R, X)` will be added to entity `Y`"), and `DontFragment`, which keeps a relationship out of the table type so sparse relationships don't multiply tables ([ComponentTraits.md](https://github.com/SanderMertens/flecs/blob/v4.1.6/docs/ComponentTraits.md#dontfragment-trait)). `DontFragment` doesn't yet support component inheritance, `Or` or `Optional` queries, or JSON serialisation (same page).

### Change tracking

- bevy_ecs stamps a 32-bit tick on every component write and clamps old ticks every 518,400,000 ticks ([change_detection/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/change_detection/mod.rs#L21-L26)). That suits a game, where systems run every frame and ask "what changed since I last ran". 0.20 adds an opt-in summary tick per column, so contiguous queries can skip whole tables ([component/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/component/mod.rs#L503-L524)).
- Flecs keeps one counter per table column, bumped on write ([table.h](https://github.com/SanderMertens/flecs/blob/v4.1.6/src/storage/table.h#L161), [table.c](https://github.com/SanderMertens/flecs/blob/v4.1.6/src/storage/table.c#L1395-L1425)), with nothing per entity.
- hecs has no built-in tracking. Its `ChangeTracker` is opt-in and works by keeping a previous copy of a component and comparing it ([change_tracker.rs](https://github.com/Ralith/hecs/blob/v0.11.2/src/change_tracker.rs#L7-L14)).

In Spare Construct every change is an operation in a transaction (ground rule 4), so each transaction already knows exactly which elements and columns it touched. Regeneration (piece A03) needs that list, not a tick per value. A per-table counter in Flecs's style is enough to let queries skip untouched tables.

### Determinism

- bevy_ecs hashes with a fixed seed (`FixedState::with_seed`, [hash.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_platform/src/hash.rs#L18-L19)), so hashing is the same on every run.
- Its iteration order is tables in the order they were created, then rows in table order, and a removal moves the last row into the gap ([table/mod.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/storage/table/mod.rs#L228-L255)). Two machines that paged chunks in a different order iterate in a different order. Floating-point addition depends on order, so a sum over a query can differ in the last bit and break ground rule 6. The only defence is to sort before every order-sensitive loop and never forget.
- Our own store can make order a property of the data: keep rows sorted by permanent ID within each table. Inserts and removals then shift rows instead of swapping, which costs time in proportion to the table. Tables are per chunk, so they stay small. This is a cost to measure, not a known result.

### Parallel work and the browser

- bevy_ecs's `par_iter` runs on its own `ComputeTaskPool` ([par_iter.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/query/par_iter.rs#L105)), a second thread pool beside the rayon pool the map plans for regeneration. The new `contiguous_iter` hands out whole column slices ([state.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/query/state.rs#L1485)), which rayon could split instead.
- bevy_ecs builds for WebAssembly, since Bevy ships browser games (*unverified* for 0.20 specifically). A store of plain `Vec`s builds for WebAssembly with nothing extra.
- Columns added at run time are possible in bevy_ecs through `ComponentDescriptor` and `insert_by_id`, but the insert is `unsafe` ([world_mut.rs](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/src/world/entity_access/world_mut.rs#L1121)).

### Keeping up

- bevy_ecs is pre-1.0. 0.19.0 came out on 18 June 2026 and 0.20.0 on 8 October 2026. The v0.20.0 tag carries 74 migration guides for that release's breaking changes, across the whole engine ([_release-content/migration-guides](https://github.com/bevyengine/bevy/tree/v0.20.0/_release-content/migration-guides)). bevy_ecs needs Rust 1.95 ([Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_ecs/Cargo.toml)).
- The project is large and healthy: 49,160 stars and 3,490 open issues on 20 October 2026. The parts this note needs most (storage, entities, archetypes) are 18,535 of bevy_ecs's 120,058 lines. The rest is queries, the world API, the scheduler, systems, observers, messages and reflection.
- The store is the most central code in the core. Every element, regeneration, sync and the API go through it. Tying it to another project's four-monthly breaking changes is a real cost. Owning 18,000 lines of our own is also a cost. Ground rule 1 says to build from scratch "where it decides speed or ownership", and the data model is first on its list.

## Recommendation

1. **Write our own store (option B).** This confirms the map's lean, for a sharper reason. Permanent IDs work in either option. The deciding gaps in bevy_ecs are that it has no chunk storage unit, it adds 8 bytes of ticks per component per element, its iteration order depends on history, and it has no type-to-instance inheritance. Working around all four means building most of option B anyway, on top of a crate whose API breaks every few months.
2. **Port named methods; invent nothing:**
   - **Tables and columns:** one table per element shape, each column a plain typed array, rows in step across columns. This is bevy_ecs's `Table` and `Column` and hecs's `Archetype`. Both are MIT or Apache-2.0, so code may be reused with credit if that turns out simpler than writing it.
   - **Chunk as storage unit:** each chunk owns its tables, so chunks page in and out whole. A chunk blob decodes straight into its columns.
   - **Handles:** 32-bit index plus 32-bit generation, as all three ECSs do, mapped to (chunk, table, row). One fixed-seed hash map gives permanent ID to handle. Handles are never saved or sent; references are saved as permanent IDs and mapped on load (bevy's `MapEntities` method).
   - **Types and instances:** Flecs's `IsA` with `Inherit`. A type is an element; an instance's parameter read looks in its own override list, then its type.
   - **Parameters:** values in a per-chunk parameter table keyed by parameter definition, never one column per parameter.
   - **Relationships:** a table per relationship kind per chunk, with source and target sorted both ways. Flecs's `Exclusive` and `Symmetric` semantics, stored without splitting tables (Flecs's `DontFragment`). A target in another chunk is held by permanent ID until that chunk loads.
   - **Change tracking:** each transaction returns the exact elements and columns it touched, for A03, plus a counter per chunk, table and column (Flecs's `dirty_state`) so queries skip what didn't change. Nothing per element.
   - **Order:** rows sorted by permanent ID within each table. Never iterate a hash map for model logic.
   - **Parallel work:** rayon over (chunk, table) slices.
3. **Use bevy_ecs as the yardstick.** The Phase 0 benchmark harness builds the same model in bevy_ecs 0.20 and in our store: the reference model at 230,000 elements and a generated city at 10,000,000. It measures memory per element, a full scan, a filtered scan, paging a chunk in and out, lookup by permanent ID, and a determinism replay, native and in the browser. If our store is not at least as fast on scans, revisit this note.
4. **Can it live in the kit?** Nothing in the store is specific to buildings: chunks, handles, columns, relationships and change lists. The map gives "chunked storage, the spatial index and paging" to `kit-store`. I recommend the generic store goes there, and `construct-model` holds only Spare Construct's schema (categories, types, parameters). Spare Studio's document model (piece S02) may want the same store (*unverified*). An owner decides, because this moves work between crates.

## Open risks

- **The figures are worked out, not measured.** No Rust toolchain was available. Repeat them in Rust in the Phase 0 harness.
- **Sorted rows cost time.** An insert or removal in the middle of a table shifts rows. That's fine for per-chunk tables of thousands of rows, but a bulk import should sort once and write whole columns.
- **References to unloaded chunks.** A wall joined to a wall in a cell that isn't loaded holds a permanent ID, not a handle. Every relationship read has to handle "not loaded": load on demand, or treat it as context. Settle this in Design, together with the A06 chunk layout.
- **Many small tables.** 2,000 chunks × 40 element shapes is up to 80,000 tables. Flecs shows hundreds of thousands can work, but our query planner has to skip empty and unmatched tables cheaply.
- **Permanent ID width.** The map says 128-bit. iModels prove 64-bit (24-bit client, 40-bit counter) at BIM scale and would save 8 bytes per element and per reference, but they need a server to hand out the client ID before offline work. This is `kit-core`'s decision, and either works with this store.
- **18,000 lines to own.** Our store will be smaller than bevy_ecs because it skips the scheduler, observers and messages, but storage still has to be fuzzed and tested to the same standard. Budget for it.
- **Flecs moves fast too.** Its `DontFragment` limits ("temporary", in its own words) may lift, and bevy_ecs plans fragmenting relations. Re-read both before the Design stage closes.

## Sources

Every source is linked where it is used above. All were read on 20 October 2026, at the versions named in the question. Bevy and hecs are MIT or Apache-2.0, Flecs is MIT, and imodel-native is Apache-2.0. No code from any source is used in this note.
