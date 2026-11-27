//! A railing's type: what it is made of, as a person sets it. A top rail by profile and height; an
//! optional handrail by profile, height and how far it stands in from the railing's line; balusters
//! by profile, spacing, justification and how far their foot stands above the floor (or, on a
//! flight, the pitch line); posts by profile, at the start, the end and every corner; and the
//! material all of it is drawn in.
//!
//! - **Profiles** ([`Profile`]): a rectangle `width` across the path by `depth` (a rail's depth is
//!   upright, a baluster's or a post's along the path), or round by its diameter. A round profile
//!   is built as a twelve-sided polygon whose corners are on its circle (interim: the engine's
//!   extrusion takes polygons; the kernel's own swept circle, chapter 13.6, replaces it).
//! - **Heights** are measured upright to the top of the rail, from the floor or a landing, and on a
//!   flight from the pitch line (F4/AS1 Table 1, Note 1; D1/AS1 6.0.6 for a handrail). A rail's
//!   section square to it is its profile, on a slope too.
//! - **Balusters** are laid on each piece of path between two posts, `spacing` apart centre to
//!   centre, measured in plan along the path ([`Justify`]: from the start, centred, or from the end).
//!
//! Stored ([`RailingType::to_json`], [`RailingType::from_stored`]) in construct-params' stored
//! length spelling; a person's typed text is read by [`RailingType::patched`]. Each check is
//! [`RailingType::check`]'s.

use crate::error::RailingError;
use construct_levels::parse_length;
use construct_materials::{MaterialId, Materials};
use construct_params::{Length, Units, Value};
use serde_json::{Map, Value as Json, json};

/// The largest a profile may be across or deep: 500 mm.
pub const MOST_PROFILE: Length = Length::from_mm(500);
/// The highest a rail may stand: 3 m.
pub const MOST_HEIGHT: Length = Length::from_mm(3000);
/// The widest baluster spacing: 10 m.
pub const MOST_SPACING: Length = Length::from_mm(10_000);
/// The farthest a handrail stands in from the railing's line, and a baluster's foot above the
/// floor: 1 m.
pub const MOST_OFFSET: Length = Length::from_mm(1000);
/// The material the standard type is drawn in: the default library's stainless steel.
pub const STANDARD_MATERIAL: &str = "steel-stainless-aisi-301";

/// A profile, as the module header says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// A rectangle.
    Rectangle {
        /// Across the path.
        width: Length,
        /// Upright for a rail; along the path for a baluster or a post.
        depth: Length,
    },
    /// Round.
    Round {
        /// Its diameter.
        diameter: Length,
    },
}

impl Profile {
    /// How wide it is across the path.
    #[must_use]
    pub fn across(self) -> Length {
        match self {
            Self::Rectangle { width, .. } => width,
            Self::Round { diameter } => diameter,
        }
    }

    /// How deep it is: upright for a rail, along the path for a baluster or a post.
    #[must_use]
    pub fn depth(self) -> Length {
        match self {
            Self::Rectangle { depth, .. } => depth,
            Self::Round { diameter } => diameter,
        }
    }

    fn check(self, of: &'static str) -> Result<(), RailingError> {
        for size in [self.across(), self.depth()] {
            if size.steps() <= 0 {
                return Err(RailingError::NotPositive { what: of });
            }
            if size.steps() > MOST_PROFILE.steps() {
                return Err(RailingError::OutOfRange {
                    what: format!("{of}, {size}"),
                });
            }
        }
        Ok(())
    }

    fn to_json(self) -> Json {
        let s = |l: Length| Value::Length(l).stored();
        match self {
            Self::Rectangle { width, depth } => json!({ "rectangle": [s(width), s(depth)] }),
            Self::Round { diameter } => json!({ "round": s(diameter) }),
        }
    }

    fn read(v: &Json, length: &Reader<'_>) -> Result<Self, RailingError> {
        let object = v
            .as_object()
            .filter(|o| o.len() == 1)
            .ok_or_else(|| bad("profile is {\"rectangle\":[w,d]} or {\"round\":d}"))?;
        if let Some(sizes) = object.get("rectangle") {
            return match sizes.as_array().map(Vec::as_slice) {
                Some([w, d]) => Ok(Self::Rectangle {
                    width: length(w, "a profile's width")?,
                    depth: length(d, "a profile's depth")?,
                }),
                _ => Err(bad("rectangle is two lengths, its width and depth")),
            };
        }
        match object.get("round") {
            Some(d) => Ok(Self::Round {
                diameter: length(d, "a profile's diameter")?,
            }),
            None => Err(bad("profile is {\"rectangle\":[w,d]} or {\"round\":d}")),
        }
    }
}

/// Where the balusters of a piece of path start from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    /// The first is one spacing from the post at the piece's start.
    Start,
    /// They are centred between the two posts, the gaps at both ends alike.
    Centre,
    /// The last is one spacing from the post at the piece's end.
    End,
}

impl Justify {
    /// Every justification, in a fixed order.
    pub const ALL: [Self; 3] = [Self::Start, Self::Centre, Self::End];

    /// Its stored name: `start`, `centre` or `end`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Centre => "centre",
            Self::End => "end",
        }
    }

    /// The justification of a stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|j| j.name() == name)
    }
}

/// The top rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rail {
    /// Its section.
    pub profile: Profile,
    /// Its top's height.
    pub height: Length,
}

/// The handrail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handrail {
    /// Its section.
    pub profile: Profile,
    /// Its top's height.
    pub height: Length,
    /// How far its centre stands in from the railing's line, toward the side it is used from.
    pub offset: Length,
}

/// The balusters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Balusters {
    /// Each one's section in plan.
    pub profile: Profile,
    /// Centre to centre, in plan along the path.
    pub spacing: Length,
    /// Where they start from on each piece of path.
    pub justify: Justify,
    /// How far each one's foot stands above the floor, or on a flight above the pitch line.
    pub bottom: Length,
}

/// A railing's type, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailingType {
    /// The top rail.
    pub top_rail: Rail,
    /// The handrail, if it has one.
    pub handrail: Option<Handrail>,
    /// The balusters.
    pub balusters: Balusters,
    /// Each post's section in plan.
    pub posts: Profile,
    /// The material every part is drawn in: an ID of the default library.
    pub material: String,
}

/// Reads one length from a JSON value, naming what it is.
type Reader<'a> = dyn Fn(&Json, &str) -> Result<Length, RailingError> + 'a;

fn bad(why: &str) -> RailingError {
    RailingError::Malformed(format!("A railing's type: {why}."))
}

/// A stored length: construct-params' one spelling.
fn stored(v: &Json, what: &str) -> Result<Length, RailingError> {
    v.as_str()
        .and_then(Value::from_stored)
        .and_then(|value| match value {
            Value::Length(l) => Some(l),
            _ => None,
        })
        .ok_or_else(|| bad(&format!("{what} is not a stored length")))
}

/// A length as a person types it.
fn typed(units: &Units) -> impl Fn(&Json, &str) -> Result<Length, RailingError> + '_ {
    move |v: &Json, what: &str| {
        let text = v
            .as_str()
            .ok_or_else(|| bad(&format!("{what} is not text")))?;
        parse_length(text, units)
            .map_err(|e| RailingError::Malformed(format!("A railing's type: {what}: {e}")))
    }
}

/// The fields of a JSON object, none but `allowed`.
fn fields<'a>(
    v: &'a Json,
    of: &str,
    allowed: &[&str],
) -> Result<&'a Map<String, Json>, RailingError> {
    let object = v
        .as_object()
        .ok_or_else(|| bad(&format!("{of} is an object")))?;
    if let Some(other) = object.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(bad(&format!("{of} has no field {other:?}")));
    }
    Ok(object)
}

fn within(
    length: Length,
    most: Length,
    what: &'static str,
    positive: bool,
) -> Result<(), RailingError> {
    if positive && length.steps() <= 0 {
        return Err(RailingError::NotPositive { what });
    }
    if length.steps() < 0 {
        return Err(RailingError::Negative { what });
    }
    if length.steps() > most.steps() {
        return Err(RailingError::OutOfRange {
            what: format!("{what}, {length}"),
        });
    }
    Ok(())
}

impl RailingType {
    /// The type a railing is given when none is: our own design. A 50 by 50 mm top rail at
    /// 1000 mm, no handrail, 20 by 20 mm balusters at 110 mm centred with their foot 50 mm up (so
    /// 90 mm clear between them), 50 by 50 mm posts, in stainless steel.
    #[must_use]
    pub fn standard() -> Self {
        let square = |mm: i32| Profile::Rectangle {
            width: Length::from_mm(mm),
            depth: Length::from_mm(mm),
        };
        Self {
            top_rail: Rail {
                profile: square(50),
                height: Length::from_mm(1000),
            },
            handrail: None,
            balusters: Balusters {
                profile: square(20),
                spacing: Length::from_mm(110),
                justify: Justify::Centre,
                bottom: Length::from_mm(50),
            },
            posts: square(50),
            material: STANDARD_MATERIAL.to_owned(),
        }
    }

    /// Every check of the type's own shape: every profile more than nothing and at most
    /// [`MOST_PROFILE`]; every height more than nothing and at most [`MOST_HEIGHT`]; the spacing
    /// more than nothing, at most [`MOST_SPACING`] and at least the balusters' depth along the
    /// path; the handrail's offset and the balusters' foot at least nothing and at most
    /// [`MOST_OFFSET`]; the material one of the default library's.
    ///
    /// # Errors
    /// The refusal of the first check that fails, in that order.
    pub fn check(&self) -> Result<(), RailingError> {
        self.top_rail.profile.check("top rail's profile")?;
        self.balusters.profile.check("balusters' profile")?;
        self.posts.check("posts' profile")?;
        if let Some(h) = &self.handrail {
            h.profile.check("handrail's profile")?;
            within(h.height, MOST_HEIGHT, "handrail's height", true)?;
            within(h.offset, MOST_OFFSET, "handrail's offset", false)?;
        }
        within(self.top_rail.height, MOST_HEIGHT, "top rail's height", true)?;
        let b = &self.balusters;
        within(b.spacing, MOST_SPACING, "baluster spacing", true)?;
        within(b.bottom, MOST_OFFSET, "balusters' foot", false)?;
        if b.profile.depth().steps() > b.spacing.steps() {
            return Err(RailingError::Crowded {
                spacing: b.spacing,
                depth: b.profile.depth(),
            });
        }
        let id = MaterialId::parse(&self.material)
            .map_err(|_| RailingError::UnknownMaterial(self.material.clone()))?;
        if Materials::default_metric().get(&id).is_none() {
            return Err(RailingError::UnknownMaterial(self.material.clone()));
        }
        Ok(())
    }

    /// Its stored form, as FORMAT.md writes it.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let s = |l: Length| Value::Length(l).stored();
        let mut out = Map::new();
        out.insert(
            "balusters".to_owned(),
            json!({
                "bottom": s(self.balusters.bottom),
                "justify": self.balusters.justify.name(),
                "profile": self.balusters.profile.to_json(),
                "spacing": s(self.balusters.spacing),
            }),
        );
        if let Some(h) = &self.handrail {
            out.insert(
                "handrail".to_owned(),
                json!({ "height": s(h.height), "offset": s(h.offset), "profile": h.profile.to_json() }),
            );
        }
        out.insert("material".to_owned(), json!(self.material));
        out.insert("posts".to_owned(), self.posts.to_json());
        out.insert(
            "top_rail".to_owned(),
            json!({ "height": s(self.top_rail.height), "profile": self.top_rail.profile.to_json() }),
        );
        Json::Object(out)
    }

    /// A type from its stored form, read strictly: as [`RailingType::to_json`] writes it, every
    /// check passed, and the same written back.
    ///
    /// # Errors
    /// [`RailingError::Malformed`] for a field missing or not in its one spelling; any refusal of
    /// [`RailingType::check`].
    pub fn from_stored(v: &Json) -> Result<Self, RailingError> {
        let kind = Self::read(v, &stored, None)?;
        if kind.to_json() != *v {
            return Err(bad("it is not in its one spelling"));
        }
        Ok(kind)
    }

    /// This type with the fields `patch` gives, in a person's typed text, in place of its own:
    /// any of `top_rail` (`profile`, `height`), `handrail` (`profile`, `height`, `offset`; or
    /// `null` for none), `balusters` (`profile`, `spacing`, `justify`, `bottom`), `posts` (a
    /// profile) and `material`. A new handrail gives its profile and height; its offset is nothing
    /// unless given.
    ///
    /// # Errors
    /// [`RailingError::Malformed`] for a field that does not read; any refusal of
    /// [`RailingType::check`].
    pub fn patched(&self, patch: &Json, units: &Units) -> Result<Self, RailingError> {
        Self::read(patch, &typed(units), Some(self))
    }

    /// Reads a type, each length by `length`, over `base` where a field is not given (none: every
    /// field must be).
    #[allow(
        clippy::too_many_lines,
        reason = "every field of a type in the stored form's order, each over its base"
    )]
    fn read(v: &Json, length: &Reader<'_>, base: Option<&Self>) -> Result<Self, RailingError> {
        let top = fields(
            v,
            "the type",
            &["top_rail", "handrail", "balusters", "posts", "material"],
        )?;
        let missing = |what: &str| bad(&format!("{what} is missing"));
        let profile =
            |o: &Map<String, Json>, held: Option<Profile>, what: &str| match o.get("profile") {
                Some(p) => Profile::read(p, length),
                None => held.ok_or_else(|| missing(what)),
            };
        let len =
            |o: &Map<String, Json>, key: &str, held: Option<Length>, what: &str| match o.get(key) {
                Some(l) => length(l, what),
                None => held.ok_or_else(|| missing(what)),
            };
        let empty = Map::new();
        let rail = match top.get("top_rail") {
            Some(r) => fields(r, "top_rail", &["profile", "height"])?,
            None if base.is_some() => &empty,
            None => return Err(missing("top_rail")),
        };
        let top_rail = Rail {
            profile: profile(
                rail,
                base.map(|b| b.top_rail.profile),
                "the top rail's profile",
            )?,
            height: len(
                rail,
                "height",
                base.map(|b| b.top_rail.height),
                "the top rail's height",
            )?,
        };
        let held_handrail = base.and_then(|b| b.handrail);
        let handrail = match top.get("handrail") {
            Some(Json::Null) if base.is_some() => None,
            Some(h) => {
                let h = fields(h, "handrail", &["profile", "height", "offset"])?;
                Some(Handrail {
                    profile: profile(
                        h,
                        held_handrail.map(|k| k.profile),
                        "the handrail's profile",
                    )?,
                    height: len(
                        h,
                        "height",
                        held_handrail.map(|k| k.height),
                        "the handrail's height",
                    )?,
                    offset: len(
                        h,
                        "offset",
                        Some(held_handrail.map_or(Length::from_steps(0), |k| k.offset)),
                        "the handrail's offset",
                    )?,
                })
            }
            None => held_handrail,
        };
        let held = base.map(|b| b.balusters);
        let bal = match top.get("balusters") {
            Some(b) => fields(b, "balusters", &["profile", "spacing", "justify", "bottom"])?,
            None if base.is_some() => &empty,
            None => return Err(missing("balusters")),
        };
        let justify = match bal.get("justify") {
            Some(j) => j
                .as_str()
                .and_then(Justify::from_name)
                .ok_or_else(|| bad("the balusters are justified start, centre or end"))?,
            None => held.map(|b| b.justify).ok_or_else(|| missing("justify"))?,
        };
        let balusters = Balusters {
            profile: profile(bal, held.map(|b| b.profile), "the balusters' profile")?,
            spacing: len(
                bal,
                "spacing",
                held.map(|b| b.spacing),
                "the baluster spacing",
            )?,
            justify,
            bottom: len(bal, "bottom", held.map(|b| b.bottom), "the balusters' foot")?,
        };
        let posts = match top.get("posts") {
            Some(p) => Profile::read(p, length)?,
            None => base.map(|b| b.posts).ok_or_else(|| missing("posts"))?,
        };
        let material = match top.get("material") {
            Some(m) => m
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad("material is text"))?,
            None => base
                .map(|b| b.material.clone())
                .ok_or_else(|| missing("material"))?,
        };
        let kind = Self {
            top_rail,
            handrail,
            balusters,
            posts,
            material,
        };
        kind.check()?;
        Ok(kind)
    }
}
