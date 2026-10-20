//! Shared engine: geometry core.
//!
//! The one 3D core every program builds on. Exact predicates, flat-faced solids,
//! extrude, sweep, revolve, blend and robust booleans, porting Manifold's published
//! method; curved surfaces later behind the same interface. Turns exact solids into
//! triangle meshes for the renderer, and holds the spatial index for fast "what is
//! near what" queries (clash, snapping, simulation, streaming).
//! Used by Spare Construct (every discipline, civil ground included), Spare Studio's
//! 3D app and the simulation engine.
//! Map: Spare Construct, "Geometry kernel". Waits on research: piece A01.
//! No code yet: this crate waits until its first program reaches the Design stage.
