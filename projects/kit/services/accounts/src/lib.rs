//! Shared service: accounts. One login for every app, the directory of who belongs where, and the
//! access checks every request goes through. Map: projects/kit/ARCHITECTURE.md; note K08.
//!
//! - [`rows`]: people, hubs, projects, teams and models, as stored.
//! - [`access`]: Cedar's yes or no, from the rows, fresh on every request.
//! - [`directory`]: the commands that change the directory, through its own edit log.
//! - [`sign_in`]: OpenID Connect and sessions.

pub mod access;
pub mod directory;
pub mod rows;
pub mod sign_in;

pub use access::{Access, Target};
pub use directory::{DirectoryCommands, SYSTEM};
pub use sign_in::{Accounts, SignInError, SignInSettings};
