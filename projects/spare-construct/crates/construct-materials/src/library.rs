//! A project's material library: the materials and the sources their values cite, the pure
//! changes a project makes, and the stored form.
//!
//! **Changes** are pure functions that take `&self` and return the new library, leaving the old
//! one as it was; a refused change is an error value and changes nothing. Putting them through
//! the edit log is a later job.
//!
//! - Add and duplicate take the new material's ID from the caller (this crate has no clock and
//!   no random numbers); an ID already in the library, or one of the built-in set's, is refused.
//! - A built-in material can be renamed and edited in the project; its ID never changes, and the
//!   functions here have no way to change an ID.
//! - A built-in cannot be deleted. A project's own material is deleted with [`Materials::without`],
//!   which is told what uses it (this crate keeps no index of users) and refuses if that list is
//!   not empty.
//! - Names are unique, compared without regard to case ([`Materials::by_name`]).
//!
//! **Stored form.** One JSON document, no spare space:
//! `{"format":1,"sources":[...],"materials":[...]}`, sources in ascending key, materials in
//! ascending ID, fields in the order the types declare them. The same library is always the same
//! bytes; a reader refuses any other spelling of the same data.

use crate::default::{default_materials, default_sources};
use crate::error::MaterialError;
use crate::mass::mass_grams;
use crate::material::{
    Class, Drawing, Material, MaterialId, NewMaterial, Physical, RefName, clean_name, name_key,
    text_ok,
};
use construct_params::Volume;
use serde::{Deserialize, Serialize};

/// The version of the stored form this crate writes. Only version 1 exists so far; a reader never
/// stops reading an old one.
pub const FORMAT_VERSION: u32 = 1;

/// A published source that physical values cite: a standard, a handbook or a report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The key a [`SourceRef`] names it by: 1 to 40 of a to z, 0 to 9 and `-`.
    pub key: String,
    /// The title.
    pub title: String,
    /// Who published it.
    pub by: String,
    /// When, as the source dates itself.
    pub date: String,
    /// What the source says of its own sharing.
    pub status: String,
    /// Where to read it.
    pub url: String,
}

impl Source {
    fn check(&self) -> Result<(), MaterialError> {
        let key_ok = !self.key.is_empty()
            && self.key.chars().count() <= 40
            && self
                .key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let fields_ok = [&self.title, &self.by, &self.date, &self.status, &self.url]
            .iter()
            .all(|field| !field.trim().is_empty() && field.chars().count() <= 400);
        if key_ok && fields_ok {
            Ok(())
        } else {
            Err(MaterialError::BadSource {
                key: self.key.clone(),
            })
        }
    }
}

/// A project's materials and the sources their values cite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Materials {
    materials: Vec<Material>,
    sources: Vec<Source>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    sources: Vec<Source>,
    materials: Vec<Material>,
}

impl Materials {
    /// The default metric library: see [`crate::default`].
    #[must_use]
    pub fn default_metric() -> Self {
        let mut library = Self {
            materials: default_materials(),
            sources: default_sources(),
        };
        library.materials.sort_by(|a, b| a.id.cmp(&b.id));
        library.sources.sort_by(|a, b| a.key.cmp(&b.key));
        library
    }

    /// Every material, in ascending ID.
    #[must_use]
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }

    /// Every source, in ascending key.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// The material with this ID.
    #[must_use]
    pub fn get(&self, id: &MaterialId) -> Option<&Material> {
        self.materials.iter().find(|material| &material.id == id)
    }

    /// The material with this name, compared without regard to case and ignoring spaces at
    /// either end.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&Material> {
        let key = name_key(name.trim());
        self.materials
            .iter()
            .find(|material| name_key(&material.name) == key)
    }

    /// The source with this key.
    #[must_use]
    pub fn source(&self, key: &str) -> Option<&Source> {
        self.sources.iter().find(|source| source.key == key)
    }

    /// The mass in whole grams of `volume` of the material `id`, by the rule in
    /// [`crate::mass`].
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`], or what [`mass_grams`] refuses.
    pub fn mass_grams(&self, id: &MaterialId, volume: Volume) -> Result<u64, MaterialError> {
        let material = self
            .get(id)
            .ok_or_else(|| MaterialError::NoSuchMaterial { id: id.clone() })?;
        mass_grams(volume, &material.physical)
    }

    /// A library with one more source.
    ///
    /// # Errors
    /// [`MaterialError::BadSource`] or [`MaterialError::DuplicateSource`].
    pub fn with_source(&self, source: Source) -> Result<Self, MaterialError> {
        source.check()?;
        if self.source(&source.key).is_some() {
            return Err(MaterialError::DuplicateSource { key: source.key });
        }
        let mut next = self.clone();
        next.sources.push(source);
        next.sources.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(next)
    }

    /// A library with one more material of the project's own, with the ID the caller gives it.
    ///
    /// # Errors
    /// [`MaterialError::DuplicateId`], [`MaterialError::DuplicateName`], a bad name, a value out
    /// of range, or a source reference to a source the library does not hold.
    pub fn with_material(
        &self,
        id: MaterialId,
        new: NewMaterial,
    ) -> Result<(Self, MaterialId), MaterialError> {
        self.check_new_id(&id)?;
        let name = clean_name(&new.name)?;
        self.check_name_free(&name, None)?;
        new.physical.check()?;
        self.check_sources(&new.physical)?;
        let material = Material {
            id: id.clone(),
            name,
            class: new.class,
            built_in: false,
            drawing: new.drawing,
            physical: new.physical,
            look: new.look,
        };
        let mut next = self.clone();
        next.materials.push(material);
        next.materials.sort_by(|a, b| a.id.cmp(&b.id));
        Ok((next, id))
    }

    /// A library with a copy of material `from` under a new ID and name. The copy is the
    /// project's own, even if `from` is a built-in.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`], and what [`Materials::with_material`] refuses.
    pub fn duplicated(
        &self,
        from: &MaterialId,
        id: MaterialId,
        name: &str,
    ) -> Result<(Self, MaterialId), MaterialError> {
        let source = self
            .get(from)
            .ok_or_else(|| MaterialError::NoSuchMaterial { id: from.clone() })?;
        self.with_material(
            id,
            NewMaterial {
                name: name.to_owned(),
                class: source.class,
                drawing: source.drawing.clone(),
                physical: source.physical.clone(),
                look: source.look.clone(),
            },
        )
    }

    /// A library with material `id` renamed. Its ID is the same.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`], a bad name, or [`MaterialError::DuplicateName`].
    pub fn renamed(&self, id: &MaterialId, name: &str) -> Result<Self, MaterialError> {
        let name = clean_name(name)?;
        self.check_name_free(&name, Some(id))?;
        self.changed(id, |material| material.name = name)
    }

    /// A library with material `id` given a new class.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`].
    pub fn with_class(&self, id: &MaterialId, class: Class) -> Result<Self, MaterialError> {
        self.changed(id, |material| material.class = class)
    }

    /// A library with material `id` given new drawing graphics (group 1). The physical data and
    /// the look are untouched.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`].
    pub fn with_drawing(&self, id: &MaterialId, drawing: Drawing) -> Result<Self, MaterialError> {
        self.changed(id, |material| material.drawing = drawing)
    }

    /// A library with material `id` given new physical data (group 2). The drawing graphics and
    /// the look are untouched.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`], a value out of range, or a source reference to a source
    /// the library does not hold.
    pub fn with_physical(
        &self,
        id: &MaterialId,
        physical: Physical,
    ) -> Result<Self, MaterialError> {
        physical.check()?;
        self.check_sources(&physical)?;
        self.changed(id, |material| material.physical = physical)
    }

    /// A library with material `id` pointing at a render look by name (group 3), or at none. The
    /// drawing graphics and the physical data are untouched.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`].
    pub fn with_look(&self, id: &MaterialId, look: Option<RefName>) -> Result<Self, MaterialError> {
        self.changed(id, |material| material.look = look)
    }

    /// A library without material `id`. `users` is what uses it, as the caller names them (this
    /// crate keeps no index of users); a list that is not empty refuses the deletion.
    ///
    /// # Errors
    /// [`MaterialError::NoSuchMaterial`], [`MaterialError::BuiltInNotDeletable`] or
    /// [`MaterialError::InUse`], checked in that order.
    pub fn without(&self, id: &MaterialId, users: &[String]) -> Result<Self, MaterialError> {
        let material = self
            .get(id)
            .ok_or_else(|| MaterialError::NoSuchMaterial { id: id.clone() })?;
        if material.built_in {
            return Err(MaterialError::BuiltInNotDeletable { id: id.clone() });
        }
        if !users.is_empty() {
            return Err(MaterialError::InUse {
                id: id.clone(),
                users: users.to_vec(),
            });
        }
        let mut next = self.clone();
        next.materials.retain(|kept| &kept.id != id);
        Ok(next)
    }

    fn changed(
        &self,
        id: &MaterialId,
        change: impl FnOnce(&mut Material),
    ) -> Result<Self, MaterialError> {
        let mut next = self.clone();
        let material = next
            .materials
            .iter_mut()
            .find(|material| &material.id == id)
            .ok_or_else(|| MaterialError::NoSuchMaterial { id: id.clone() })?;
        change(material);
        Ok(next)
    }

    fn check_new_id(&self, id: &MaterialId) -> Result<(), MaterialError> {
        let taken = self.get(id).is_some()
            || default_materials()
                .iter()
                .any(|built_in| &built_in.id == id);
        if taken {
            return Err(MaterialError::DuplicateId { id: id.clone() });
        }
        Ok(())
    }

    fn check_name_free(&self, name: &str, own: Option<&MaterialId>) -> Result<(), MaterialError> {
        let key = name_key(name);
        match self
            .materials
            .iter()
            .find(|material| Some(&material.id) != own && name_key(&material.name) == key)
        {
            Some(other) => Err(MaterialError::DuplicateName {
                existing: other.id.clone(),
            }),
            None => Ok(()),
        }
    }

    fn check_sources(&self, physical: &Physical) -> Result<(), MaterialError> {
        for reference in physical.source_refs() {
            if self.source(&reference.source).is_none() {
                return Err(MaterialError::UnknownSource {
                    key: reference.source.clone(),
                });
            }
            if reference.at.trim().is_empty() || !text_ok(&reference.at) {
                return Err(MaterialError::BadText {
                    field: "source place",
                });
            }
        }
        Ok(())
    }

    /// Checks every rule over the whole library: each source valid and unique, each material's
    /// name valid and unique, each ID unique, each value in range and each source reference
    /// naming a source held.
    ///
    /// # Errors
    /// The first rule broken.
    pub fn check(&self) -> Result<(), MaterialError> {
        for (at, source) in self.sources.iter().enumerate() {
            source.check()?;
            if self
                .sources
                .iter()
                .take(at)
                .any(|earlier| earlier.key == source.key)
            {
                return Err(MaterialError::DuplicateSource {
                    key: source.key.clone(),
                });
            }
        }
        for (at, material) in self.materials.iter().enumerate() {
            if clean_name(&material.name)? != material.name {
                return Err(MaterialError::BadText { field: "name" });
            }
            for earlier in self.materials.iter().take(at) {
                if earlier.id == material.id {
                    return Err(MaterialError::DuplicateId {
                        id: material.id.clone(),
                    });
                }
                if name_key(&earlier.name) == name_key(&material.name) {
                    return Err(MaterialError::DuplicateName {
                        existing: earlier.id.clone(),
                    });
                }
            }
            material.physical.check()?;
            self.check_sources(&material.physical)?;
        }
        Ok(())
    }

    /// The stored bytes: the same library always gives the same bytes.
    ///
    /// # Errors
    /// [`MaterialError::Format`] if the writer fails, which it does not for these types.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MaterialError> {
        let mut materials = self.materials.clone();
        materials.sort_by(|a, b| a.id.cmp(&b.id));
        let mut sources = self.sources.clone();
        sources.sort_by(|a, b| a.key.cmp(&b.key));
        serde_json::to_vec(&Stored {
            format: FORMAT_VERSION,
            sources,
            materials,
        })
        .map_err(|why| MaterialError::Format(why.to_string()))
    }

    /// Reads stored bytes, checking every rule on the way in.
    ///
    /// # Errors
    /// [`MaterialError::Format`] for anything that is not valid JSON of this shape,
    /// [`MaterialError::Version`] for a version other than 1, the refusal of any rule the data
    /// breaks, and [`MaterialError::NotCanonical`] for valid data spelt another way.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MaterialError> {
        let stored: Stored =
            serde_json::from_slice(bytes).map_err(|why| MaterialError::Format(why.to_string()))?;
        if stored.format != FORMAT_VERSION {
            return Err(MaterialError::Version {
                found: stored.format,
            });
        }
        let library = Self {
            materials: stored.materials,
            sources: stored.sources,
        };
        library.check()?;
        if library.to_bytes()?.as_slice() != bytes {
            return Err(MaterialError::NotCanonical);
        }
        Ok(library)
    }
}
