//! Shared engine: simulation.
//!
//! Moves many agents through networks over time, on a fixed time step, so the same
//! inputs always give the same run: vehicles on roads, buses and trains on public
//! transport timetables, and people walking through buildings and streets. Uses the
//! geometry core's spatial index for paths, collisions and line of sight.
//! Used by Spare Construct (traffic, public transport and people movement), and open
//! to any program that needs it.
//! Map: Spare Construct, "Traffic and public transport".
//!
//! In place so far: the shared network search ([`network`]), edge-based shortest paths with
//! turn penalties and bans, and its binary heap ([`heap`]). Spare Construct's transport model
//! routes every trip with it; any program with a network (roads, rail, corridors and stairs)
//! can use the same search. The time-stepped movement of agents is still to come.

pub mod heap;
pub mod network;

pub use heap::Heap;
pub use network::{EdgeGraph, Network, Tree, Turns, Workspace};
