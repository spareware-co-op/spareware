//! Spare Construct: command and query API.
//!
//! The one door into Spare Construct for the interface, AI agents, plugins and scripts: every action is a named command with
//! JSON in and JSON out, and the same commands run natively (command line, MCP server) and in the browser (WebAssembly).
//! Map: projects/spare-construct/ARCHITECTURE.md, "Headless API, AI and add-ins".
//!
//! The Transport discipline is the first to be covered: see [`transport`].

pub mod error;
pub mod schema;
pub mod transport;

pub use error::{ApiError, ApiResult};
pub use transport::Session;
