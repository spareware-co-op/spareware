//! Spare Construct's cloud server. See README.md for what it proves, deploy/README.md for putting
//! it online, and note K08 for why it is shaped this way. Thin on purpose: the rules are in the
//! kit, in `construct-api`, and in [`service`].

pub mod backup;
pub mod directory;
pub mod http;
pub mod seed;
pub mod service;
pub mod settings;
pub mod shapes;
mod ws;
