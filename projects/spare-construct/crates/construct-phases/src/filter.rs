//! The rule a view uses to decide whether an element is shown, and in which state.
//!
//! A view has a phase (the moment in the work it shows) and a phase filter (which of the states
//! below it draws). An element is, relative to that phase:
//!
//! - **new**: created in the view's phase and still standing at its end;
//! - **existing**: created in an earlier phase and still standing at the end of the view's phase;
//! - **demolished**: created earlier and demolished in the view's phase;
//! - **temporary**: created and demolished within the view's phase;
//! - not shown at all: created in a later phase, or demolished in an earlier one.
//!
//! The filter then keeps some of the four states. A view also picks one option per option set;
//! where it picks none for a set, the set's primary shows. Drawing the states is a later job.

use crate::error::PhaseError;
use crate::options::{DesignOptions, OptionRef};
use crate::phases::{ElementStaging, PhaseList};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// How an element stands in a view's phase.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ElementState {
    /// Built in an earlier phase and standing through this one.
    Existing,
    /// Built in this phase and standing at its end.
    New,
    /// Built earlier and taken down in this phase.
    Demolished,
    /// Built and taken down within this phase.
    Temporary,
}

/// Which states a view draws. Seven are offered: everything, the three single-purpose drawings
/// staged work needs (what is built, what stands, what is removed), and three pairings (the
/// building as found, as left, and the work of the phase).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseFilter {
    /// Everything: existing, new, demolished and temporary.
    ShowAll,
    /// What this phase builds: new and temporary.
    ShowNew,
    /// What stands through the phase: existing only.
    ShowExisting,
    /// What the phase removes: demolished only.
    ShowDemolished,
    /// The building as found at the start of the phase: existing and demolished.
    ShowExistingAndDemolished,
    /// The building as left at the end of the phase: existing and new.
    ShowExistingAndNew,
    /// The work of the phase: new, temporary and demolished.
    ShowNewAndDemolished,
}

impl PhaseFilter {
    /// Whether the filter draws an element in this state.
    #[must_use]
    pub fn shows(self, state: ElementState) -> bool {
        use ElementState::{Demolished, Existing, New, Temporary};
        match self {
            Self::ShowAll => true,
            Self::ShowNew => matches!(state, New | Temporary),
            Self::ShowExisting => state == Existing,
            Self::ShowDemolished => state == Demolished,
            Self::ShowExistingAndDemolished => matches!(state, Existing | Demolished),
            Self::ShowExistingAndNew => matches!(state, Existing | New),
            Self::ShowNewAndDemolished => matches!(state, New | Temporary | Demolished),
        }
    }
}

/// A view's choice of option for one set.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OptionChoice {
    /// The option set.
    #[serde(with = "kit_ops::id")]
    pub set: Id,
    /// The option the view shows from it.
    #[serde(with = "kit_ops::id")]
    pub option: Id,
}

/// What a view holds for this rule: its phase, its filter and its option choices.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ViewSettings {
    /// The phase the view shows.
    #[serde(with = "kit_ops::id")]
    pub phase: Id,
    /// Which states it draws.
    pub filter: PhaseFilter,
    /// The option it picks per set. A set not listed shows its primary.
    pub choices: Vec<OptionChoice>,
}

/// The answer for one element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Not drawn: not built yet, already gone, filtered out, or in an option the view does not show.
    Hidden,
    /// Drawn, in this state.
    Shown(ElementState),
}

/// How an element stands in the phase at position `at`, or `None` if it is not there at all.
fn state_in(created: usize, demolished: Option<usize>, at: usize) -> Option<ElementState> {
    if created > at {
        return None;
    }
    match (created == at, demolished.map(|d| d.cmp(&at))) {
        (_, Some(Ordering::Less)) => None,
        (true, Some(Ordering::Equal)) => Some(ElementState::Temporary),
        (true, _) => Some(ElementState::New),
        (false, Some(Ordering::Equal)) => Some(ElementState::Demolished),
        (false, _) => Some(ElementState::Existing),
    }
}

/// Decides whether a view shows an element, and in which state.
///
/// Refused, rather than guessed at, if the view's phase, the element's phases or option, or a
/// choice names something the project does not have, if the element's demolished phase comes
/// before its created phase, or if the view chooses twice for one set.
///
/// # Errors
///
/// Returns the [`PhaseError`] that names the refusal described above.
pub fn visibility(
    phases: &PhaseList,
    options: &DesignOptions,
    view: &ViewSettings,
    element: &ElementStaging,
) -> Result<Visibility, PhaseError> {
    let at = phases
        .position(&view.phase)
        .ok_or_else(|| PhaseError::UnknownPhase(view.phase.clone()))?;
    element.check(phases, options)?;
    for (index, choice) in view.choices.iter().enumerate() {
        if view
            .choices
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.set == choice.set)
        {
            return Err(PhaseError::DuplicateChoice(choice.set.clone()));
        }
        options.check(&OptionRef {
            set: choice.set.clone(),
            option: choice.option.clone(),
        })?;
    }

    let created = phases
        .position(&element.created)
        .ok_or_else(|| PhaseError::UnknownPhase(element.created.clone()))?;
    let demolished = match &element.demolished {
        Some(id) => Some(
            phases
                .position(id)
                .ok_or_else(|| PhaseError::UnknownPhase(id.clone()))?,
        ),
        None => None,
    };
    let Some(state) = state_in(created, demolished, at) else {
        return Ok(Visibility::Hidden);
    };
    if !view.filter.shows(state) {
        return Ok(Visibility::Hidden);
    }

    if let Some(reference) = &element.design_option {
        let chosen = view
            .choices
            .iter()
            .find(|choice| choice.set == reference.set)
            .map(|choice| &choice.option)
            .or_else(|| options.set(&reference.set).map(|set| &set.primary));
        if chosen != Some(&reference.option) {
            return Ok(Visibility::Hidden);
        }
    }
    Ok(Visibility::Shown(state))
}
