//! Spare Construct's tables in the store, on top of the kit's key layout. FORMAT.md describes them
//! for people; this is the reference for code. Published: a key written here stays readable
//! forever. [`Place`] is the one list: every key is built from a place, and parsing a key gives
//! that place back or nothing. The fuzz target holds the two together.

use kit_core::Id;
use kit_store::{Key, Prefix, Scope, table, table_prefix};

/// Everything Spare Construct stores outside the kit's own tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A built body, by content ID. Derived.
    Body {
        project: Id,
        id: Id,
    },
    /// A display record, by content ID. Derived.
    Display {
        project: Id,
        id: Id,
    },
    /// What regeneration made of an element. Derived.
    Built {
        project: Id,
        document: Id,
        element: Id,
    },
    /// How far regeneration has got through a project's log. Derived.
    Regenerated {
        project: Id,
    },
    /// Which elements touch a 5 m cell of a level, so a browser loads only what is in view. Derived.
    Area {
        project: Id,
        document: Id,
        level: i32,
        x: i64,
        y: i64,
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
    Hub(Id),
    HubProject {
        hub: Id,
        project: Id,
    },
    Project(Id),
    /// A person's home hub.
    Home(Id),
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
}

fn in_project(project: &Id) -> Scope {
    Scope::Project(project.clone())
}

fn id(text: &str) -> Option<Id> {
    Id::parse(text).ok()
}

/// A number, only in the one way this layout writes it: no `+`, no leading zeros, no `-0`.
fn number<T: std::str::FromStr + ToString>(text: &str) -> Option<T> {
    let value = text.parse::<T>().ok()?;
    (value.to_string() == text).then_some(value)
}

impl Place {
    /// The key of this place.
    #[must_use]
    pub fn key(&self) -> Key {
        match self {
            Self::Body { project, id } => in_project(project).table("body", &[id.as_str()]),
            Self::Display { project, id } => in_project(project).table("display", &[id.as_str()]),
            Self::Built {
                project,
                document,
                element,
            } => in_project(project).table("built", &[document.as_str(), element.as_str()]),
            Self::Regenerated { project } => in_project(project).table("regenerated", &[]),
            Self::Area {
                project,
                document,
                level,
                x,
                y,
                element,
            } => {
                let (level, x, y) = (level.to_string(), x.to_string(), y.to_string());
                let parts = [document.as_str(), &level, &x, &y, element.as_str()];
                in_project(project).table("area", &parts)
            }
            Self::Model { project, model } => in_project(project).table("model", &[model.as_str()]),
            Self::Team { project, team } => in_project(project).table("team", &[team.as_str()]),
            Self::Item {
                project,
                team,
                folder,
                item,
            } => {
                let parts = [team.as_str(), folder.as_str(), item.as_str()];
                in_project(project).table("item", &parts)
            }
            Self::Version { project, model } => {
                in_project(project).table("version", &[model.as_str()])
            }
            Self::Incoming { project, id } => in_project(project).table("incoming", &[id.as_str()]),
            Self::Link {
                project,
                model,
                link,
            } => in_project(project).table("link", &[model.as_str(), link.as_str()]),
            Self::Hub(id) => table("dir", "hub", &[id.as_str()]),
            Self::HubProject { hub, project } => {
                table("dir", "hub", &[hub.as_str(), "project", project.as_str()])
            }
            Self::Project(id) => table("dir", "project", &[id.as_str()]),
            Self::Home(person) => table("dir", "home", &[person.as_str()]),
            Self::MemberOfHub { person, hub } => {
                table("acl", "person", &[person.as_str(), "hub", hub.as_str()])
            }
            Self::MemberOfProject { person, project } => table(
                "acl",
                "person",
                &[person.as_str(), "project", project.as_str()],
            ),
            Self::MemberOfTeam {
                person,
                project,
                team,
            } => table(
                "acl",
                "person",
                &[person.as_str(), "team", project.as_str(), team.as_str()],
            ),
            Self::HubUser { hub, person } => {
                table("acl", "hub", &[hub.as_str(), "user", person.as_str()])
            }
            Self::Bridge { hub, other } => {
                table("acl", "hub", &[hub.as_str(), "bridge", other.as_str()])
            }
        }
    }

    /// Reads a key back. `None` for anything that isn't Spare Construct's, never a panic.
    #[must_use]
    pub fn parse(key: &Key) -> Option<Self> {
        let parts: Vec<&str> = key.as_str().split('/').collect();
        Some(match parts.as_slice() {
            ["dir", "hub", hub] => Self::Hub(id(hub)?),
            ["dir", "hub", hub, "project", project] => Self::HubProject {
                hub: id(hub)?,
                project: id(project)?,
            },
            ["dir", "project", project] => Self::Project(id(project)?),
            ["dir", "home", person] => Self::Home(id(person)?),
            ["acl", "person", person, "hub", hub] => Self::MemberOfHub {
                person: id(person)?,
                hub: id(hub)?,
            },
            ["acl", "person", person, "project", project] => Self::MemberOfProject {
                person: id(person)?,
                project: id(project)?,
            },
            ["acl", "person", person, "team", project, team] => Self::MemberOfTeam {
                person: id(person)?,
                project: id(project)?,
                team: id(team)?,
            },
            ["acl", "hub", hub, "user", person] => Self::HubUser {
                hub: id(hub)?,
                person: id(person)?,
            },
            ["acl", "hub", hub, "bridge", other] => Self::Bridge {
                hub: id(hub)?,
                other: id(other)?,
            },
            [scope, rest @ ..] => return Self::parse_project(id(scope.strip_prefix("p:")?)?, rest),
            [] => return None,
        })
    }

    fn parse_project(project: Id, parts: &[&str]) -> Option<Self> {
        Some(match parts {
            ["body", hash] => Self::Body {
                project,
                id: id(hash)?,
            },
            ["display", hash] => Self::Display {
                project,
                id: id(hash)?,
            },
            ["built", document, element] => Self::Built {
                project,
                document: id(document)?,
                element: id(element)?,
            },
            ["regenerated"] => Self::Regenerated { project },
            ["area", document, level, x, y, element] => Self::Area {
                project,
                document: id(document)?,
                level: number(level)?,
                x: number(x)?,
                y: number(y)?,
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
    use super::{Id, Prefix, in_project, table_prefix};

    #[must_use]
    pub fn cell(project: &Id, document: &Id, level: i32, x: i64, y: i64) -> Prefix {
        let (level, x, y) = (level.to_string(), x.to_string(), y.to_string());
        in_project(project).table_prefix("area", &[document.as_str(), &level, &x, &y])
    }
    #[must_use]
    pub fn built(project: &Id, document: &Id) -> Prefix {
        in_project(project).table_prefix("built", &[document.as_str()])
    }
    /// Every derived table of a project.
    #[must_use]
    pub fn derived(project: &Id) -> [Prefix; 4] {
        ["body", "display", "built", "area"].map(|name| in_project(project).table_prefix(name, &[]))
    }
    #[must_use]
    pub fn models(project: &Id) -> Prefix {
        in_project(project).table_prefix("model", &[])
    }
    #[must_use]
    pub fn teams(project: &Id) -> Prefix {
        in_project(project).table_prefix("team", &[])
    }
    #[must_use]
    pub fn items(project: &Id, team: &Id, folder: &Id) -> Prefix {
        in_project(project).table_prefix("item", &[team.as_str(), folder.as_str()])
    }
    #[must_use]
    pub fn incoming(project: &Id) -> Prefix {
        in_project(project).table_prefix("incoming", &[])
    }
    #[must_use]
    pub fn links(project: &Id, model: &Id) -> Prefix {
        in_project(project).table_prefix("link", &[model.as_str()])
    }
    #[must_use]
    pub fn projects() -> Prefix {
        table_prefix("dir", "project", &[])
    }
    #[must_use]
    pub fn hub_projects(hub: &Id) -> Prefix {
        table_prefix("dir", "hub", &[hub.as_str(), "project"])
    }
    /// Everything a person belongs to.
    #[must_use]
    pub fn memberships(person: &Id) -> Prefix {
        table_prefix("acl", "person", &[person.as_str()])
    }
    #[must_use]
    pub fn hubs_of(person: &Id) -> Prefix {
        table_prefix("acl", "person", &[person.as_str(), "hub"])
    }
    #[must_use]
    pub fn teams_of(person: &Id, project: &Id) -> Prefix {
        table_prefix(
            "acl",
            "person",
            &[person.as_str(), "team", project.as_str()],
        )
    }
    #[must_use]
    pub fn hub_users(hub: &Id) -> Prefix {
        table_prefix("acl", "hub", &[hub.as_str(), "user"])
    }
    #[must_use]
    pub fn bridges(hub: &Id) -> Prefix {
        table_prefix("acl", "hub", &[hub.as_str(), "bridge"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Key {
        Key::from_stored(text.to_owned())
    }

    #[test]
    fn numbers_are_read_in_one_spelling_only() {
        for bad in [
            "p:x/area/m/1/007/0/e",
            "p:x/area/m/1/+7/0/e",
            "p:x/area/m/-0/7/0/e",
        ] {
            assert_eq!(Place::parse(&key(bad)), None, "{bad}");
        }
        let negative = Place::parse(&key("p:x/area/m/-1/-7/0/e"));
        assert!(matches!(
            negative,
            Some(Place::Area {
                level: -1,
                x: -7,
                ..
            })
        ));
    }

    #[test]
    fn the_kits_own_keys_arent_read_as_constructs() {
        for kit in [
            "p:x/head",
            "p:x/piece/m/e",
            "dir/person/anna",
            "meta/format",
        ] {
            assert_eq!(Place::parse(&key(kit)), None, "{kit}");
        }
    }

    #[test]
    fn a_list_holds_the_keys_it_should() {
        let i = |text: &str| Id::parse(text).unwrap();
        let place = Place::MemberOfTeam {
            person: i("a"),
            project: i("p"),
            team: i("t"),
        };
        let (start, end) = list::teams_of(&i("a"), &i("p")).bounds();
        assert!(place.key().as_bytes() >= &start[..] && place.key().as_bytes() < &end[..]);
        let place = Place::HubProject {
            hub: i("h"),
            project: i("p"),
        };
        let (start, end) = list::hub_projects(&i("h")).bounds();
        assert!(place.key().as_bytes() >= &start[..] && place.key().as_bytes() < &end[..]);
    }
}
