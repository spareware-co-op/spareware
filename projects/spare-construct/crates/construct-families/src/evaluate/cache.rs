//! Evaluated types, kept so that the same recipe and the same values are built once.
//!
//! The key is the recipe version's content id and every value the evaluation reads that is not
//! worked out: each parameter without a formula, in recipe order, with the type's value or, if
//! the type sets none, its default, in stored text; then any value set for a key the recipe has
//! no such parameter for (an evaluation that then fails, and so is never kept). Two types with
//! the same values share one solid; a changed value is a new key, so only that type is built
//! again, and every other type is found as it was. The key holds no clock, no address and no
//! hash-map order: the same inputs give the same key on every machine.
//!
//! A named type (a built-in type or a type table row) is keyed by its values, not its name: two
//! named types with the same values share one evaluation, and a type table is evaluated row by
//! row through the same cache. The limits are the evaluator's, and are not part of the key: an
//! evaluator keeps only what it built within its own limits.

use super::Evaluated;
use super::build::run;
use super::error::EvalError;
use super::limits::Limits;
use super::values::TypeValues;
use crate::model::{Key, ParameterValue, Recipe};
use crate::write::ContentId;
use engine_geometry::value::engine::Engine;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

/// What one evaluation is kept under.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    recipe: ContentId,
    values: Vec<(Key, String)>,
}

impl CacheKey {
    /// The key of `recipe` evaluated with `values`.
    #[must_use]
    pub fn of(recipe: &Recipe, values: &TypeValues) -> Self {
        let mut read: Vec<(Key, String)> = Vec::with_capacity(recipe.parameters.len());
        for p in &recipe.parameters {
            if let ParameterValue::Default(default) = &p.value {
                let text = values.get(p.key.as_str()).unwrap_or(default);
                read.push((p.key.clone(), text.to_owned()));
            }
        }
        for (key, text) in values.iter() {
            let known = recipe
                .parameters
                .iter()
                .any(|p| p.key == *key && matches!(p.value, ParameterValue::Default(_)));
            if !known {
                read.push((key.clone(), text.to_owned()));
            }
        }
        Self {
            recipe: recipe.content_id(),
            values: read,
        }
    }

    /// The recipe version.
    #[must_use]
    pub fn recipe(&self) -> &ContentId {
        &self.recipe
    }

    /// The values read, by parameter key, in stored text, in the order above.
    #[must_use]
    pub fn values(&self) -> &[(Key, String)] {
        &self.values
    }
}

/// One type's evaluation as an [`Evaluator`] hands it out (shared), or why there is none.
pub type TypeResult<S> = Result<Arc<Evaluated<S>>, EvalError>;

/// Recipe evaluation through one engine, with every evaluated type kept by its [`CacheKey`].
pub struct Evaluator<E: Engine> {
    engine: E,
    limits: Limits,
    cache: BTreeMap<CacheKey, Arc<Evaluated<E::Solid>>>,
    builds: u64,
}

impl<E: Engine> Evaluator<E> {
    /// An evaluator asking `engine` for its solids, with an empty cache and the default limits.
    pub fn new(engine: E) -> Self {
        Self::with_limits(engine, Limits::DEFAULT)
    }

    /// An evaluator with these limits, for every evaluation it builds.
    pub fn with_limits(engine: E, limits: Limits) -> Self {
        Self {
            engine,
            limits,
            cache: BTreeMap::new(),
            builds: 0,
        }
    }

    /// `recipe` evaluated for the type whose values are `values`: as kept, if this recipe
    /// version was evaluated with these values before, or built now and kept. The evaluation is
    /// shared, so the engine stays free to mesh or measure its solids ([`Evaluator::engine_mut`]).
    ///
    /// # Errors
    ///
    /// As [`crate::evaluate()`], within this evaluator's limits. A refusal is not kept: the same
    /// inputs are tried again next time.
    pub fn evaluate(
        &mut self,
        recipe: &Recipe,
        values: &TypeValues,
    ) -> Result<Arc<Evaluated<E::Solid>>, EvalError> {
        match self.cache.entry(CacheKey::of(recipe, values)) {
            Entry::Occupied(kept) => Ok(Arc::clone(kept.get())),
            Entry::Vacant(slot) => {
                let built = run(&mut self.engine, recipe, values, self.limits)?;
                self.builds = self.builds.saturating_add(1);
                Ok(Arc::clone(slot.insert(Arc::new(built))))
            }
        }
    }

    /// `recipe` evaluated for the type it names `ty` (a built-in type or a type table row), as
    /// [`Evaluator::evaluate`] with that type's values ([`TypeValues::of`]).
    ///
    /// # Errors
    ///
    /// As [`TypeValues::of`], then as [`Evaluator::evaluate`].
    pub fn evaluate_type(
        &mut self,
        recipe: &Recipe,
        ty: &Key,
    ) -> Result<Arc<Evaluated<E::Solid>>, EvalError> {
        let values = TypeValues::of(recipe, ty)?;
        self.evaluate(recipe, &values)
    }

    /// Every type `recipe` names evaluated, one by one through the cache, in the order
    /// [`Recipe::named_types`] gives (the built-in types, then the type table's rows): each
    /// type's key with its evaluation or its refusal. One type's refusal does not stop the rest;
    /// types with the same values share one evaluation.
    pub fn evaluate_types(&mut self, recipe: &Recipe) -> Vec<(Key, TypeResult<E::Solid>)> {
        recipe
            .named_types()
            .into_iter()
            .map(|ty| {
                let key = ty.key().clone();
                let result = self.evaluate_type(recipe, &key);
                (key, result)
            })
            .collect()
    }

    /// How many evaluations this evaluator has built (not found kept).
    #[must_use]
    pub fn builds(&self) -> u64 {
        self.builds
    }

    /// How many evaluated types are kept.
    #[must_use]
    pub fn kept(&self) -> usize {
        self.cache.len()
    }

    /// Whether an evaluation is kept under `key`.
    #[must_use]
    pub fn has(&self, key: &CacheKey) -> bool {
        self.cache.contains_key(key)
    }

    /// Drops every type kept for this recipe version.
    pub fn forget(&mut self, recipe: &ContentId) {
        self.cache.retain(|key, _| key.recipe != *recipe);
    }

    /// The limits it evaluates within.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// The engine.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// The engine, to mesh or measure the solids it made.
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// The engine, given back, with the cache dropped.
    pub fn into_engine(self) -> E {
        self.engine
    }
}
