# 22. Errors, queries and what AI agents see

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

The kernel is headless by design. An AI agent cannot look at the screen, so everything it needs to know is data.

**Errors explain themselves.** Every error is structured, in the same shape for people and AI:

```rust
struct OpError {
    kind: ErrorKind,            // the shared base, one of the closed set below: e.g. NoSolution, Ambiguous, ToleranceExceeded, Cancelled
    detail: OpErrorDetail,      // the operation's own typed kind and its data: e.g. RadiusTooLarge { largest }, SelfIntersection { at }
    op: OpPath,                 // which operation, which step
    entities: Vec<NameId>,      // the faces, edges and vertices involved
    at: Vec<Evidence>,          // points, parameters, distances, the bound that was exceeded
    message: Text,              // plain words, translatable
    remedies: Vec<Remedy>,      // what would make it succeed: "radius at most 4.2 mm here", "use the mesh boolean", "heal with ε_heal 0.3 mm"
}
```

Compare "Can't create form element" in Revit with "fillet radius 6 mm exceeds the 4.2 mm available between faces exterior and top of wall W-12 at station 3.40 m; largest radius that fits: 4.2 mm".

**A shared base, and a detail per operation.** Section 10.1 says `OpError` is typed per operation, on a shared base. The two fields `kind` and `detail` are how:

- **`kind` is the shared base**, one of the kinds in the table below. Generic code (the command layer, the MCP tools, AI agents, the test harness) can handle any error by its kind alone. The set is closed and Stable (section 25). It is not marked `#[non_exhaustive]`, so callers handle every kind, and adding one is a breaking change: it needs a decision note and a new major version.
- **`detail` is typed per operation.** `OpErrorDetail` has one variant per operation family (booleans, sweeps, sketches and so on), each holding that family's own enum, such as a sweep's `MitreMismatch { .. }` or a sketch's `Conflict { .. }`. Each operation's schema lists the details it can return, with each one's base kind and data, and a detail is versioned with its operation's schema (section 26.2). `OpErrorDetail` is `#[non_exhaustive]`: it grows with the kernel, and generic code reads the schema instead of matching it. A new failure is a new detail, never a new base kind.
- **The two always agree.** An `OpError` is built from its detail, and `kind` is that detail's base kind.
- **A crate's own error type never reaches a caller.** The calling operation wraps it into its own detail and base kind, adding the entities involved, as section 12.16 asks for `IntersectError`.
- **The numbers layer's errors map like any crate's, by meaning, not by name.** `geom-num`'s `NumError::BudgetExceeded` means an exact computation exceeded its proven bit budget, which only a defect can cause (6.2): it becomes `Internal`, never `BudgetExceeded`. Its `ToleranceExceeded`, which `Exactness::within` and `combine_within` report for a certified bound over its ceiling, becomes `ExactnessNotAccepted`. Its `OutOfRange` for a grid coordinate becomes `FrameRangeExceeded`, its `Degenerate` becomes `Degenerate`, and its `UnprovenMaths` becomes `Unsupported`, with the function and piece named by the wrapping error (12.16), because `NumError::UnprovenMaths` carries neither. G82 decided the six 31.7 left open, by the same rule: `NotFinite` is `Degenerate` (kernel values are bounded, so on valid input a computation leaves the finite doubles only at a singular configuration; an operation checks a caller's own number first, as its `InvalidInput`), `OutsideDomain` is `NoSolution` (no real value exists), `DivisorContainsZero` and `NegativeSqrt` are `Undecided` (the enclosure cannot say whether the divisor vanishes or the value is negative; an exact zero is found first by the exact predicates, as `Degenerate`), and `EmptyInterval` and `WrongDirection` are `Internal` (both are chosen by kernel code, never by data). `geom-op` wraps each `NumError` into a `NumDetail` named by its meaning, `BitBudget` for `BudgetExceeded` and `BoundOverCeiling` for `ToleranceExceeded`, so the two misleading names never reach a caller and `geom-num` keeps its own. `geom-math` (G03) also reports its own impossible failures as `NumError::BudgetExceeded`; wrapped, that is `Internal` with the wrapping operation's path and message, so `geom-math` needs no error type of its own for them. `geom-run`'s `RunError::Internal` (G04) is already `Internal`, and its other stops become `geom-op`'s `RunDetail`.
- **Every error table maps to the base.** Each chapter's error table and each schema gives every detail its base kind, as 15.13 does. A test or fixture that expects an error names both (section 28.16).

**The base kinds.** Each kind is defined once, here, and means the same in every chapter, schema and test, section 28.2's contract included. This table seeds the error catalogue that section 26.3 generates from the code.

| Base kind | What it means | What the caller can do | Details, for example |
| --- | --- | --- | --- |
| `InvalidInput` | An input breaks the operation's preconditions: it fails the checker, is malformed, or repeats a key | Fix or replace the input | `InvalidOperand` (11.11), `InvalidView` (18.11), `Malformed` (24.7) |
| `Degenerate` | A valid input is degenerate for this operation: a zero length, angle or radius, coincident anchors, a folded arc, a singular start | Change the value it names | The entity and the value (13.20, 15.13) |
| `NoSolution` | The input is valid, but no result meets it and the rules the caller set | Change a value; the remedies give the nearest one that works | `RadiusTooLarge`, `SelfIntersection` (13.20), `NonManifoldResult` (11.11), `Conflict` (15.13) |
| `Ambiguous` | More than one result fits, and nothing given chooses between them | Choose, or add what decides | `RootChoiceUndetermined` (15.13); an ambiguous reference (14.13) |
| `Undecided` | A decision the result needs can be neither made exactly nor certified at the data's own resolution. A three-way query, such as clearance (14.9), returns `Undecided` as an answer instead | Move things apart, close the gap explicitly, or take another route | `NearContact`, `UndecidedOrder` (11.11), `IllConditioned` (13.20) |
| `ExactnessNotAccepted` | The result would be less exact than the caller accepted: a lower class, a certified bound past the accepted one (ε_fit, the operation's ceiling, unless the caller explicitly accepts more; 11.1, 14.3), or a commit to the grid that would flip a decision | Accept explicitly the class and bound it would reach, which the error carries; or make the change that keeps it exact | `Class`, `Bound { needed }`, `CommitFlip`, the one name for a flipping commit (13.2, 14.3, 15.10) |
| `ToleranceExceeded` | Only section 6.5's meaning, for Tolerant data: an operation would need a larger tolerance than a Tolerant body carries, or healing would need to close a gap wider than ε_heal allows | Heal first; raise ε_heal up to its cap | The edge or gap, its tolerance and the tolerance needed |
| `FrameRangeExceeded` | A coordinate would leave ±(2^40 − 1) steps of its frame's origin (section 6.1) | Work in a frame placed at the overlap or on section 17.1's lattice; split; or ask for a coarser step, which lowers the class | `Operands` (the inputs, or imported data, do not fit one frame), `Result`, `Commit` (an implicit point beyond the range, 14.4) |
| `BudgetExceeded` | A deterministic work or logical-memory budget ran out (section 23) | Raise the budget, split the call, or ask for a preview | `{ budget, kind, phase, used }`: `kind` says which limit, work or logical bytes, and `used` is counted as 23.10 says |
| `Cancelled` | The caller's cancellation token, or a deadline it set, stopped the operation | Nothing. It is not deterministic, so it is never logged or cached | None |
| `OutOfMemory` | The machine refused a large allocation | Nothing. Never logged or cached, like `Cancelled` | None |
| `Unsupported` | No route exists yet for this input | The routes that exist, or the build piece that adds one | `RepresentationUnsupported` (13.20), `UnprovenMaths` (12.16), `UnsupportedRecord` (24.2) |
| `Internal` | An invariant broke: a bug | Report it. It carries what reproduces it, and is never a panic or a success | None |

Where two kinds sit close, the line between them is fixed:

- **Exactness against tolerance.** A result less exact than the caller accepted is always `ExactnessNotAccepted`, whatever the cause. `ToleranceExceeded` never means a certified bound; it is for Tolerant data only.
- **Frame range against malformed bytes.** Valid data too wide for one frame is `FrameRangeExceeded`, such as foreign coordinates that need splitting across lattice frames (24.8). A field that breaks its bit budget, such as a non-finite number, an oversized plane or a native file's coordinate past ±2^40 steps, cannot be valid data. It is `InvalidInput` with `Malformed { record, offset, field, bits }` (24.7).
- **Budget against certification.** Running out of budget is always `BudgetExceeded`, including a certification that runs out of budget (18.5). `Undecided` means more work would not help at the data's resolution.

**Everything can be asked.** Queries are first-class operations: classify a point, list a body's faces with their names and roles, measure, find the distance or clearance between two things, area and volume, exactness class and bounds, validity report, curvature, section a body to get a plan. An AI can check its own work the way a person looks at the screen.

**Every operation is described as data.** Each operation has a schema: inputs with types and units, outputs, roles it names, errors it can return (each detail with its base kind and data), its exactness, and an example. Spare Kit's command layer generates the API, the MCP tools and the documentation from these schemas (EXTENDING.md, section 2.1), so AI tools are never written by hand and never drift from the kernel.

**Determinism makes AI work safe.** An agent can try an operation, inspect the result, and replay or undo it, and get exactly the same answer every time.

**A proposed change can be reviewed with certainty (proposed).** A program may let an AI agent work on a draft and hand back a proposal for a person to accept or reject change by change. The review screen and the accept-or-reject flow belong to Spare Kit's command layer; the kernel supplies the three things it needs. A **preview** is just an operation run without the program adopting its result: operations are pure (section 10.1), so trying one never touches the model, and the program decides whether to keep the new bodies. A **model diff** is a query that compares two models by persistent names (section 20) and lists every face, edge and vertex added, removed, modified, split or merged, with the operation that did it and its exactness class and bounds before and after; it is computed from the operations' history, never by guessing which shapes look alike. And because each change is an operation with its own `OpId` in the log, a person can accept some and reject others: the program replays the accepted operations, and one that leaned on a rejected change fails with its reference error (section 20.4) rather than guessing. The diff query's exact output shape is an open question for the piece that builds it.
