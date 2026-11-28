//! A field: one column a schedule can show, filter on or sort by, and what a category has of them.
//!
//! A field is one of five things, each with one stored spelling ([`Field::key`]):
//! - `name`: the element's own name, text;
//! - `type`: the name of the element's type, text, empty when it has none;
//! - `parameter:<id>`: a parameter by its definition's permanent ID, of any kind;
//! - `quantity:<name>`: a read-only quantity the product works out of an element and never
//!   stores, by the name it shows it under (`Area`, `Volume`...). [`QUANTITIES`] lists them;
//! - `calculated:<name>`: a formula over the schedule's other fields, kept in the schedule
//!   ([`crate::Calculated`]) and worked out for each row; only a schedule knows what it is, so
//!   [`Basis::definition`] has none for it and [`crate::Schedule::definition`] has.
//!
//! What a category has ([`Basis::definition`]) is decided by the document: a parameter is there
//! when its binding gives it to the category (construct-params' `parameters_of`), a quantity when
//! this crate's table gives it to the category's top-level category, and `name` and `type` are
//! there for every category.

use crate::error::ScheduleError;
use construct_categories::{
    CategoryId, CategoryKind, CategoryTable, FLOORS, ROOFS, ROOMS, SHEETS, VIEWS,
};
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
    /// A calculated field of the schedule, by its name.
    Calculated(String),
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
            Self::Calculated(name) => format!("calculated:{name}"),
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
                } else if let Some(name) = key.strip_prefix("calculated:") {
                    Self::Calculated(name.to_owned())
                } else {
                    return Err(bad());
                }
            }
        };
        if field.key() == key
            && !matches!(&field, Self::Quantity(name) | Self::Calculated(name) if name.is_empty())
        {
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

/// Every read-only quantity a schedule can list, by category. The names and kinds are those the
/// server's own read-only rows carry (construct-slabs, construct-roofs and construct-regen's
/// rooms), pinned by a test in construct-api that reads each element kind and compares.
///
/// Not here: a roof's pitch (its kind is a slope or an angle by how the roof is set, which a
/// field's kind cannot follow) and a floor's or roof's per-layer volumes and masses (their number
/// follows the type's layers).
///
/// The sheet list and the view list are schedules of the sheets and views categories: a sheet has
/// its number, paper and how many views and schedules stand on it, and a view its kind, its scale
/// as the `n` of 1:n, its detail level and the number of the sheet it stands on (empty when it is
/// on none). The sheet's and the view's own parameters are the category's parameters as for any
/// element. There are no revisions in the model yet, so there is no revision list: it waits for
/// the job that makes them.
pub const QUANTITIES: [Quantity; 23] = [
    quantity(FLOORS, "floor.area", "Area", Kind::Area),
    quantity(FLOORS, "floor.perimeter", "Perimeter", Kind::Length),
    quantity(FLOORS, "floor.thickness", "Thickness", Kind::Length),
    quantity(FLOORS, "floor.volume", "Volume", Kind::Volume),
    quantity(FLOORS, "floor.mass", "Mass (kg)", Kind::Number),
    quantity(ROOFS, "roof.area", "Area", Kind::Area),
    quantity(ROOFS, "roof.slope-area", "Slope area", Kind::Area),
    quantity(ROOFS, "roof.thickness", "Thickness", Kind::Length),
    quantity(ROOFS, "roof.volume", "Volume", Kind::Volume),
    quantity(ROOFS, "roof.mass", "Mass (kg)", Kind::Number),
    quantity(ROOFS, "roof.ridge-height", "Ridge height", Kind::Length),
    quantity(ROOMS, "room.state", "State", Kind::Text),
    quantity(ROOMS, "room.area", "Area", Kind::Area),
    quantity(ROOMS, "room.perimeter", "Perimeter", Kind::Length),
    quantity(ROOMS, "room.volume", "Volume", Kind::Volume),
    quantity(SHEETS, "sheet.number", "Number", Kind::Text),
    quantity(SHEETS, "sheet.paper", "Paper", Kind::Text),
    quantity(SHEETS, "sheet.views", "Views placed", Kind::Whole),
    quantity(SHEETS, "sheet.schedules", "Schedules placed", Kind::Whole),
    quantity(VIEWS, "view.kind", "Kind", Kind::Text),
    quantity(VIEWS, "view.scale", "Scale", Kind::Whole),
    quantity(VIEWS, "view.detail", "Detail level", Kind::Text),
    quantity(VIEWS, "view.sheet", "On sheet", Kind::Text),
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

    /// Whether a schedule may list this category: it exists and holds model elements or datums,
    /// or it is the sheets or the views (the sheet list and the view list).
    ///
    /// # Errors
    /// [`ScheduleError::UnknownCategory`] or [`ScheduleError::NotListable`].
    pub fn listable(&self, category: CategoryId) -> Result<(), ScheduleError> {
        let found = self
            .categories
            .get(category)
            .ok_or(ScheduleError::UnknownCategory(category))?;
        match found.kind {
            _ if found.id == SHEETS || found.id == VIEWS => Ok(()),
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
    /// when the category does not have it.
    #[must_use]
    pub fn definition(&self, category: CategoryId, field: &Field) -> Option<Definition> {
        match field {
            Field::Name => builtin(NAME_ID, "Name"),
            Field::Type => builtin(TYPE_ID, "Type"),
            Field::Parameter(id) => self
                .params
                .parameters_of(self.categories, category)
                .into_iter()
                .find(|applicable| applicable.definition.id() == id)
                .map(|applicable| applicable.definition.clone()),
            // A schedule's own calculated field: the schedule holds it, not the document.
            Field::Calculated(_) => None,
            Field::Quantity(name) => {
                let top = self.top(category)?;
                let found = QUANTITIES
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
        self.definition(category, field)
            .ok_or_else(|| ScheduleError::NoSuchField {
                category: self.category_name(category),
                field: field.key(),
            })
    }

    /// Every field a category has, in a fixed order: name, type, its parameters by name then ID,
    /// its quantities in the table's order.
    #[must_use]
    pub fn fields_of(&self, category: CategoryId) -> Vec<(Field, Definition)> {
        let mut fields: Vec<(Field, Definition)> = [Field::Name, Field::Type]
            .into_iter()
            .filter_map(|field| self.definition(category, &field).map(|def| (field, def)))
            .collect();
        for applicable in self.params.parameters_of(self.categories, category) {
            fields.push((
                Field::Parameter(applicable.definition.id().clone()),
                applicable.definition.clone(),
            ));
        }
        if let Some(top) = self.top(category) {
            for q in QUANTITIES.iter().filter(|q| q.category == top) {
                let field = Field::Quantity(q.name.to_owned());
                if let Some(def) = self.definition(category, &field) {
                    fields.push((field, def));
                }
            }
        }
        fields
    }
}
