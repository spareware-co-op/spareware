//! Spare Construct: transport model.
//!
//! The calculation core of the strategic traffic model, headless: network graph, trip
//! generation and distribution, user-equilibrium assignment with junction delay, junction
//! results, indicative economics and the assessment tests. The same code runs from the
//! command line, for AI agents over MCP, and in the browser as WebAssembly.
//! It is a faithful port of the browser prototype's calculation scripts
//! (projects/spare-construct/demo/transport/js: engine.js, util.js, model.js and the
//! scripts beside them). Parity is proven by truth tests: tools/transport-truth runs the
//! prototype in Node on saved projects and records every answer, and this crate's tests
//! must reproduce them (tests/support has the loader and the comparator).
//! Porting rule: same formulas, same operation order, same constants, and JavaScript's
//! number semantics where the prototype relies on them (see [`jsnum`]).
//! Map: projects/spare-construct/ARCHITECTURE.md, "Traffic and public transport".

pub mod jsnum;
pub mod project;
pub mod standards;
pub mod util;

pub mod analysis;
pub mod assign;
pub mod demand;
pub mod econ;
pub mod graph;
pub mod junction;
pub mod results;
