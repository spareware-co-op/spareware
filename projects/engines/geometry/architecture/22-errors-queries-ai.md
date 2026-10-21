# 22. Errors, queries and what AI agents see

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

The kernel is headless by design. An AI agent cannot look at the screen, so everything it needs to know is data.

**Errors explain themselves.** Every error is structured, in the same shape for people and AI:

```rust
struct OpError {
    kind: ErrorKind,            // e.g. SelfIntersection, ToleranceExceeded, RadiusTooLarge, Ambiguous, NoSolution, Cancelled
    op: OpPath,                 // which operation, which step
    entities: Vec<NameId>,      // the faces, edges and vertices involved
    at: Vec<Evidence>,          // points, parameters, distances, the bound that was exceeded
    message: Text,              // plain words, translatable
    remedies: Vec<Remedy>,      // what would make it succeed: "radius at most 4.2 mm here", "use the mesh boolean", "heal with ε_heal 0.3 mm"
}
```

Compare "Can't create form element" in Revit with "fillet radius 6 mm exceeds the 4.2 mm available between faces exterior and top of wall W-12 at station 3.40 m; largest radius that fits: 4.2 mm".

**Everything can be asked.** Queries are first-class operations: classify a point, list a body's faces with their names and roles, measure, find the distance or clearance between two things, area and volume, exactness class and bounds, validity report, curvature, section a body to get a plan. An AI can check its own work the way a person looks at the screen.

**Every operation is described as data.** Each operation has a schema: inputs with types and units, outputs, roles it names, errors it can return, its exactness, and an example. Spare Kit's command layer generates the API, the MCP tools and the documentation from these schemas (EXTENDING.md, section 2.1), so AI tools are never written by hand and never drift from the kernel.

**Determinism makes AI work safe.** An agent can try an operation, inspect the result, and replay or undo it, and get exactly the same answer every time.
