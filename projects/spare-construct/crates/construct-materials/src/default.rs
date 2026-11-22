//! Our default metric library of common building materials.
//!
//! Each material's drawing graphics are our own choice: a mid colour for the surface seen in
//! projection, a darker colour for where a view cuts it, a solid line pattern, and a named area
//! fill for the cut (the fill patterns themselves are a later job, see [`crate::material`]).
//! Each material's density is a published value, kept with the source and the place in it, and a
//! test fails if any default value has no source. The list is ours; no other product's material
//! library was used. The steel, aluminium and timber densities, and the first two sources below,
//! come from the earlier Workshop materials list (piece A27, which holds a copy of its own that
//! should later read from this crate); the places are kept as that list records them. Timber
//! densities are at 12 percent moisture. Densities are in whole grams per cubic metre.
//!
//! No masonry, finish or plasterboard is in the default set yet: no public source with a value
//! for a building brick, block or board was found. A project adds its own.
//!
//! Thermal conductivity and specific heat are left empty in the defaults for the same reason:
//! a value is only stored with a public source for it.
//!
//! The IDs are permanent and pinned by a test.

use crate::library::Source;
use crate::material::{
    Class, Drawing, Graphics, Material, MaterialId, Measure, Physical, RefName, SourceRef,
};
use construct_styles::{Colour, patterns::SOLID};

struct Row {
    id: &'static str,
    name: &'static str,
    class: Class,
    surface: (u8, u8, u8),
    cut: (u8, u8, u8),
    cut_fill: Option<&'static str>,
    density_g_per_m3: u64,
    source: &'static str,
    at: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        id: "concrete-regular",
        name: "Concrete, regular",
        class: Class::Concrete,
        surface: (150, 150, 150),
        cut: (90, 90, 90),
        cut_fill: Some("concrete"),
        density_g_per_m3: 2_300_000,
        source: "pnnl-15870-rev1",
        at: "Material 99, page 112 of 357",
    },
    Row {
        id: "steel-aisi-1025",
        name: "Carbon steel, AISI 1025",
        class: Class::Steel,
        surface: (110, 120, 130),
        cut: (40, 50, 60),
        cut_fill: Some("steel"),
        density_g_per_m3: 7_861_000,
        source: "mil-hdbk-5j",
        at: "Table 2.2.1.0(b), page 2-8",
    },
    Row {
        id: "steel-stainless-aisi-301",
        name: "Stainless steel, AISI 301 (and 302, 304, 316)",
        class: Class::Steel,
        surface: (170, 175, 180),
        cut: (60, 65, 70),
        cut_fill: Some("steel"),
        density_g_per_m3: 7_916_000,
        source: "mil-hdbk-5j",
        at: "Table 2.7.1.0(b), page 2-223",
    },
    Row {
        id: "aluminium-6061",
        name: "Aluminium 6061",
        class: Class::Metal,
        surface: (190, 195, 200),
        cut: (80, 85, 90),
        cut_fill: Some("metal"),
        density_g_per_m3: 2_713_000,
        source: "mil-hdbk-5j",
        at: "Table 3.6.2.0(b1), page 3-264",
    },
    Row {
        id: "timber-white-oak",
        name: "White oak",
        class: Class::Timber,
        surface: (190, 150, 100),
        cut: (120, 80, 40),
        cut_fill: Some("timber"),
        density_g_per_m3: 762_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-6",
    },
    Row {
        id: "timber-douglas-fir",
        name: "Douglas-fir, coast",
        class: Class::Timber,
        surface: (210, 170, 120),
        cut: (140, 100, 50),
        cut_fill: Some("timber"),
        density_g_per_m3: 538_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-7",
    },
    Row {
        id: "timber-loblolly-pine",
        name: "Loblolly pine",
        class: Class::Timber,
        surface: (215, 180, 125),
        cut: (145, 105, 55),
        cut_fill: Some("timber"),
        density_g_per_m3: 571_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-7",
    },
    Row {
        id: "timber-eastern-white-pine",
        name: "Eastern white pine",
        class: Class::Timber,
        surface: (225, 195, 145),
        cut: (155, 120, 70),
        cut_fill: Some("timber"),
        density_g_per_m3: 392_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-7",
    },
    Row {
        id: "timber-sitka-spruce",
        name: "Sitka spruce",
        class: Class::Timber,
        surface: (225, 200, 150),
        cut: (160, 125, 75),
        cut_fill: Some("timber"),
        density_g_per_m3: 448_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-8",
    },
    Row {
        id: "timber-western-redcedar",
        name: "Western redcedar",
        class: Class::Timber,
        surface: (190, 120, 90),
        cut: (120, 65, 40),
        cut_fill: Some("timber"),
        density_g_per_m3: 358_000,
        source: "fpl-gtr-282",
        at: "Table 5-3a, page 5-7",
    },
    Row {
        id: "glass-plate",
        name: "Glass, plate",
        class: Class::Glass,
        surface: (150, 200, 220),
        cut: (60, 120, 150),
        cut_fill: Some("glass"),
        density_g_per_m3: 2_400_000,
        source: "pnnl-15870-rev1",
        at: "Material 146, page 154 of 357",
    },
    Row {
        id: "insulation-foam-glass",
        name: "Foam glass insulation",
        class: Class::Insulation,
        surface: (240, 220, 120),
        cut: (200, 170, 40),
        cut_fill: Some("insulation"),
        density_g_per_m3: 128_000,
        source: "pnnl-15870-rev1",
        at: "Material 144, page 152 of 357",
    },
    Row {
        id: "plaster-gypsum",
        name: "Gypsum plaster",
        class: Class::Plaster,
        surface: (235, 230, 220),
        cut: (150, 145, 135),
        cut_fill: Some("plaster"),
        density_g_per_m3: 2_320_000,
        source: "pnnl-15870-rev1",
        at: "Material 149, page 156 of 357",
    },
    Row {
        id: "membrane-polyethylene",
        name: "Polyethylene sheet",
        class: Class::Membrane,
        surface: (60, 60, 60),
        cut: (20, 20, 20),
        cut_fill: Some("membrane"),
        density_g_per_m3: 930_000,
        source: "pnnl-15870-rev1",
        at: "Material 248, page 239 of 357",
    },
    Row {
        id: "earth-average",
        name: "Earth, average",
        class: Class::Earth,
        surface: (140, 110, 80),
        cut: (90, 65, 40),
        cut_fill: Some("earth"),
        density_g_per_m3: 1_520_000,
        source: "pnnl-15870-rev1",
        at: "Material 105, page 119 of 357",
    },
    Row {
        id: "air-dry",
        name: "Air, dry",
        class: Class::AirGap,
        surface: (220, 235, 245),
        cut: (120, 150, 170),
        cut_fill: None,
        density_g_per_m3: 1_205,
        source: "pnnl-15870-rev1",
        at: "Material 4, page 18 of 357",
    },
];

fn colour((red, green, blue): (u8, u8, u8)) -> Colour {
    Colour(red, green, blue)
}

/// The built-in materials, unsorted.
pub(crate) fn default_materials() -> Vec<Material> {
    ROWS.iter()
        .filter_map(|row| {
            Some(Material {
                id: MaterialId::parse(row.id).ok()?,
                name: row.name.to_owned(),
                class: row.class,
                built_in: true,
                drawing: Drawing {
                    surface: Graphics {
                        colour: colour(row.surface),
                        pattern: SOLID,
                        fill: None,
                    },
                    cut: Graphics {
                        colour: colour(row.cut),
                        pattern: SOLID,
                        fill: row.cut_fill.and_then(|name| RefName::new(name).ok()),
                    },
                },
                physical: Physical {
                    density_g_per_m3: Measure {
                        value: row.density_g_per_m3,
                        source: Some(SourceRef {
                            source: row.source.to_owned(),
                            at: row.at.to_owned(),
                        }),
                    },
                    conductivity_mw_per_m_k: None,
                    specific_heat_j_per_kg_k: None,
                },
                look: None,
            })
        })
        .collect()
}

fn source(key: &str, title: &str, by: &str, date: &str, status: &str, url: &str) -> Source {
    Source {
        key: key.to_owned(),
        title: title.to_owned(),
        by: by.to_owned(),
        date: date.to_owned(),
        status: status.to_owned(),
        url: url.to_owned(),
    }
}

/// The sources the default values cite, unsorted.
pub(crate) fn default_sources() -> Vec<Source> {
    vec![
        source(
            "mil-hdbk-5j",
            "MIL-HDBK-5J, Metallic Materials and Elements for Aerospace Vehicle Structures",
            "United States Department of Defense",
            "2003-01-31",
            "Distribution Statement A: approved for public release; distribution is unlimited",
            "https://everyspec.com/MIL-HDBK/MIL-HDBK-0001-0099/download.php?spec=MIL_HDBK_5J.139.pdf",
        ),
        source(
            "fpl-gtr-282",
            "Wood Handbook: Wood as an Engineering Material, General Technical Report FPL-GTR-282, chapters 4 and 5",
            "United States Department of Agriculture, Forest Service, Forest Products Laboratory",
            "2021",
            "A work of the United States government, in the public domain",
            "https://research.fs.usda.gov/download/treesearch/62244.pdf",
        ),
        source(
            "pnnl-15870-rev1",
            "Compendium of Material Composition Data for Radiation Transport Modeling, PNNL-15870 Revision 1",
            "Pacific Northwest National Laboratory, for the United States Department of Homeland Security under a Department of Energy contract",
            "2011-03-04",
            "A public technical report; only its printed densities, which are facts, are used",
            "https://www.pnnl.gov/main/publications/external/technical_reports/PNNL-15870Rev1.pdf",
        ),
    ]
}
