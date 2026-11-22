//! Fuzz targets for stored and typed text. Each runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package construct-params`, as construct-model's do.

use bolero::check;
use construct_params::{Choice, Definition, Holder, Kind, Units, Value};
use kit_core::Id;

/// Any text at all reads to nothing or to the value whose stored text is exactly that text, so a
/// stored value has one spelling and reading never panics.
#[test]
fn stored_values_have_one_spelling() {
    check!().for_each(|bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        if let Some(value) = Value::from_stored(&text) {
            assert_eq!(value.stored(), text);
        }
    });
}

/// Any typed text either is refused or reads to a value its parameter accepts, which formats to
/// text that reads back to that same value's display.
#[test]
fn typed_text_reads_to_an_accepted_value_or_is_refused() {
    let units = Units::metric();
    let Ok(id) = Id::parse("P") else {
        return;
    };
    let mut defs: Vec<Definition> = Kind::ALL
        .iter()
        .filter(|kind| **kind != Kind::Choice)
        .filter_map(|kind| Definition::new(id.clone(), "P", *kind, Holder::Instance).ok())
        .collect();
    if let Ok(choice) = Definition::choice(
        id,
        "P",
        Holder::Type,
        vec![Choice::new("oak", "Oak"), Choice::new("ash", "Ash")],
    ) {
        defs.push(choice);
    }
    check!().for_each(|bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        for def in &defs {
            if let Ok(value) = def.parse(&text, &units) {
                assert_eq!(def.check(&value), Ok(()));
                let shown = def.format(&value, &units).unwrap();
                let again = def.parse(&shown, &units).unwrap();
                assert_eq!(def.format(&again, &units).unwrap(), shown);
            }
        }
    });
}
