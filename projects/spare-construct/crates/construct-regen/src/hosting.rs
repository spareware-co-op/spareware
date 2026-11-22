//! Hosting (CON-A4-F3-P1): how an element hosted on another sits in its record, and where that puts
//! it. A door is hosted on a face of a wall, named by the face's persistent name, never by its
//! place in a list (chapter 20.4), at an offset along that face and up it. The regenerator builds a
//! wall together with the doors hosted on it through this module, and the server's commands check
//! a door against its wall through it, so what is refused and what is built cannot differ.
//!
//! The record (FORMAT.md, "Hosted elements"):
//!
//! - `references` holds one reference of kind [`FACE_KIND`] in this document: the host element,
//!   the face's persistent name, `want` `carrier` (an opening cut through the face does not break
//!   what is hosted on it), and the face's role as the `signature`, kept only to suggest a repair.
//! - `props` holds [`ALONG_KEY`] and [`UP_KEY`]: its offset along the face and up it, each a
//!   construct-params stored length (`length:q0:<steps>`).
//! - `placement` is where it sat when a command last wrote it. Where it sits now is read from its
//!   host ([`at`], [`in_model`]): a door moves with its wall, and its record is not rewritten.
//!
//! Where an offset puts it, in the host's frame (the frame its shape is built in): along the
//! wall's axis from its start (`end0`), whichever side it is on; up from its base; across the
//! wall, flush with the side it is hosted on, so a door as thick as its wall fills it. A wall's
//! exterior is the side on its right walking it from start to end (construct-regen's builders).
//! The opening it cuts is the builder's to decide (the kernel's `opening`): its width and height
//! through the wall's whole thickness.
//!
//! An older record, written before hosting was stored this way, is a `door.single` with a face
//! reference and no offsets ([`Hosting::ByPosition`]). It finds its host by that reference and sits
//! where its own placement puts it: its place in its host is the difference of the two placements.
//! It cuts its host as a new one does, but stays where it was placed when its host moves; a
//! `hosted.move` writes its offsets, and from then on it moves with its host. Why: the decision
//! note `projects/spare-construct/notes/CON-A4-F3-P1-hosting.md`.
//!
//! Depends on construct-model (records), construct-params (lengths and their stored text) and
//! this crate's builders' face names ([`crate::face_names`]). Interim: only a `wall.straight`
//! hosts, only on its two sides, and a hosted element sits flush with its side; a family that
//! declares its host and the void it cuts (job 6.4, CON-A7-F3-P2) replaces these fixed rules.

use construct_model::element::{Element, Placement, Reference, Want};
use construct_model::geometry::{Edition, FaceName, GeometryError, Recipe};
use construct_params::{Length, Value};
use kit_core::Id;

/// The `props` key of a hosted element's offset along its host's face.
pub const ALONG_KEY: &str = "@host.along";
/// The `props` key of a hosted element's offset up its host's face.
pub const UP_KEY: &str = "@host.up";
/// The kind of reference a hosted element names its host's face by.
pub const FACE_KIND: &str = "face";
/// The one operation that hosts: a straight wall.
pub const HOST_OP: &str = "wall.straight";
/// The faces of a host an element may sit on, by role.
pub const SIDES: [&str; 2] = ["exterior", "interior"];
/// The operation of an older record that is hosted by position.
const OPENING_OP: &str = "door.single";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// How an element is hosted, as its record says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hosting {
    /// On a face of its host at an offset along it and up it: it moves with its host.
    OnFace {
        /// The host element.
        host: Id,
        /// The host's face, by its persistent name.
        face: String,
        /// How far along the face from the host's start.
        along: Length,
        /// How far up the face from the host's base.
        up: Length,
    },
    /// An older record: a door with a face reference and no offsets, placed by its position.
    ByPosition {
        /// The host element.
        host: Id,
        /// The host's face, by its persistent name.
        face: String,
    },
}

impl Hosting {
    /// The host element.
    #[must_use]
    pub fn host(&self) -> &Id {
        match self {
            Hosting::OnFace { host, .. } | Hosting::ByPosition { host, .. } => host,
        }
    }

    /// The host's face, by its persistent name.
    #[must_use]
    pub fn face(&self) -> &str {
        match self {
            Hosting::OnFace { face, .. } | Hosting::ByPosition { face, .. } => face,
        }
    }
}

/// How `element` is hosted, if it is.
///
/// # Errors
///
/// Why its record does not read as hosted, in words: one offset without the other, an offset that
/// is not a stored length, or offsets with no face, or more than one, to sit on.
pub fn read(element: &Element) -> Result<Option<Hosting>, String> {
    let faces: Vec<&Reference> = element
        .references
        .iter()
        .filter(|r| r.kind == FACE_KIND && r.doc.is_none())
        .collect();
    let name = &element.name;
    let (along, up) = (element.props.get(ALONG_KEY), element.props.get(UP_KEY));
    if along.is_none() && up.is_none() {
        let opening = element.recipe.owner == OWN && element.recipe.op == OPENING_OP;
        return Ok(match faces.first() {
            Some(face) if opening => Some(Hosting::ByPosition {
                host: face.element.clone(),
                face: face.name.clone(),
            }),
            _ => None,
        });
    }
    let (Some(along), Some(up)) = (along, up) else {
        return Err(format!("{name}: its offset on its host is half written"));
    };
    let [face] = faces.as_slice() else {
        return Err(format!(
            "{name}: a hosted element names one face of its host, not {}",
            faces.len()
        ));
    };
    let length = |key: &str, text: &str| match Value::from_stored(text) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(format!("{name}: {key} is {text}, not a stored length")),
    };
    Ok(Some(Hosting::OnFace {
        host: face.element.clone(),
        face: face.name.clone(),
        along: length(ALONG_KEY, along)?,
        up: length(UP_KEY, up)?,
    }))
}

/// The element `element` is hosted on, if its record reads as hosted.
#[must_use]
pub fn host_of(element: &Element) -> Option<Id> {
    read(element)
        .ok()
        .flatten()
        .map(|hosting| hosting.host().clone())
}

/// Writes `element` as hosted on `face` of `host` at `along` and `up`: its one face reference and
/// its two offsets. Its other references and `props` stay.
pub fn write(element: &mut Element, host: &Id, face: &FaceName, along: Length, up: Length) {
    element
        .references
        .retain(|r| !(r.kind == FACE_KIND && r.doc.is_none()));
    element.references.push(Reference {
        doc: None,
        element: host.clone(),
        name: face.name.clone(),
        kind: FACE_KIND.to_owned(),
        want: Want::Carrier,
        signature: Some(face.role.clone()),
    });
    element
        .props
        .insert(ALONG_KEY.to_owned(), Value::Length(along).stored());
    element
        .props
        .insert(UP_KEY.to_owned(), Value::Length(up).stored());
}

/// The face of `host` named `face`, under `edition`, if it is a side an element may sit on.
///
/// # Errors
///
/// Why not, in words: the host's faces cannot be named, or that face is not one of its sides.
pub fn side(edition: &Edition, host: &Recipe, face: &str) -> Result<FaceName, String> {
    let names = crate::face_names(edition, host).map_err(|e| e.message)?;
    names
        .into_iter()
        .find(|n| n.name == face && SIDES.contains(&n.role.as_str()))
        .ok_or_else(|| {
            format!(
                "the face {face} is not one of its sides; an element sits on a wall's {} or its {}",
                SIDES[0], SIDES[1]
            )
        })
}

/// Where `hosted` sits in `host`'s frame, in whole millimetres: what a builder is handed with the
/// host. `host_recipe` and `hosted_recipe` are their recipes as built, each parameter filled, and
/// `edition` is the document's, which names the host's faces.
///
/// # Errors
///
/// A [`GeometryError`] naming the hosted element: `NotAHost` (only a `wall.straight` hosts on a
/// face), `NotASide` (its face is not one of its host's sides), `MissingArgument` (a thickness
/// that is not whole millimetres), `NotWholeMillimetres` (an offset finer than a placement holds),
/// `TooBig` (beyond a 64-bit number of millimetres).
pub fn at(
    edition: &Edition,
    host: &Element,
    host_recipe: &Recipe,
    hosted: &Element,
    hosted_recipe: &Recipe,
    hosting: &Hosting,
) -> Result<Placement, GeometryError> {
    let key = hosted.id.as_str();
    let refuse = |code: &str, why: String| GeometryError {
        code: code.to_owned(),
        message: format!("door {key}: {why}"),
    };
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let (along, up, face) = match hosting {
        Hosting::ByPosition { .. } => {
            let less = |a: i64, b: i64| a.checked_sub(b).ok_or_else(too_big);
            let (own, host) = (hosted.placement, host.placement);
            return Ok(Placement {
                x: less(own.x, host.x)?,
                y: less(own.y, host.y)?,
                z: less(own.z, host.z)?,
            });
        }
        Hosting::OnFace {
            along, up, face, ..
        } => (*along, *up, face),
    };
    if host_recipe.owner != OWN || host_recipe.op != HOST_OP {
        return Err(refuse(
            "NotAHost",
            format!(
                "{} is a {}, not a wall: only a {HOST_OP} hosts",
                host.name, host_recipe.op
            ),
        ));
    }
    let role = side(edition, host_recipe, face).map_err(|why| refuse("NotASide", why))?;
    let whole = |recipe: &Recipe, arg: &str| {
        recipe
            .args
            .get(arg)
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                refuse(
                    "MissingArgument",
                    format!("{arg} must be a whole number of millimetres"),
                )
            })
    };
    let (thickness, own) = (
        whole(host_recipe, "thickness")?,
        whole(hosted_recipe, "thickness")?,
    );
    let mm = |what: &str, length: Length| {
        length.whole_mm().ok_or_else(|| {
            refuse(
                "NotWholeMillimetres",
                format!(
                    "its offset {what} its host's face, {length}, is not a whole number of \
                     millimetres, as every placement is"
                ),
            )
        })
    };
    let (along, up) = (mm("along", along)?, mm("up", up)?);
    let flush = thickness.checked_sub(own).ok_or_else(too_big)?;
    let along_y = host_recipe
        .args
        .get("axis")
        .and_then(serde_json::Value::as_str)
        == Some("y");
    // Along x the exterior is the side at y = 0; along y, the side at x = the wall's thickness.
    let across = match (role.role.as_str(), along_y) {
        ("exterior", false) | ("interior", true) => 0,
        _ => flush,
    };
    Ok(if along_y {
        Placement {
            x: across,
            y: along,
            z: up,
        }
    } else {
        Placement {
            x: along,
            y: across,
            z: up,
        }
    })
}

/// Where an element at `at` in the frame of a host placed at `host` sits in the model.
///
/// # Errors
///
/// `TooBig` when that is beyond a 64-bit number of millimetres.
pub fn in_model(host: Placement, at: Placement) -> Result<Placement, GeometryError> {
    let add = |a: i64, b: i64| {
        a.checked_add(b).ok_or_else(|| GeometryError {
            code: "TooBig".to_owned(),
            message: "a hosted element is beyond the model's reach".to_owned(),
        })
    };
    Ok(Placement {
        x: add(host.x, at.x)?,
        y: add(host.y, at.y)?,
        z: add(host.z, at.z)?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use crate::stand_in::Boxes;
    use construct_model::geometry::Geometry;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn i(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    fn element(id: &str, op: &str, args: serde_json::Value, at: [i64; 3]) -> Element {
        Element {
            id: i(id),
            document: i("doc"),
            kind: "door".to_owned(),
            name: id.to_owned(),
            level: 1,
            placement: Placement {
                x: at[0],
                y: at[1],
                z: at[2],
            },
            recipe: Recipe::construct(op, serde_json::from_value(args).unwrap()),
            props: BTreeMap::new(),
            references: Vec::new(),
        }
    }

    fn wall(axis: &str) -> Element {
        let args = json!({ "length": 4000, "thickness": 200, "height": 2700, "axis": axis });
        element("W-1", "wall.straight", args, [1000, 2000, 0])
    }

    fn door() -> Element {
        let args = json!({ "width": 900, "thickness": 100, "height": 2100 });
        element("D-1", "door.single", args, [0, 0, 0])
    }

    fn face(role: &str) -> FaceName {
        crate::stand_in::names(&wall("x").recipe)
            .unwrap()
            .into_iter()
            .find(|n| n.role == role)
            .unwrap()
    }

    /// A hosted element's record holds its host's face and its offsets, and reads back as written;
    /// writing it again replaces its face reference and keeps everything else it holds.
    #[test]
    fn a_hosted_record_reads_back_as_written() {
        let mut d = door();
        d.props.insert("note".to_owned(), "kept".to_owned());
        let along = Length::from_steps(80_000);
        write(
            &mut d,
            &i("W-1"),
            &face("exterior"),
            along,
            Length::from_mm(0),
        );
        assert_eq!(d.props[ALONG_KEY], "length:q0:80000");
        assert_eq!(d.props[UP_KEY], "length:q0:0");
        assert_eq!(d.references.len(), 1);
        assert_eq!(d.references[0].name, "box.exterior");
        assert_eq!(d.references[0].kind, "face");
        assert_eq!(d.references[0].want, Want::Carrier);
        assert_eq!(d.references[0].signature.as_deref(), Some("exterior"));
        let read_back = read(&d).unwrap().unwrap();
        assert_eq!(
            read_back,
            Hosting::OnFace {
                host: i("W-1"),
                face: "box.exterior".to_owned(),
                along,
                up: Length::from_mm(0),
            }
        );
        write(&mut d, &i("W-2"), &face("interior"), along, along);
        assert_eq!(d.references.len(), 1);
        assert_eq!(host_of(&d), Some(i("W-2")));
        assert_eq!(d.props["note"], "kept");
    }

    /// An older door, a face reference and no offsets, finds its host by that reference and sits by
    /// its position; anything else with no offsets is not hosted; a record half written does not read.
    #[test]
    fn an_older_door_is_hosted_by_its_reference_and_its_position() {
        let mut d = door();
        assert_eq!(read(&d).unwrap(), None);
        d.references.push(Reference {
            doc: None,
            element: i("W-1"),
            name: "box.exterior".to_owned(),
            kind: "face".to_owned(),
            want: Want::Carrier,
            signature: None,
        });
        assert_eq!(
            read(&d).unwrap(),
            Some(Hosting::ByPosition {
                host: i("W-1"),
                face: "box.exterior".to_owned(),
            })
        );
        let mut block = d.clone();
        block.recipe.op = "block".to_owned();
        assert_eq!(read(&block).unwrap(), None);
        let mut half = d.clone();
        half.props
            .insert(ALONG_KEY.to_owned(), "length:q0:0".to_owned());
        assert!(read(&half).unwrap_err().contains("half written"));
        half.props.insert(UP_KEY.to_owned(), "900".to_owned());
        assert!(read(&half).unwrap_err().contains("not a stored length"));
        half.references.clear();
        half.props
            .insert(UP_KEY.to_owned(), "length:q0:0".to_owned());
        assert!(read(&half).unwrap_err().contains("not 0"));
        assert_eq!(host_of(&half), None);
    }

    /// An offset puts a door along its wall from the wall's start and up from its base, flush with
    /// the side it is on: along x the exterior is at y = 0, along y at x = the wall's thickness.
    /// An older door's place is the difference of the two placements.
    #[test]
    fn an_offset_puts_a_door_flush_with_its_side() {
        let edition = Boxes.edition();
        let (along, up) = (Length::from_mm(1200), Length::from_mm(300));
        for (axis, role, expected) in [
            ("x", "exterior", [1200, 0, 300]),
            ("x", "interior", [1200, 100, 300]),
            ("y", "exterior", [100, 1200, 300]),
            ("y", "interior", [0, 1200, 300]),
        ] {
            let host = wall(axis);
            let mut d = door();
            write(&mut d, &host.id, &face(role), along, up);
            let hosting = read(&d).unwrap().unwrap();
            let got = at(&edition, &host, &host.recipe, &d, &d.recipe, &hosting).unwrap();
            assert_eq!([got.x, got.y, got.z], expected, "{axis} {role}");
            let placed = in_model(host.placement, got).unwrap();
            assert_eq!([placed.x - got.x, placed.y - got.y], [1000, 2000]);
        }
        let host = wall("x");
        let mut old = door();
        old.placement = Placement {
            x: 2500,
            y: 1950,
            z: 0,
        };
        let hosting = Hosting::ByPosition {
            host: host.id.clone(),
            face: "box.exterior".to_owned(),
        };
        let got = at(&edition, &host, &host.recipe, &old, &old.recipe, &hosting).unwrap();
        assert_eq!([got.x, got.y, got.z], [1500, -50, 0]);
    }

    /// Each case where an offset puts nothing anywhere is a failure naming the door and why.
    #[test]
    fn an_offset_that_puts_nothing_says_why() {
        let edition = Boxes.edition();
        let host = wall("x");
        let mut d = door();
        write(
            &mut d,
            &host.id,
            &face("exterior"),
            Length::from_steps(80_000),
            Length::from_mm(0),
        );
        let hosting = read(&d).unwrap().unwrap();
        let code = |host: &Element, hosting: &Hosting| {
            at(&edition, host, &host.recipe, &d, &d.recipe, hosting).unwrap_err()
        };
        let fine = code(&host, &hosting);
        assert_eq!(fine.code, "NotWholeMillimetres");
        assert!(fine.message.starts_with("door D-1: "), "{}", fine.message);
        let mut whole = d.clone();
        write(
            &mut whole,
            &host.id,
            &face("top"),
            Length::from_mm(1),
            Length::from_mm(0),
        );
        let top = read(&whole).unwrap().unwrap();
        assert_eq!(
            at(&edition, &host, &host.recipe, &whole, &whole.recipe, &top)
                .unwrap_err()
                .code,
            "NotASide"
        );
        let mut block = host.clone();
        block.recipe.op = "block".to_owned();
        assert_eq!(code(&block, &hosting).code, "NotAHost");
        let other = Edition {
            engine: "elsewhere".to_owned(),
            ..edition.clone()
        };
        let named = at(
            &other,
            &host,
            &host.recipe,
            &whole,
            &whole.recipe,
            &read(&whole).unwrap().unwrap(),
        );
        assert_eq!(named.unwrap_err().code, "NotASide");
        let far = Placement {
            x: i64::MAX,
            y: 0,
            z: 0,
        };
        assert_eq!(in_model(far, far).unwrap_err().code, "TooBig");
    }
}
