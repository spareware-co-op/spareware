//! A field: one column a schedule can show, filter on or sort by, and what a category has of them.
//!
//! A field is one of four things, each with one stored spelling ([`Field::key`]):
//! - `name`: the element's own name, text;
//! - `type`: the name of the element's type, text, empty when it has none;
//! - `parameter:<id>`: a parameter by its definition's permanent ID, of any kind;
//! - `quantity:<name>`: a read-only quantity the product works out of an element and never
//!   stores, by the name it shows it under (`Area`, `Volume`...). [`QUANTITIES`] lists them.
//!
//! What a category has ([`Basis::definition`]) is decided by the document: a parameter is there
//! when its binding gives it to the category (construct-params' `parameters_of`), a quantity when
//! this crate's table gives it to the category's top-level category, and `name` and `type` are
//! there for every category.

use crate::error::ScheduleError;
use construct_categories::{CategoryId, CategoryKind, CategoryTable, FLOORS, ROOFS, ROOMS, WALLS};
use construct_params::{Definition, Holder, Kind, ProjectParams};
use kit_core::Id;

/// The ID of the definition `name` is read through.
const NAME_ID: &str = "element.name";
/// The ID of the definition `type` is read through.
const TYPE_ID: &str = "element.type";

/// One column of a schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// The element's name.
    Name,
    /// The name of the element's type.
    Type,
    /// A parameter, by its definition's ID.
    Parameter(Id),
    /// A read-only quantity, by its name.
    Quantity(String),
}

impl Field {
    /// The field's one stored spelling.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Name => "name".to_owned(),
            Self::Type => "type".to_owned(),
            Self::Parameter(id) => format!("parameter:{id}"),
            Self::Quantity(name) => format!("quantity:{name}"),
        }
    }

    /// The field a key spells. Only a key in its one spelling reads.
    ///
    /// # Errors
    /// [`ScheduleError::BadField`] for anything else.
    pub fn parse(key: &str) -> Result<Self, ScheduleError> {
        let bad = || ScheduleError::BadField(key.to_owned());
        let field = match key {
            "name" => Self::Name,
            "type" => Self::Type,
            _ => {
                if let Some(id) = key.strip_prefix("parameter:") {
                    Self::Parameter(Id::parse(id).map_err(|_| bad())?)
                } else if let Some(name) = key.strip_prefix("quantity:") {
                    Self::Quantity(name.to_owned())
                } else {
                    return Err(bad());
                }
            }
        };
        if field.key() == key && !matches!(&field, Self::Quantity(name) if name.is_empty()) {
            Ok(field)
        } else {
            Err(bad())
        }
    }
}

/// A read-only quantity a category has: the ID the product's own rows carry it under, the name
/// they show it by, and the kind of value it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantity {
    /// The top-level category whose elements have it.
    pub category: CategoryId,
    /// The ID of the definition the quantity is worked out under.
    pub id: &'static str,
    /// The name it is shown and chosen by.
    pub name: &'static str,
    /// What its value is.
    pub kind: Kind,
}

const fn quantity(
    category: CategoryId,
    id: &'static str,
    name: &'static str,
    kind: Kind,
) -> Quantity {
    Quantity {
        category,
        id,
        name,
        kind,
    }
}

/// The read-only quantities of the built-in tools' elements, by category, each a constant. The
/// names and kinds are those the
/// server's own read-only rows carry (construct-slabs, construct-roofs and construct-regen's
/// rooms and walls), pinned by a test in construct-api that reads each element kind and compares.
///
/// A wall's are its reference line's length, its one-face area and its volume, openings taken out
/// and joints made (construct-regen's `wall_measure`).
///
/// Not here: a roof's pitch (its kind is a slope or an angle by how the roof is set, which a
/// field's kind cannot follow) and a floor's, roof's or wall's per-layer volumes and masses
/// (their number follows the type's layers).
/// A wall's reference line's length.
pub const WALL_LENGTH: Quantity = quantity(WALLS, "wall.length", "Length", Kind::Length);
/// A wall's one-face area, openings taken out.
pub const WALL_AREA: Quantity = quantity(WALLS, "wall.area", "Area", Kind::Area);
/// A wall's volume, openings taken out and joints made.
pub const WALL_VOLUME: Quantity = quantity(WALLS, "wall.volume", "Volume", Kind::Volume);
/// A floor's area.
pub const FLOOR_AREA: Quantity = quantity(FLOORS, "floor.area", "Area", Kind::Area);
/// A floor's perimeter.
pub const FLOOR_PERIMETER: Quantity =
    quantity(FLOORS, "floor.perimeter", "Perimeter", Kind::Length);
/// A floor's thickness.
pub const FLOOR_THICKNESS: Quantity =
    quantity(FLOORS, "floor.thickness", "Thickness", Kind::Length);
/// A floor's volume.
pub const FLOOR_VOLUME: Quantity = quantity(FLOORS, "floor.volume", "Volume", Kind::Volume);
/// A floor's mass in kilograms.
pub const FLOOR_MASS: Quantity = quantity(FLOORS, "floor.mass", "Mass (kg)", Kind::Number);
/// A roof's area.
pub const ROOF_AREA: Quantity = quantity(ROOFS, "roof.area", "Area", Kind::Area);
/// A roof's area along its slopes.
pub const ROOF_SLOPE_AREA: Quantity = quantity(ROOFS, "roof.slope-area", "Slope area", Kind::Area);
/// A roof's thickness.
pub const ROOF_THICKNESS: Quantity = quantity(ROOFS, "roof.thickness", "Thickness", Kind::Length);
/// A roof's volume.
pub const ROOF_VOLUME: Quantity = quantity(ROOFS, "roof.volume", "Volume", Kind::Volume);
/// A roof's mass in kilograms.
pub const ROOF_MASS: Quantity = quantity(ROOFS, "roof.mass", "Mass (kg)", Kind::Number);
/// A roof's ridge height.
pub const ROOF_RIDGE_HEIGHT: Quantity =
    quantity(ROOFS, "roof.ridge-height", "Ridge height", Kind::Length);
/// What became of a room.
pub const ROOM_STATE: Quantity = quantity(ROOMS, "room.state", "State", Kind::Text);
/// A room's area.
pub const ROOM_AREA: Quantity = quantity(ROOMS, "room.area", "Area", Kind::Area);
/// A room's perimeter.
pub const ROOM_PERIMETER: Quantity = quantity(ROOMS, "room.perimeter", "Perimeter", Kind::Length);
/// A room's volume.
pub const ROOM_VOLUME: Quantity = quantity(ROOMS, "room.volume", "Volume", Kind::Volume);

/// Every quantity above, in this order. A tool registers the ones of its elements as its
/// schedule fields (construct-api's tools, note A96), and a schedule is read against the ones it
/// is handed ([`Basis::definition_among`]); this table is what the plain forms read.
pub const QUANTITIES: [Quantity; 18] = [
    WALL_LENGTH,
    WALL_AREA,
    WALL_VOLUME,
    FLOOR_AREA,
    FLOOR_PERIMETER,
    FLOOR_THICKNESS,
    FLOOR_VOLUME,
    FLOOR_MASS,
    ROOF_AREA,
    ROOF_SLOPE_AREA,
    ROOF_THICKNESS,
    ROOF_VOLUME,
    ROOF_MASS,
    ROOF_RIDGE_HEIGHT,
    ROOM_STATE,
    ROOM_AREA,
    ROOM_PERIMETER,
    ROOM_VOLUME,
];

/// What a document gives a schedule to read its fields against: its categories and its
/// parameters.
#[derive(Clone, Copy, Debug)]
pub struct Basis<'a> {
    /// The category table.
    pub categories: &'a CategoryTable,
    /// The document's parameter table.
    pub params: &'a ProjectParams,
}

fn builtin(id: &str, name: &str) -> Option<Definition> {
    let id = Id::parse(id).ok()?;
    Definition::new(id, name, Kind::Text, Holder::Instance).ok()
}

impl Basis<'_> {
    /// The top-level category a category belongs to: itself when it is one. `None` for a
    /// category the table does not have.
    #[must_use]
    pub fn top(&self, category: CategoryId) -> Option<CategoryId> {
        let found = self.categories.get(category)?;
        Some(found.parent.unwrap_or(found.id))
    }

    /// Whether a schedule may list this category: it exists and holds model elements or datums.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownCategory`] or [`ScheduleError::NotListable`].
    pub fn listable(&self, category: CategoryId) -> Result<(), ScheduleError> {
        let found = self
            .categories
            .get(category)
            .ok_or(ScheduleError::UnknownCategory(category))?;
        match found.kind {
            CategoryKind::Model | CategoryKind::Datum => Ok(()),
            _ => Err(ScheduleError::NotListable {
                category: found.name,
            }),
        }
    }

    /// The name of a category, for words; its number when it has none.
    #[must_use]
    pub fn category_name(&self, category: CategoryId) -> String {
        self.categories
            .get(category)
            .map_or_else(|| category.0.to_string(), |found| found.name)
    }

    /// Whether an element of category `element` is listed by a schedule of `listed`: it is in
    /// that category or in one of its subcategories.
    #[must_use]
    pub fn lists(&self, listed: CategoryId, element: CategoryId) -> bool {
        element == listed
            || self
                .categories
                .get(element)
                .is_some_and(|found| found.parent == Some(listed))
    }

    /// The definition a category's field is read, filtered, typed and shown through, or `None`
    /// when the category does not have it, its quantities those of [`QUANTITIES`].
    #[must_use]
    pub fn definition(&self, category: CategoryId, field: &Field) -> Option<Definition> {
        self.definition_among(&QUANTITIES, category, field)
    }

    /// [`Basis::definition`], a category's quantities being those among `quantities`.
    #[must_use]
    pub fn definition_among(
        &self,
        quantities: &[Quantity],
        category: CategoryId,
        field: &Field,
    ) -> Option<Definition> {
        match field {
            Field::Name => builtin(NAME_ID, "Name"),
            Field::Type => builtin(TYPE_ID, "Type"),
            Field::Parameter(id) => self
                .params
                .parameters_of(self.categories, category)
                .into_iter()
                .find(|applicable| applicable.definition.id() == id)
                .map(|applicable| applicable.definition.clone()),
            Field::Quantity(name) => {
                let top = self.top(category)?;
                let found = quantities
                    .iter()
                    .find(|q| q.category == top && q.name == name)?;
                let id = Id::parse(found.id).ok()?;
                Definition::new(id, found.name, found.kind, Holder::Instance).ok()
            }
        }
    }

    /// The definition of a field the category must have.
    ///
    /// # Errors
    /// [`ScheduleError::NoSuchField`].
    pub fn require(
        &self,
        category: CategoryId,
        field: &Field,
    ) -> Result<Definition, ScheduleError> {
        self.require_among(&QUANTITIES, category, field)
    }

    /// [`Basis::require`], a category's quantities being those among `quantities`.
    ///
    /// # Errors
    /// [`ScheduleError::NoSuchField`].
    pub fn require_among(
        &self,
        quantities: &[Quantity],
        category: CategoryId,
        field: &Field,
    ) -> Result<Definition, ScheduleError> {
        self.definition_among(quantities, category, field)
            .ok_or_else(|| ScheduleError::NoSuchField {
                category: self.category_name(category),
                field: field.key(),
            })
    }

    /// Every field a category has, in a fixed order: name, type, its parameters by name then ID,
    /// its quantities in the table's order.
    #[must_use]
    pub fn fields_of(&self, category: CategoryId) -> Vec<(Field, Definition)> {
        self.fields_of_among(&QUANTITIES, category)
    }

    /// [`Basis::fields_of`], a category's quantities being those among `quantities`, in their
    /// order.
    #[must_use]
    pub fn fields_of_among(
        &self,
        quantities: &[Quantity],
        category: CategoryId,
    ) -> Vec<(Field, Definition)> {
        let mut fields: Vec<(Field, Definition)> = [Field::Name, Field::Type]
            .into_iter()
            .filter_map(|field| {
                self.definition_among(quantities, category, &field)
                    .map(|def| (field, def))
            })
            .collect();
        for applicable in self.params.parameters_of(self.categories, category) {
            fields.push((
                Field::Parameter(applicable.definition.id().clone()),
                applicable.definition.clone(),
            ));
        }
        if let Some(top) = self.top(category) {
            for q in quantities.iter().filter(|q| q.category == top) {
                let field = Field::Quantity(q.name.to_owned());
                if let Some(def) = self.definition_among(quantities, category, &field) {
                    fields.push((field, def));
                }
            }
        }
        fields
    }
}
