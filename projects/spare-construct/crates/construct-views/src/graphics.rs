//! A view's graphics rules: how a category or a subcategory looks in this one view (CON-A8-F3-P2).
//!
//! A rule is for one category or subcategory (by [`CategoryId`]) and one [`Edge`]: what a view cuts
//! through ([`Edge::Cut`]) or what it sees beyond the cut plane ([`Edge::Projected`], "projection").
//! Each rule is a whole [`RuleStyle`]: the pen (line weight, by number, as the view's scale prints
//! it), the line pattern, the line colour, the fill where a cut has one, halftone, transparency and
//! shown or hidden. A rule that names only some of these is made by the command from what the
//! element would look like without it, so the stored rule is always complete and a reader never
//! has to merge two rules.
//!
//! **Which views hold rules.** Plans, sections and elevations hold both edges. A three-dimensional
//! view cuts nothing, so it holds projection rules only. A drafting view and a legend show no
//! model, so they hold none ([`ViewError::NoGraphicsOnKind`]). A view with no rules looks as the
//! project's object styles say: that is every view stored before this part existed.
//!
//! **Which rules are allowed** ([`Graphics::check`]): the category exists and is not the views
//! category; a cut rule only on a category that can be cut; a fill only on a cut rule and only one
//! of [`FILL_PATTERNS`]; the pen exists in the pen table the category's kind uses (annotation and
//! view categories use the annotation pens, every other the model pens, as construct-styles'
//! object styles do); the line pattern exists; transparency is a whole percentage, 0 to 100.
//!
//! **Stored form** ([`GRAPHICS_KEY`]): one JSON text on the view record, written only when the view
//! has a rule: `{"format":1,"rules":[{"category":n,"cut":style or null,"projection":style or
//! null}]}`, rules in ascending category, never one with neither side, no spare space. The same
//! rules are always the same bytes, and a reader refuses any other spelling.

use crate::error::ViewError;
use crate::kind::ViewKind;
use construct_categories::{CategoryId, CategoryKind, CategoryTable};
use construct_styles::{Colour, Edge, LineStyles, PatternId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The record key a view's graphics rules are stored under.
pub const GRAPHICS_KEY: &str = "@view.graphics";
/// The version of the stored form this crate writes. A reader never stops reading an old one.
pub const GRAPHICS_FORMAT_VERSION: u32 = 1;
/// The most transparency there is, as a percentage: nothing of the element shows.
pub const MOST_TRANSPARENCY: u8 = 100;
/// The fill patterns a rule may name: construct-drawing's default set, by name. Interim: a project
/// stores no fill patterns of its own yet; the server's tests pin this list to the drawing's own,
/// and the job that stores a project's patterns replaces this list with the project's.
pub const FILL_PATTERNS: [&str; 9] = [
    "concrete",
    "earth",
    "glass",
    "insulation",
    "membrane",
    "metal",
    "plaster",
    "steel",
    "timber",
];

/// The key an edge is spelt with, in commands and in answers.
#[must_use]
pub const fn edge_key(edge: Edge) -> &'static str {
    match edge {
        Edge::Cut => "cut",
        Edge::Projected => "projection",
    }
}

/// The edge a key names: `cut` or `projection`.
#[must_use]
pub fn edge_from_key(key: &str) -> Option<Edge> {
    match key {
        "cut" => Some(Edge::Cut),
        "projection" => Some(Edge::Projected),
        _ => None,
    }
}

/// A fill: a pattern by name and the colour its lines print in.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fill {
    /// One of [`FILL_PATTERNS`].
    pub pattern: String,
    /// The colour of the pattern's lines.
    pub colour: Colour,
}

/// How one edge of a category looks in one view.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleStyle {
    /// The line weight: a pen by number, printed as thin or thick as the view's scale says.
    pub pen: u16,
    /// The line pattern.
    pub pattern: PatternId,
    /// The line colour.
    pub colour: Colour,
    /// The fill of a cut. `None` is no rule about it: the element's own material fills it. Always
    /// `None` on a projection rule.
    pub fill: Option<Fill>,
    /// Drawn in a lighter tone, as what is not the subject of the view.
    pub halftone: bool,
    /// How much of what lies behind shows through, as a whole percentage from 0 (solid) to 100.
    pub transparency: u8,
    /// Whether it is drawn at all.
    pub shown: bool,
}

/// What a view says about one category or subcategory: a style for each edge, either of which may
/// be left to the level below it.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// How it looks where the view cuts it.
    pub cut: Option<RuleStyle>,
    /// How it looks where the view sees it beyond the cut plane.
    pub projection: Option<RuleStyle>,
}

impl Rule {
    /// The style for one edge, if the rule has one.
    #[must_use]
    pub fn get(&self, edge: Edge) -> Option<&RuleStyle> {
        match edge {
            Edge::Cut => self.cut.as_ref(),
            Edge::Projected => self.projection.as_ref(),
        }
    }

    fn slot(&mut self, edge: Edge) -> &mut Option<RuleStyle> {
        match edge {
            Edge::Cut => &mut self.cut,
            Edge::Projected => &mut self.projection,
        }
    }

    fn is_empty(&self) -> bool {
        self.cut.is_none() && self.projection.is_none()
    }
}

/// A view's graphics rules: the rule of each category and subcategory that has one. Changes are
/// pure: they return new rules and leave the old ones as they were.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graphics {
    rules: BTreeMap<CategoryId, Rule>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRule {
    category: CategoryId,
    cut: Option<RuleStyle>,
    projection: Option<RuleStyle>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    rules: Vec<StoredRule>,
}

/// Whether a category's pen numbers name the annotation pens (true) or the model pens (false): the
/// rule construct-styles' object styles use.
fn uses_annotation_pens(kind: CategoryKind) -> bool {
    matches!(kind, CategoryKind::Annotation | CategoryKind::View)
}

/// The words for a kind of view in a refusal: `three d`.
fn words(kind: &ViewKind) -> String {
    kind.key().replace('_', " ")
}

impl Graphics {
    /// No rules: the view looks as the project's object styles say.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is no rule.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Every rule, in ascending category.
    pub fn rules(&self) -> impl Iterator<Item = (CategoryId, &Rule)> {
        self.rules.iter().map(|(id, rule)| (*id, rule))
    }

    /// The rule of one category or subcategory, if it has one.
    #[must_use]
    pub fn rule(&self, id: CategoryId) -> Option<&Rule> {
        self.rules.get(&id)
    }

    /// The style of one edge of one category or subcategory, if a rule gives one.
    #[must_use]
    pub fn style(&self, id: CategoryId, edge: Edge) -> Option<&RuleStyle> {
        self.rules.get(&id).and_then(|rule| rule.get(edge))
    }

    /// These rules with `style` for one edge of `id`, replacing the one it had. Not checked: ask
    /// [`Graphics::check`].
    #[must_use]
    pub fn with_style(&self, id: CategoryId, edge: Edge, style: RuleStyle) -> Self {
        let mut rules = self.rules.clone();
        *rules.entry(id).or_default().slot(edge) = Some(style);
        Self { rules }
    }

    /// These rules without the rule of one edge of `id`, or of both when `edge` is `None`. A
    /// category left with no edge has no rule.
    #[must_use]
    pub fn without(&self, id: CategoryId, edge: Option<Edge>) -> Self {
        let mut rules = self.rules.clone();
        if let Some(rule) = rules.get_mut(&id) {
            match edge {
                Some(edge) => *rule.slot(edge) = None,
                None => *rule = Rule::default(),
            }
            if rule.is_empty() {
                rules.remove(&id);
            }
        }
        Self { rules }
    }

    /// These rules as a view of `kind` can hold them: none for a drafting view or a legend, and
    /// the projection rules only for a three-dimensional view. A view template uses this to apply
    /// one set of rules to views of every kind.
    #[must_use]
    pub fn for_kind(&self, kind: &ViewKind) -> Self {
        match kind {
            ViewKind::Drafting | ViewKind::Legend => Self::new(),
            ViewKind::ThreeD => {
                let mut rules = BTreeMap::new();
                for (id, rule) in &self.rules {
                    let kept = Rule {
                        cut: None,
                        projection: rule.projection.clone(),
                    };
                    if !kept.is_empty() {
                        rules.insert(*id, kept);
                    }
                }
                Self { rules }
            }
            _ => self.clone(),
        }
    }

    /// Asks whether a view of `kind` can hold these rules, whatever the categories are.
    ///
    /// # Errors
    /// [`ViewError::NoGraphicsOnKind`] for a drafting view or a legend, and
    /// [`ViewError::NoCutInKind`] for a cut rule in a three-dimensional view.
    pub fn check_kind(&self, kind: &ViewKind) -> Result<(), ViewError> {
        if self.is_empty() {
            return Ok(());
        }
        match kind {
            ViewKind::Drafting | ViewKind::Legend => {
                Err(ViewError::NoGraphicsOnKind { kind: words(kind) })
            }
            ViewKind::ThreeD if self.rules.values().any(|rule| rule.cut.is_some()) => {
                Err(ViewError::NoCutInKind { kind: words(kind) })
            }
            _ => Ok(()),
        }
    }

    /// Asks whether a view of `kind` can hold these rules against `categories` and `lines`: every
    /// rule of the module's header.
    ///
    /// # Errors
    /// The first refusal, in the order of the categories: [`ViewError::NoGraphicsOnKind`],
    /// [`ViewError::NoCutInKind`], [`ViewError::UnknownCategory`], [`ViewError::NotStyled`],
    /// [`ViewError::CutOnUncuttable`], [`ViewError::FillOnProjection`],
    /// [`ViewError::NoSuchFill`], [`ViewError::NoSuchPen`], [`ViewError::NoSuchPattern`] and
    /// [`ViewError::TransparencyAbove`].
    pub fn check(
        &self,
        kind: &ViewKind,
        categories: &CategoryTable,
        lines: &LineStyles,
    ) -> Result<(), ViewError> {
        self.check_kind(kind)?;
        self.check_rules(categories, lines)
    }

    /// Asks whether these rules stand against `categories` and `lines` whatever kind of view holds
    /// them: [`Graphics::check`] without the question of the kind, which is what a view template,
    /// that is of no kind, is asked.
    ///
    /// # Errors
    /// The first refusal of [`Graphics::check`] after its two about the kind.
    pub fn check_rules(
        &self,
        categories: &CategoryTable,
        lines: &LineStyles,
    ) -> Result<(), ViewError> {
        for (id, rule) in self.rules() {
            let category = categories.get(id).ok_or(ViewError::UnknownCategory(id))?;
            if category.kind == CategoryKind::View {
                return Err(ViewError::NotStyled(id));
            }
            if rule.cut.is_some() && !category.cuttable {
                return Err(ViewError::CutOnUncuttable(id));
            }
            for (edge, style) in [(Edge::Cut, &rule.cut), (Edge::Projected, &rule.projection)] {
                if let Some(style) = style {
                    check_style(lines, category.kind, edge, style)?;
                }
            }
        }
        Ok(())
    }

    /// The stored text, or `None` for no rules (the record then has no key).
    pub(crate) fn to_text(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let rules = self
            .rules()
            .map(|(category, rule)| StoredRule {
                category,
                cut: rule.cut.clone(),
                projection: rule.projection.clone(),
            })
            .collect();
        // Plain structs of numbers and strings: serialising one cannot fail, and a record that
        // held nothing here would be refused when read.
        Some(
            serde_json::to_string(&Stored {
                format: GRAPHICS_FORMAT_VERSION,
                rules,
            })
            .unwrap_or_default(),
        )
    }

    /// Reads the stored text. The caller compares the record it makes of the result with the one
    /// it was given, so any other spelling of the same rules is refused there.
    pub(crate) fn from_text(text: &str) -> Result<Self, String> {
        let stored: Stored = serde_json::from_str(text)
            .map_err(|why| format!("holds graphics rules that do not read: {why}"))?;
        if stored.format != GRAPHICS_FORMAT_VERSION {
            return Err(format!(
                "holds graphics rules of version {}, which is not known",
                stored.format
            ));
        }
        let mut rules = BTreeMap::new();
        let mut last = None;
        for one in stored.rules {
            if last.is_some_and(|before| before >= one.category) {
                return Err("holds graphics rules not in ascending category order".to_owned());
            }
            last = Some(one.category);
            let rule = Rule {
                cut: one.cut,
                projection: one.projection,
            };
            if rule.is_empty() {
                return Err("holds a graphics rule with no style".to_owned());
            }
            rules.insert(one.category, rule);
        }
        if rules.is_empty() {
            return Err("holds graphics rules with no rule in them".to_owned());
        }
        Ok(Self { rules })
    }
}

/// Whether pen `number` is in the pen table a category of `kind` uses.
pub(crate) fn pen_exists(lines: &LineStyles, kind: CategoryKind, number: u16) -> bool {
    if uses_annotation_pens(kind) {
        lines
            .annotation_pens
            .pens()
            .iter()
            .any(|pen| pen.number == number)
    } else {
        lines.model_pens.pen(number).is_some()
    }
}

/// The values of one style, for a category of `kind`, on one edge.
///
/// # Errors
/// The refusal of the first value that is not allowed.
pub fn check_style(
    lines: &LineStyles,
    kind: CategoryKind,
    edge: Edge,
    style: &RuleStyle,
) -> Result<(), ViewError> {
    if !pen_exists(lines, kind, style.pen) {
        return Err(ViewError::NoSuchPen { pen: style.pen });
    }
    if lines.patterns.get(style.pattern).is_none() {
        return Err(ViewError::NoSuchPattern {
            pattern: style.pattern.0,
        });
    }
    if let Some(fill) = &style.fill {
        if edge == Edge::Projected {
            return Err(ViewError::FillOnProjection);
        }
        if !FILL_PATTERNS.contains(&fill.pattern.as_str()) {
            return Err(ViewError::NoSuchFill(fill.pattern.clone()));
        }
    }
    if style.transparency > MOST_TRANSPARENCY {
        return Err(ViewError::TransparencyAbove(style.transparency));
    }
    Ok(())
}
