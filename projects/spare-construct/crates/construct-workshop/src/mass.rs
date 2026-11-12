//! Materials and a part's mass.
//!
//! The materials are data: one file, `demo/workshop/materials.json`, which the Workshop page reads
//! too, so both give a part the same density. Each material keeps the value its source prints (a
//! density in lb/in³ from MIL-HDBK-5J for metals, or a specific gravity at 12% moisture from the
//! USDA Wood Handbook, FPL-GTR-282, for timbers), the table and page it is on, and the density in
//! kg/m³ worked out from it; the tests hold every density to its printed value. Both sources are
//! works of the United States government, in the public domain. Plastics are not listed yet: no
//! public-domain source for their densities has been found that says it may be shared.
//!
//! A part's mass is its volume times its material's density. The volume of a flat-faced body is the
//! kernel's `query.volume`, exact; a cylinder's is worked out from the recipe, as the kernel's
//! certified mass properties wait for G247, and the answer says how much of the volume is which.
//! The centre of mass is worked out from the recipe too (each body's centroid, through its map,
//! weighted by its volume): the kernel's `query.volume` gives no first moments yet (G247).
//!
//! The answer's `mass`: `{ material, name, density, source, grams, exact, from_recipe, centre,
//! message }`, with volumes in mm³, the density in kg/m³ and the centre in millimetres; `name`,
//! `density`, `source` and `grams` are `None` for a material that isn't in the list.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::build::{BodyOut, fmt_mm};

/// The materials, as the page reads them.
const DATA: &str = include_str!("../../../demo/workshop/materials.json");

/// The materials file.
#[derive(Debug, Deserialize)]
pub struct Materials {
    /// Each source, by its key.
    pub sources: BTreeMap<String, Source>,
    /// Each material, in the order the page lists them.
    pub materials: Vec<Material>,
}

/// A source the densities come from.
#[derive(Debug, Deserialize)]
pub struct Source {
    pub title: String,
    pub by: String,
    pub date: String,
    /// What it says of its sharing.
    pub status: String,
    pub url: String,
}

/// A material.
#[derive(Debug, Deserialize)]
pub struct Material {
    /// What a part names it by.
    pub id: String,
    pub name: String,
    /// `metal` or `timber`.
    pub group: String,
    /// Kilograms per cubic metre, worked out from `printed`.
    pub density: f64,
    /// The value as its source prints it, in `unit`.
    pub printed: f64,
    /// `lb/in3`, or `sg12` for a specific gravity at 12% moisture.
    pub unit: String,
    /// Its source's key.
    pub source: String,
    /// The table and page it is on.
    #[serde(rename = "where")]
    pub at: String,
}

/// Kilograms per cubic metre in a pound per cubic inch: 0.45359237 kg over 0.0254³ m³.
pub const LB_PER_IN3: f64 = 27_679.904_7;

impl Material {
    /// The density its printed value gives, in kg/m³: by the pound and inch's exact definitions, or
    /// by the Wood Handbook's equation 4-14 at 12% moisture.
    #[must_use]
    pub fn worked_out(&self) -> Option<f64> {
        match self.unit.as_str() {
            "lb/in3" => Some(self.printed * LB_PER_IN3),
            "sg12" => Some(1000.0 * self.printed * 1.12),
            _ => None,
        }
    }
}

/// Every material, read once.
///
/// # Panics
///
/// If the file built in is not a materials file, which its tests rule out.
#[must_use]
pub fn materials() -> &'static Materials {
    static READ: OnceLock<Materials> = OnceLock::new();
    READ.get_or_init(|| serde_json::from_str(DATA).expect("materials.json is a materials file"))
}

/// A part's mass, as the answer gives it.
#[derive(Debug, Serialize)]
pub struct MassOut {
    /// The material the part names.
    pub material: String,
    /// Its name, if it is in the list.
    pub name: Option<String>,
    /// Its density, in kg/m³.
    pub density: Option<f64>,
    /// Where the density comes from: the source, its table and page.
    pub source: Option<String>,
    /// The part's mass, in grams.
    pub grams: Option<f64>,
    /// How much of the volume is the kernel's, exact, in mm³.
    pub exact: f64,
    /// How much is worked out from the recipe (cylinders, until G247), in mm³.
    pub from_recipe: f64,
    /// The centre of mass, in millimetres, worked out from the recipe; `None` with no bodies.
    pub centre: Option<[f64; 3]>,
    /// What it is and where it comes from, in words.
    pub message: String,
}

/// A body's centroid in its own place: a prism's outline's centroid, or a cylinder's centre, half way
/// up.
fn centroid(b: &BodyOut) -> [f64; 3] {
    let z = b.z + b.height / 2.0;
    if let Some(pts) = &b.outline {
        let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
        for (i, p) in pts.iter().enumerate() {
            let q = pts[(i + 1) % pts.len()];
            let cross = p[0] * q[1] - q[0] * p[1];
            a += cross;
            cx += (p[0] + q[0]) * cross;
            cy += (p[1] + q[1]) * cross;
        }
        return [cx / (3.0 * a), cy / (3.0 * a), z];
    }
    let c = b.center.unwrap_or([0.0, 0.0]);
    [c[0], c[1], z]
}

fn apply(m: &[f64; 12], p: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i * 4] * p[0] + m[i * 4 + 1] * p[1] + m[i * 4 + 2] * p[2] + m[i * 4 + 3])
}

/// The mass of `bodies` in the material `id`.
#[must_use]
pub fn mass_of(id: &str, bodies: &[BodyOut]) -> MassOut {
    let all = materials();
    let found = all.materials.iter().find(|m| m.id == id);
    let exact: f64 = bodies
        .iter()
        .filter(|b| b.volume_from == "kernel")
        .map(|b| b.volume)
        .sum();
    let from_recipe: f64 = bodies
        .iter()
        .filter(|b| b.volume_from != "kernel")
        .map(|b| b.volume)
        .sum();
    let volume = exact + from_recipe;
    let centre = (volume > 0.0).then(|| {
        let mut s = [0.0; 3];
        for b in bodies {
            let c = apply(&b.m, centroid(b));
            for (sk, ck) in s.iter_mut().zip(c) {
                *sk += ck * b.volume;
            }
        }
        s.map(|v| v / volume)
    });
    let Some(m) = found else {
        return MassOut {
            material: id.to_owned(),
            name: None,
            density: None,
            source: None,
            grams: None,
            exact,
            from_recipe,
            centre,
            message: format!(
                "There is no material called {id} with a sourced density, so the part has no mass. \
                 Pick one from the list."
            ),
        };
    };
    let source = all
        .sources
        .get(&m.source)
        .map_or_else(|| m.source.clone(), |s| s.title.clone());
    let grams = volume * m.density / 1e6;
    let volumes = if from_recipe > 0.0 {
        format!(
            "{} mm³ of the volume is the kernel's, exact, and {} mm³ is worked out from the recipe \
             for its cylinders, whose certified volume waits for the kernel's mass properties (G247)",
            fmt_mm(exact),
            fmt_mm(from_recipe)
        )
    } else {
        "the volume is the kernel's, exact".to_owned()
    };
    MassOut {
        material: id.to_owned(),
        name: Some(m.name.clone()),
        density: Some(m.density),
        source: Some(format!("{source}, {}", m.at)),
        grams: Some(grams),
        exact,
        from_recipe,
        centre,
        message: format!(
            "{} g of {}: {volumes}; the density, {} kg/m³, is from {source}, {}. The centre of mass \
             is worked out from the recipe; the kernel's certified mass properties wait for G247.",
            fmt_mm(grams),
            m.name,
            fmt_mm(m.density),
            m.at
        ),
    }
}
