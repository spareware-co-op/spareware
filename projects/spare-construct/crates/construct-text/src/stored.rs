//! Reading stored values out of a record's `props`: only the one spelling is read.

use construct_model::element::Element;
use construct_params::{Length, Value};

/// A stored value under `key`, if it is in its one spelling.
pub(crate) fn value(record: &Element, key: &str) -> Option<Value> {
    let text = record.props.get(key)?;
    Value::from_stored(text).filter(|value| value.stored() == *text)
}

/// A stored length under `key`, if it is in its one spelling.
pub(crate) fn length(record: &Element, key: &str) -> Option<Length> {
    match value(record, key)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}
