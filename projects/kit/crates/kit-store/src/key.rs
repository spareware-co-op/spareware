//! Where everything is stored: the key layout, version 1. FORMAT.md describes it for people; this
//! file is the reference for code. Published: a key written under this layout must stay readable
//! forever, so a change here is a new format version, never an edit.
//!
//! A key starts with its scope, so one project's pieces sit side by side and a scan of
//! `p:<project>/` finds them all. Every part is a checked [`Id`] or a number, so no part can hold
//! a `/` and one project's prefix can never reach into another's: `p:ab/` is not a prefix of
//! `p:abc/...`.
//!
//! The kit lists only what every program has: logs, content-addressed records, pieces, claims and
//! people. A program adds tables of its own with [`Scope::table`] and [`table`], and keeps its own
//! list of them. [`Place`] is the kit's list: every key here is built from a place, and parsing
//! a key gives that place back or nothing. The fuzz target holds the two together.

use kit_core::Id;
use std::fmt;

/// The version of the key layout. It is stored in the store itself, at `meta/format`.
pub const FORMAT: u32 = 1;

/// Log numbers are padded to the full width of a `u64`, so byte order is log order.
const LOG_WIDTH: usize = 20;

/// A key, always plain ASCII.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(String);

impl Key {
    /// Wraps a key read back from a store. It may be one no version of this code wrote.
    #[must_use]
    pub fn from_stored(text: String) -> Self {
        Self(text)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Reads a key back into the place it names. `None` for a key that isn't the kit's, such as a
    /// program's own table, and never a panic.
    #[must_use]
    pub fn parse(&self) -> Option<Place> {
        Place::parse(&self.0)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Every key beginning with some text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefix(String);

impl Prefix {
    /// Every key the store holds. All keys sort between `!` and `~`.
    #[must_use]
    pub fn everything() -> Self {
        Self(String::new())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first key in the range, and the first key past it.
    #[must_use]
    pub fn bounds(&self) -> (Vec<u8>, Vec<u8>) {
        let start = self.0.as_bytes().to_vec();
        if start.is_empty() {
            return (b"!".to_vec(), b"~".to_vec());
        }
        let mut end = start.clone();
        if let Some(last) = end.last_mut() {
            *last += 1;
        }
        (start, end)
    }

    /// What a key in this range has after the prefix.
    #[must_use]
    pub fn rest<'a>(&self, key: &'a Key) -> Option<&'a str> {
        key.as_str().strip_prefix(&self.0)
    }
}

/// What a log belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Who exists and who belongs where.
    Directory,
    /// One project's documents and files.
    Project(Id),
}

impl Scope {
    fn root(&self) -> String {
        match self {
            Self::Directory => "dir".to_owned(),
            Self::Project(project) => format!("p:{project}"),
        }
    }

    /// The last log number given out. Every commit changes this key, so only one writer can win
    /// each number, however many servers are running.
    #[must_use]
    pub fn head(&self) -> Key {
        Key(format!("{}/head", self.root()))
    }

    #[must_use]
    pub fn log(&self, seq: u64) -> Key {
        Key(format!("{}/log/{seq:0LOG_WIDTH$}", self.root()))
    }

    /// The last edit taken from one browser tab, so a resent edit is never applied twice.
    #[must_use]
    pub fn client(&self, person: &Id, client: &Id) -> Key {
        Key(format!("{}/client/{person}/{client}", self.root()))
    }

    /// A key in a table of a program's own, in this scope.
    #[must_use]
    pub fn table(&self, table: &str, parts: &[&str]) -> Key {
        Key(join(&self.root(), table, parts))
    }

    /// A range in a table of a program's own, in this scope.
    #[must_use]
    pub fn table_prefix(&self, table: &str, parts: &[&str]) -> Prefix {
        Prefix(format!("{}/", join(&self.root(), table, parts)))
    }
}

fn join(root: &str, table: &str, parts: &[&str]) -> String {
    let mut key = format!("{root}/{table}");
    for part in parts {
        key.push('/');
        key.push_str(part);
    }
    key
}

/// A key in a program's own top-level table, outside any scope, such as its access rows.
#[must_use]
pub fn table(root: &str, table: &str, parts: &[&str]) -> Key {
    Key(join(root, table, parts))
}

/// A range in a program's own top-level table.
#[must_use]
pub fn table_prefix(root: &str, table: &str, parts: &[&str]) -> Prefix {
    Prefix(format!("{}/", join(root, table, parts)))
}

/// Everything the kit itself stores, one variant to a kind of key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// The store's format version: `meta/format`.
    Format,
    Head(Scope),
    Log(Scope, u64),
    Client(Scope, Id, Id),
    /// A piece's data, named by the hash of its bytes, so a version is never rewritten.
    Record { project: Id, hash: Id },
    /// Which record is a piece's current version, in one document of a project.
    Piece { project: Id, document: Id, piece: Id },
    /// Who is working on a piece.
    Claim { project: Id, document: Id, piece: Id },
    Person(Id),
    /// Who an address belongs to, under the hash of the address.
    Email(Id),
    /// Who a login belongs to, under the hash of its issuer and subject.
    Login(Id),
    /// Someone who may change the directory.
    Admin(Id),
    /// A signed-in browser, under the hash of its cookie.
    Session(Id),
    /// A sign-in on its way to the provider and back.
    Pending(Id),
}

fn id(text: &str) -> Option<Id> {
    Id::parse(text).ok()
}

fn log_number(text: &str) -> Option<u64> {
    let digits = text.len() == LOG_WIDTH && text.bytes().all(|b| b.is_ascii_digit());
    digits.then(|| text.parse().ok()).flatten()
}

impl Place {
    /// The key of this place.
    #[must_use]
    pub fn key(&self) -> Key {
        Key(match self {
            Self::Format => "meta/format".to_owned(),
            Self::Head(scope) => return scope.head(),
            Self::Log(scope, seq) => return scope.log(*seq),
            Self::Client(scope, person, client) => return scope.client(person, client),
            Self::Record { project, hash } => format!("p:{project}/record/{hash}"),
            Self::Piece { project, document, piece } => {
                format!("p:{project}/piece/{document}/{piece}")
            }
            Self::Claim { project, document, piece } => {
                format!("p:{project}/claim/{document}/{piece}")
            }
            Self::Person(id) => format!("dir/person/{id}"),
            Self::Email(hash) => format!("dir/email/{hash}"),
            Self::Login(hash) => format!("dir/login/{hash}"),
            Self::Admin(person) => format!("acl/admin/{person}"),
            Self::Session(hash) => format!("auth/session/{hash}"),
            Self::Pending(state) => format!("auth/pending/{state}"),
        })
    }

    fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split('/').collect();
        Some(match parts.as_slice() {
            ["meta", "format"] => Self::Format,
            ["acl", "admin", person] => Self::Admin(id(person)?),
            ["auth", "session", hash] => Self::Session(id(hash)?),
            ["auth", "pending", state] => Self::Pending(id(state)?),
            ["dir", "head"] => Self::Head(Scope::Directory),
            ["dir", "log", seq] => Self::Log(Scope::Directory, log_number(seq)?),
            ["dir", "client", person, client] => {
                Self::Client(Scope::Directory, id(person)?, id(client)?)
            }
            ["dir", "person", person] => Self::Person(id(person)?),
            ["dir", "email", hash] => Self::Email(id(hash)?),
            ["dir", "login", hash] => Self::Login(id(hash)?),
            [scope, rest @ ..] => return Self::parse_project(id(scope.strip_prefix("p:")?)?, rest),
            [] => return None,
        })
    }

    fn parse_project(project: Id, parts: &[&str]) -> Option<Self> {
        let scope = || Scope::Project(project.clone());
        Some(match parts {
            ["head"] => Self::Head(scope()),
            ["log", seq] => Self::Log(scope(), log_number(seq)?),
            ["client", person, client] => Self::Client(scope(), id(person)?, id(client)?),
            ["record", hash] => Self::Record { project, hash: id(hash)? },
            ["piece", document, piece] => {
                Self::Piece { project, document: id(document)?, piece: id(piece)? }
            }
            ["claim", document, piece] => {
                Self::Claim { project, document: id(document)?, piece: id(piece)? }
            }
            _ => return None,
        })
    }
}

/// The ranges the kit scans. Each ends in `/`, so no prefix runs into a longer name.
pub mod list {
    use super::{Id, Prefix, Scope};

    #[must_use]
    pub fn log(scope: &Scope) -> Prefix {
        Prefix(format!("{}/log/", scope.root()))
    }
    #[must_use]
    pub fn project(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/"))
    }
    #[must_use]
    pub fn pieces(project: &Id, document: &Id) -> Prefix {
        Prefix(format!("p:{project}/piece/{document}/"))
    }
    #[must_use]
    pub fn all_pieces(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/piece/"))
    }
    #[must_use]
    pub fn claims(project: &Id, document: &Id) -> Prefix {
        Prefix(format!("p:{project}/claim/{document}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    #[test]
    fn log_keys_sort_in_log_order() {
        let scope = Scope::Project(i("x"));
        assert!(scope.log(9) < scope.log(10));
        assert!(scope.log(99_999) < scope.log(100_000));
        assert!(scope.log(u64::MAX - 1) < scope.log(u64::MAX));
    }

    #[test]
    fn a_prefix_never_reaches_into_a_longer_project_name() {
        let (start, end) = list::project(&i("ab")).bounds();
        let other = Scope::Project(i("abc")).head();
        assert!(other.as_bytes() < &start[..] || other.as_bytes() >= &end[..]);
    }

    #[test]
    fn every_key_in_a_project_list_is_inside_its_bounds() {
        let (start, end) = list::log(&Scope::Project(i("x"))).bounds();
        let key = Scope::Project(i("x")).log(7);
        assert!(key.as_bytes() >= &start[..] && key.as_bytes() < &end[..]);
    }

    #[test]
    fn log_numbers_are_read_in_one_spelling_only() {
        for bad in ["p:x/log/7", "p:x/log/0000000000000000000a", "dir/log/99999999999999999999"] {
            assert_eq!(Key::from_stored(bad.to_owned()).parse(), None, "{bad}");
        }
    }

    #[test]
    fn a_programs_table_keeps_to_its_scope() {
        let scope = Scope::Project(i("x"));
        let key = scope.table("thing", &["a", "b"]);
        assert_eq!(key.as_str(), "p:x/thing/a/b");
        let (start, end) = scope.table_prefix("thing", &["a"]).bounds();
        assert!(key.as_bytes() >= &start[..] && key.as_bytes() < &end[..]);
        assert_eq!(key.parse(), None, "the kit doesn't read a program's tables");
    }

    #[test]
    fn nothing_outside_the_list_parses() {
        for bad in
            ["", "/", "dir", "dir/", "p:/head", "p:a/b/head", "p:a/nope/b", "meta/other", "x/y"]
        {
            assert_eq!(Key::from_stored(bad.to_owned()).parse(), None, "{bad}");
        }
    }
}
