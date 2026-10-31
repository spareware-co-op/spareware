//! Shared service: accounts. One login for every app, the people the directory holds, and Cedar
//! to answer access questions. Map: projects/kit/ARCHITECTURE.md; note K08.
//!
//! - [`rows`]: people, as stored.
//! - [`cedar`]: Cedar's yes or no, to a program's own rules.
//! - [`people`]: the commands that change who exists, through the directory's edit log.
//! - [`sign_in`]: OpenID Connect and sessions.
//!
//! Where people belong, and who may do what, is each program's own. Spare Construct's are in
//! `construct-api`'s `cloud` module.

pub mod cedar;
pub mod people;
pub mod rows;
pub mod sign_in;

pub use cedar::Cedar;
pub use people::SYSTEM;
pub use sign_in::{Accounts, SignInError, SignInSettings};
