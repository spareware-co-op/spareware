//! Shared engine: simulation.
//!
//! Moves many agents through networks over time, on a fixed time step, so the same
//! inputs always give the same run: vehicles on roads, buses and trains on public
//! transport timetables, and people walking through buildings and streets. Uses the
//! geometry core's spatial index for paths, collisions and line of sight.
//! Used by Spare Construct (traffic, public transport and people movement), and open
//! to any program that needs it.
//! Map: Spare Construct, "Traffic and public transport".
//! No code yet: this crate waits until its first program reaches the Design stage.
