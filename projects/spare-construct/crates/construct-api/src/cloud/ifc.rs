//! A model read out for the IFC writer (job 12.1, CON-A18-F1-P2): the levels, layered walls and
//! floors of a document, each as the builders read it, handed to construct-interop's
//! [`construct_interop::ifc_spatial::write`].
//!
//! What is read is what the regenerator builds from, not what it built: a wall's filled recipe
//! with its joints (`construct_regen::joins::recipe_at`), a floor's filled recipe, and where each
//! stands (`construct_regen::levels::placed`), so the lengths are the exact steps of the length
//! grid and no display rounding to whole millimetres is in them. Elements of other kinds are not
//! read here (the doors, windows, openings, roofs and rooms are job 12.2's).
//!
//! A wall or floor whose shape cannot be built is not left out quietly: the read refuses, naming
//! it and saying why, so a file never claims to hold a model it does not.
//!
//! Depends on construct-regen's readers, construct-levels, construct-interop and kit-store. Nothing
//! here is interim.

use construct_interop::ifc_spatial::{Export, Slab, Storey, Wall};
use construct_model::element::Element;
use construct_params::Length;
use construct_regen::floor_plan::{self, FloorPlan};
use construct_regen::joins;
use construct_regen::levels::{self, read_levels};
use construct_regen::parameters::{self as records, At};
use construct_regen::wall_plan::{self, Layered};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use std::collections::{BTreeMap, BTreeSet};

/// The model `document` of `project` as an [`Export`], its header stamped `stamp` (ISO 8601 text)
/// and its name `name`. The inner error is why the model cannot be written, in words.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn read_export<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    name: &str,
    stamp: &str,
) -> Result<Result<Export, String>, StoreError> {
    let (levels, _) = match read_levels(reader, project, document).await? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let storeys: Vec<Storey> = levels
        .ordered()
        .into_iter()
        .filter(|level| level.is_storey() && !levels.is_implied(level.id()))
        .map(|level| Storey {
            id: level.id().to_string(),
            name: level.name().to_owned(),
            elevation: level.elevation(),
        })
        .collect();
    let known: BTreeSet<String> = storeys.iter().map(|s| s.id.clone()).collect();
    let documents = records::read_document(reader, project, document).await?;
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    let mut type_names: BTreeMap<Id, String> = BTreeMap::new();
    let mut walls = Vec::new();
    let mut slabs = Vec::new();
    for (id, (_, record)) in documents {
        let is_wall = record.kind == "wall" && wall_plan::is_layered(&record.recipe);
        let is_floor = record.kind == "floor" && floor_plan::is_floor(&record.recipe);
        if !is_wall && !is_floor {
            continue;
        }
        let here = |why: String| format!("{} {id}: {why}", record.kind);
        let placed = match levels::placed(reader, project, document, &record, At::Now).await? {
            Ok(placed) => placed,
            Err(e) => return Ok(Err(here(e.message))),
        };
        let recipe = match joins::recipe_at(reader, project, document, &record, At::Now).await? {
            Ok(recipe) => recipe,
            Err(e) => return Ok(Err(here(e.message))),
        };
        let type_name = type_name(
            reader,
            project,
            document,
            &record,
            &context,
            &mut type_names,
        )
        .await?;
        let storey = placed
            .level
            .as_ref()
            .map(Id::to_string)
            .filter(|level| known.contains(level));
        let height = placed.elevation.unwrap_or(Length::from_mm(0));
        let (id, name) = (id.to_string(), record.name.clone());
        if is_wall {
            match Layered::of(&recipe) {
                Ok(shape) => walls.push(Wall {
                    id,
                    name,
                    type_name,
                    storey,
                    base: height,
                    shape,
                }),
                Err(e) => return Ok(Err(here(e.message))),
            }
        } else {
            match FloorPlan::of(&recipe) {
                Ok(shape) => slabs.push(Slab {
                    id,
                    name,
                    type_name,
                    storey,
                    top: height,
                    shape,
                }),
                Err(e) => return Ok(Err(here(e.message))),
            }
        }
    }
    Ok(Ok(Export {
        document: document.to_string(),
        name: name.to_owned(),
        stamp: stamp.to_owned(),
        storeys,
        walls,
        slabs,
    }))
}

/// The name of an element's type, or the type's ID when it cannot be read, or nothing when the
/// element names none; each read once.
async fn type_name<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
    context: &records::Context,
    cache: &mut BTreeMap<Id, String>,
) -> Result<String, StoreError> {
    let Some(ty) = records::type_of(record) else {
        return Ok(String::new());
    };
    if let Some(name) = cache.get(&ty) {
        return Ok(name.clone());
    }
    let name = match records::read_type(reader, project, document, &ty, context).await? {
        Ok(found) => found.name().to_owned(),
        Err(_) => ty.to_string(),
    };
    cache.insert(ty, name.clone());
    Ok(name)
}
