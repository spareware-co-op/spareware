//! Reading a parameter of an instance: the one function every reader uses.

use crate::element_type::ElementType;
use crate::error::TypeError;
use crate::instance::Instance;
use crate::parameters::Parameters;
use construct_params::Value;
use kit_core::Id;

/// Where a resolved value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The instance's own value.
    Instance,
    /// Its type's value.
    Type,
    /// The parameter's default.
    Default,
    /// Nothing holds a value and the parameter has no default.
    Unset,
}

/// A parameter's value as an instance reads it, and where it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved<'a> {
    /// The value; `None` only when [`Resolved::from`] is [`Source::Unset`].
    pub value: Option<&'a Value>,
    /// Where it came from.
    pub from: Source,
}

/// The value of parameter `id` on `instance`, whose type is `ty`: the instance's own value, else
/// its type's, else the parameter's default, else unset. An instance holds values only of
/// parameters each instance holds and a type only of those the type holds, so for any one
/// parameter at most one of the first two can be there; the order is the same for all.
///
/// # Errors
/// [`TypeError::NotItsType`] if `ty` is not the instance's type; [`TypeError::UnknownParameter`]
/// if `params` does not define `id`; [`TypeError::NotOnType`] if the type's elements lack it.
pub fn resolve<'a>(
    params: &'a Parameters,
    ty: &'a ElementType,
    instance: &'a Instance,
    id: &Id,
) -> Result<Resolved<'a>, TypeError> {
    instance.its_type(ty)?;
    ty.require_param(params, id)?;
    let (value, from) = if let Some(value) = instance.values().get(id) {
        (Some(value), Source::Instance)
    } else if let Some(value) = ty.value(id) {
        (Some(value), Source::Type)
    } else if let Some(value) = params.default_of(id) {
        (Some(value), Source::Default)
    } else {
        (None, Source::Unset)
    };
    Ok(Resolved { value, from })
}
