# A21: Project locks: from the start, or families first

**Recommendation in one line:** families first, as the map leans, but with the model shaped for locks from the start: one-way references to grids, levels and faces now, two-way locks between elements as the next release's piece, built on the kernel's datum systems (kernel chapter 15.15). This confirms the map's lean and narrows it.

## The question

Revit lets a person lock a dimension or an alignment between two elements in a project, so moving one moves the other. Spare Construct's map asks "How much constraint solving at project level (Revit-style locked dimensions and alignments), versus only inside families?" ([ARCHITECTURE.md](../ARCHITECTURE.md), Regeneration and constraints, Discuss) and leans "Families first, project locks later" (Open decisions, Project-level constraints). The features list has the work as [CON-A3-F2-P3](../FEATURES.md) (project locks, large) waiting on this decision, [CON-A3-F2-P4](../FEATURES.md). The board asks for a recommendation that starts from kernel chapter 15.15 and covers scale, how locks are logged and what waits on the kernel.

Sources were read on 13 November 2026 in this repository: the kernel map's chapters [15](../../engines/geometry/architecture/15-sketches.md) and [20](../../engines/geometry/architecture/20-naming-history.md), notes [A05](A05-multi-user.md) and A11 (#793, in review), and the code of `construct-model` and `construct-api`. How Revit's locks behave is described here as its users know them; it was not checked against Autodesk's documentation and is marked *unverified* where it matters.

## The options

| | A. Locks from the start | B. Families first, locks later | C. Never: one-way references only |
| --- | --- | --- | --- |
| What a person gets at the first release (Gate D, a council set for a house) | Locked dimensions and alignments between any elements | Constraints inside family sketches; elements follow grids, levels and host faces one way | The same as B, for good |
| Kernel work it needs before Gate D | The sketch solver (G216 to G231) at project scale | The sketch solver at family scale | The same |
| Effect on multi-user | An edit can move elements other people hold | None until locks arrive | None |
| Familiar to Revit users | Yes | Partly, then yes | No |

## The evidence

**The kernel already has a design for locks, and it is the program's job.** Chapter 15.15: locks between elements "are a datum system that the program builds". Spare Construct turns each locked element's driving datums (location-line ends, reference planes, grid lines; levels as scalars) into sketch entities, one system per plan level or elevation, each lock into a constraint, and writes the result back "as element edits in one logged operation. The kernel solves; it still knows no walls." The solver is written for any number of freedoms and any variable kind (15.5, 15.15), so a project system reuses the family solver rather than needing a second one. Nothing in the kernel stands in the way of option A or B; the difference is when Spare Construct builds its side.

**Scale is decomposed, but not measured.** The solver splits a system into components that do not touch, and "Components keep a lock between two walls a two-wall problem" (15.15). Within a component, the cluster search is skipped above a size N, "Until it is measured, N is 2,000 entities" (15.5). A plan level of a large building could hold thousands of locked datums in one component (a grid every wall is aligned to joins them all). A11 lists this as untested: the kernel's benchmarks cover sketches, not a project's locks.

**References already give most of what locks are used for, one way.** An element stores references to parts of other elements' shapes by persistent name ([`element.rs`](../crates/construct-model/src/element.rs), `Reference`), with `Want::Carrier` as the default for hosting, "so an opening cut through a face doesn't break what's hosted on it". The kernel resolves them without guessing and keeps an unresolved one as model state (20.4). A door already moves with its wall this way. A wall whose location line refers to a grid line, or a floor whose edge refers to a wall face, follows it the same way: regeneration re-reads the reference, no solver needed. What one-way references cannot do is what makes locks locks: moving either end, equal spacing kept between three walls, and a dimension that drives rather than reports (*unverified* as a list of what Revit users rely on most).

**Locks and claims pull against each other.** Under the server-ordered log, editing an element takes a short claim, and "the orderer turns back their writes to it until the claim is released or expires" (A05, recommendation 6; the server's `element.claim`). A lock means one person's edit writes to elements nobody claimed, possibly ones another person holds. Option A has to settle that before Gate D: refuse the edit, take every locked element's claim with it, or split the lock. Option B can settle it when locks are built, with the claims code already in use.

**How a lock would be logged.** Following 15.15 and A05, a lock is a record of its own, written by a logged operation: the two (or more) references by persistent name, the constraint kind and value, and the plan level or elevation whose datum system it joins. An edit that moves a locked element is one operation whose result includes every element the solve moved, with before and after values, so undo, replay and checkpoints work as for any edit, and the solve reads no clock and gives the same bits everywhere (15.12, A05 recommendation 7). A lock that cannot hold is model state, a warning with the constraints the solver names (15.8), never a silent drop (15.18).

## Recommendation

1. **Families first for Gate D.** Constraints inside family sketches only (A07's recipe, solved by the kernel's solver, CON-A3-F2-P1). No two-way locks between elements before the first release.
2. **One-way references now, for what locks are mostly used for.** Location lines that follow a grid line, bases and tops that follow levels, edges that follow a host face, all through `Reference` with persistent names, resolved in regeneration. They need no solver and no change to claims, and they are the datums a later lock will join.
3. **Shape the data for locks now, so adding them changes no format.** Keep every lockable datum (location-line ends, reference planes, grid lines, levels) addressable by persistent name, and reserve a lock record in `construct-model`'s published format (FORMAT.md) when the first piece of 4 is designed.
4. **Project locks as the first piece after Gate D,** in `construct-solver` as 15.15 and A11 describe: datum systems per plan level or elevation, one logged operation per edit that moves locked elements, a lock that cannot hold kept as a warning. Its first task is the claims rule: the recommendation to test is that an edit takes the claims of every element its solve would move, all or nothing, and is turned back if any is held, as iModel's locks are taken all or nothing (A05).
5. **Measure before building it:** a benchmark of datum systems from generated plans (a 230,000-element building's plan levels, every wall aligned to a grid) through the kernel's solver once G223 to G228 land, against the map's 16 ms budget for a local edit.

## What waits on the kernel

The solver itself (G216 to G231: decomposition, rigid clusters, commit, certificates, dragging, the corpus), persistent names and their resolution (chapter 20's pieces), and the incremental engine that reruns only what a lock touches (A03). Recommendation 2 needs only reference resolution, which the stand-in geometry already supports by name.

## Open risks

- **Revit users expect locks** and may find one-way references limiting at Gate D. The interface should say that locks come later, in plain words, rather than offering a padlock that does nothing.
- **One component per plan level** is plausible for buildings aligned to one grid; its solve time is unmeasured (recommendation 5).
- **The claims rule** in recommendation 4 is untested with real teams: all-or-nothing claims may refuse too often on a busy level.
- **What Revit does exactly** when a locked element is moved by someone else, and how it reports a lock that cannot hold, is *unverified* here; it was not read from Autodesk's documentation, and only the interface structure may be followed (AGENTS.md, 5).

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| Kernel map, chapters [15](../../engines/geometry/architecture/15-sketches.md) (15.5, 15.8, 15.12, 15.15, 15.18) and [20](../../engines/geometry/architecture/20-naming-history.md) (20.4) | This repository | Locks as datum systems, decomposition and its limit, logging, references |
| [A05-multi-user.md](A05-multi-user.md) | This repository | Claims, the log, determinism |
| A11 (#793, in review) | This repository | The remaining solver question, scale untested |
| [`construct-model/src/element.rs`](../crates/construct-model/src/element.rs), `construct-api/src/cloud/elements.rs` | This repository | References and claims as built |
| [ARCHITECTURE.md](../ARCHITECTURE.md), [FEATURES.md](../FEATURES.md) | This repository | The question, the lean, the parts |
