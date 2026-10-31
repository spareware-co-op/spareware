//! Where everything is stored: the key layout, version 1. FORMAT.md describes it for people; this
//! file is the reference for code. Published: a key written under this layout must stay readable
//! forever, so a change here is a new format version, never an edit.
//!
//! A key starts with its scope, so one project's pieces sit side by side in TiKV and a scan of
//! `p:<project>/` finds them all. Every part is a checked [`Id`] or a number, so no part can hold
//! a `/` and one project's prefix can never reach into another's: `p:ab/` is not a prefix of
//! `p:abc/...`. [`Place`] is the one list of what can be stored: every key is built from a place,
//! and parsing a key gives that place back or nothing. The fuzz target holds the two together.

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

    /// Reads a key back into the place it names. `None` for anything else, never a panic.
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
        let mut start = self.0.as_bytes().to_vec();
        if start.is_empty() {
            return (b"!".to_vec(), b"~".to_vec());
        }
        let mut end = start.clone();
        if let Some(last) = end.last_mut() {
            *last += 1;
        }
        start.shrink_to_fit();
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
    /// People, hubs and who belongs where.
    Directory,
    /// One project's model and files.
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
}

/// Everything that can be stored, one variant to a kind of key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// The store's format version: `meta/format`.
    Format,
    Head(Scope),
    Log(Scope, u64),
    Client(Scope, Id, Id),
    /// An element's data, named by the hash of its bytes, so a version is never rewritten.
    Record {
        project: Id,
        hash: Id,
    },
    /// A shape, named by hash too: every door of one type shares it.
    Shape {
        project: Id,
        hash: Id,
    },
    /// Which record is an element's current version.
    Element {
        project: Id,
        model: Id,
        element: Id,
    },
    /// Which elements touch a 5 m cell of a level, so a browser loads only what is in view.
    Area {
        project: Id,
        model: Id,
        level: i32,
        x: i64,
        y: i64,
        element: Id,
    },
    /// Who is working on an element.
    Claim {
        project: Id,
        model: Id,
        element: Id,
    },
    Model {
        project: Id,
        model: Id,
    },
    Team {
        project: Id,
        team: Id,
    },
    /// A package in a team's Shared or Consumed folder.
    Item {
        project: Id,
        team: Id,
        folder: Id,
        item: Id,
    },
    /// The last package number of a model.
    Version {
        project: Id,
        model: Id,
    },
    /// A package bridged in and waiting for the team to take it.
    Incoming {
        project: Id,
        id: Id,
    },
    Link {
        project: Id,
        model: Id,
        link: Id,
    },
    Person(Id),
    /// Who an address belongs to, under the hash of the address.
    Email(Id),
    /// Who a login belongs to, under the hash of its issuer and subject.
    Login(Id),
    Hub(Id),
    HubProject {
        hub: Id,
        project: Id,
    },
    Project(Id),
    /// Someone who may change the directory.
    Admin(Id),
    MemberOfHub {
        person: Id,
        hub: Id,
    },
    MemberOfProject {
        person: Id,
        project: Id,
    },
    MemberOfTeam {
        person: Id,
        project: Id,
        team: Id,
    },
    /// The same hub membership looked up from the hub's side.
    HubUser {
        hub: Id,
        person: Id,
    },
    /// `hub` permits bridges with `other`. A bridge needs this row from both sides.
    Bridge {
        hub: Id,
        other: Id,
    },
    /// A signed-in browser, under the hash of its cookie.
    Session(Id),
    /// A sign-in on its way to the provider and back.
    Pending(Id),
}

fn id(text: &str) -> Option<Id> {
    Id::parse(text).ok()
}

/// A number, only in the one way this layout writes it: no `+`, no leading zeros, no `-0`.
fn number<T: std::str::FromStr + ToString>(text: &str) -> Option<T> {
    let value = text.parse::<T>().ok()?;
    (value.to_string() == text).then_some(value)
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
            Self::Shape { project, hash } => format!("p:{project}/shape/{hash}"),
            Self::Element {
                project,
                model,
                element,
            } => {
                format!("p:{project}/elem/{model}/{element}")
            }
            Self::Area {
                project,
                model,
                level,
                x,
                y,
                element,
            } => {
                format!("p:{project}/area/{model}/{level}/{x}/{y}/{element}")
            }
            Self::Claim {
                project,
                model,
                element,
            } => {
                format!("p:{project}/claim/{model}/{element}")
            }
            Self::Model { project, model } => format!("p:{project}/model/{model}"),
            Self::Team { project, team } => format!("p:{project}/team/{team}"),
            Self::Item {
                project,
                team,
                folder,
                item,
            } => {
                format!("p:{project}/item/{team}/{folder}/{item}")
            }
            Self::Version { project, model } => format!("p:{project}/version/{model}"),
            Self::Incoming { project, id } => format!("p:{project}/incoming/{id}"),
            Self::Link {
                project,
                model,
                link,
            } => format!("p:{project}/link/{model}/{link}"),
            Self::Person(id) => format!("dir/person/{id}"),
            Self::Email(hash) => format!("dir/email/{hash}"),
            Self::Login(hash) => format!("dir/login/{hash}"),
            Self::Hub(id) => format!("dir/hub/{id}"),
            Self::HubProject { hub, project } => format!("dir/hub/{hub}/project/{project}"),
            Self::Project(id) => format!("dir/project/{id}"),
            Self::Admin(person) => format!("acl/admin/{person}"),
            Self::MemberOfHub { person, hub } => format!("acl/person/{person}/hub/{hub}"),
            Self::MemberOfProject { person, project } => {
                format!("acl/person/{person}/project/{project}")
            }
            Self::MemberOfTeam {
                person,
                project,
                team,
            } => {
                format!("acl/person/{person}/team/{project}/{team}")
            }
            Self::HubUser { hub, person } => format!("acl/hub/{hub}/user/{person}"),
            Self::Bridge { hub, other } => format!("acl/hub/{hub}/bridge/{other}"),
            Self::Session(hash) => format!("auth/session/{hash}"),
            Self::Pending(state) => format!("auth/pending/{state}"),
        })
    }

    fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split('/').collect();
        match parts.as_slice() {
            ["meta", "format"] => Some(Self::Format),
            ["dir", rest @ ..] => Self::parse_directory(rest),
            ["acl", rest @ ..] => Self::parse_access(rest),
            ["auth", "session", hash] => Some(Self::Session(id(hash)?)),
            ["auth", "pending", state] => Some(Self::Pending(id(state)?)),
            [scope, rest @ ..] => Self::parse_project(id(scope.strip_prefix("p:")?)?, rest),
            [] => None,
        }
    }

    fn parse_directory(parts: &[&str]) -> Option<Self> {
        Some(match parts {
            ["head"] => Self::Head(Scope::Directory),
            ["log", seq] => Self::Log(Scope::Directory, log_number(seq)?),
            ["client", person, client] => Self::Client(Scope::Directory, id(person)?, id(client)?),
            ["person", person] => Self::Person(id(person)?),
            ["email", hash] => Self::Email(id(hash)?),
            ["login", hash] => Self::Login(id(hash)?),
            ["hub", hub] => Self::Hub(id(hub)?),
            ["hub", hub, "project", project] => Self::HubProject {
                hub: id(hub)?,
                project: id(project)?,
            },
            ["project", project] => Self::Project(id(project)?),
            _ => return None,
        })
    }

    fn parse_access(parts: &[&str]) -> Option<Self> {
        Some(match parts {
            ["admin", person] => Self::Admin(id(person)?),
            ["person", person, "hub", hub] => Self::MemberOfHub {
                person: id(person)?,
                hub: id(hub)?,
            },
            ["person", person, "project", project] => Self::MemberOfProject {
                person: id(person)?,
                project: id(project)?,
            },
            ["person", person, "team", project, team] => Self::MemberOfTeam {
                person: id(person)?,
                project: id(project)?,
                team: id(team)?,
            },
            ["hub", hub, "user", person] => Self::HubUser {
                hub: id(hub)?,
                person: id(person)?,
            },
            ["hub", hub, "bridge", other] => Self::Bridge {
                hub: id(hub)?,
                other: id(other)?,
            },
            _ => return None,
        })
    }

    fn parse_project(project: Id, parts: &[&str]) -> Option<Self> {
        let scope = || Scope::Project(project.clone());
        Some(match parts {
            ["head"] => Self::Head(scope()),
            ["log", seq] => Self::Log(scope(), log_number(seq)?),
            ["client", person, client] => Self::Client(scope(), id(person)?, id(client)?),
            ["record", hash] => Self::Record {
                project,
                hash: id(hash)?,
            },
            ["shape", hash] => Self::Shape {
                project,
                hash: id(hash)?,
            },
            ["elem", model, element] => Self::Element {
                project,
                model: id(model)?,
                element: id(element)?,
            },
            ["area", model, level, x, y, element] => Self::Area {
                project,
                model: id(model)?,
                level: number(level)?,
                x: number(x)?,
                y: number(y)?,
                element: id(element)?,
            },
            ["claim", model, element] => Self::Claim {
                project,
                model: id(model)?,
                element: id(element)?,
            },
            ["model", model] => Self::Model {
                project,
                model: id(model)?,
            },
            ["team", team] => Self::Team {
                project,
                team: id(team)?,
            },
            ["item", team, folder, item] => Self::Item {
                project,
                team: id(team)?,
                folder: id(folder)?,
                item: id(item)?,
            },
            ["version", model] => Self::Version {
                project,
                model: id(model)?,
            },
            ["incoming", incoming] => Self::Incoming {
                project,
                id: id(incoming)?,
            },
            ["link", model, link] => Self::Link {
                project,
                model: id(model)?,
                link: id(link)?,
            },
            _ => return None,
        })
    }
}

/// The ranges scanned for lists. Each ends in `/`, so no prefix runs into a longer name.
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
    pub fn elements(project: &Id, model: &Id) -> Prefix {
        Prefix(format!("p:{project}/elem/{model}/"))
    }
    #[must_use]
    pub fn all_elements(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/elem/"))
    }
    #[must_use]
    pub fn cell(project: &Id, model: &Id, level: i32, x: i64, y: i64) -> Prefix {
        Prefix(format!("p:{project}/area/{model}/{level}/{x}/{y}/"))
    }
    #[must_use]
    pub fn claims(project: &Id, model: &Id) -> Prefix {
        Prefix(format!("p:{project}/claim/{model}/"))
    }
    #[must_use]
    pub fn models(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/model/"))
    }
    #[must_use]
    pub fn teams(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/team/"))
    }
    #[must_use]
    pub fn items(project: &Id, team: &Id, folder: &Id) -> Prefix {
        Prefix(format!("p:{project}/item/{team}/{folder}/"))
    }
    #[must_use]
    pub fn incoming(project: &Id) -> Prefix {
        Prefix(format!("p:{project}/incoming/"))
    }
    #[must_use]
    pub fn links(project: &Id, model: &Id) -> Prefix {
        Prefix(format!("p:{project}/link/{model}/"))
    }
    #[must_use]
    pub fn projects() -> Prefix {
        Prefix("dir/project/".to_owned())
    }
    #[must_use]
    pub fn hub_projects(hub: &Id) -> Prefix {
        Prefix(format!("dir/hub/{hub}/project/"))
    }
    /// Everything a person belongs to.
    #[must_use]
    pub fn memberships(person: &Id) -> Prefix {
        Prefix(format!("acl/person/{person}/"))
    }
    #[must_use]
    pub fn hubs_of(person: &Id) -> Prefix {
        Prefix(format!("acl/person/{person}/hub/"))
    }
    #[must_use]
    pub fn teams_of(person: &Id, project: &Id) -> Prefix {
        Prefix(format!("acl/person/{person}/team/{project}/"))
    }
    #[must_use]
    pub fn hub_users(hub: &Id) -> Prefix {
        Prefix(format!("acl/hub/{hub}/user/"))
    }
    #[must_use]
    pub fn bridges(hub: &Id) -> Prefix {
        Prefix(format!("acl/hub/{hub}/bridge/"))
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
    fn numbers_are_read_in_one_spelling_only() {
        for bad in [
            "p:x/area/m/1/007/0/e",
            "p:x/area/m/1/+7/0/e",
            "p:x/area/m/-0/7/0/e",
            "p:x/log/7",
            "p:x/log/0000000000000000000a",
            "dir/log/99999999999999999999",
        ] {
            assert_eq!(Key::from_stored(bad.to_owned()).parse(), None, "{bad}");
        }
        let negative = Key::from_stored("p:x/area/m/-1/-7/0/e".to_owned());
        assert!(matches!(
            negative.parse(),
            Some(Place::Area {
                level: -1,
                x: -7,
                ..
            })
        ));
    }

    #[test]
    fn nothing_outside_the_list_parses() {
        for bad in [
            "",
            "/",
            "dir",
            "dir/",
            "p:/head",
            "p:a/b/head",
            "p:a/nope/b",
            "meta/other",
            "x/y",
        ] {
            assert_eq!(Key::from_stored(bad.to_owned()).parse(), None, "{bad}");
        }
    }
}
