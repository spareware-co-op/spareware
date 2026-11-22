//! Fuzz targets for stored and typed text. Each runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package construct-params`, as construct-model's do.

use bolero::check;
use construct_params::{Area, Choice, Definition, Holder, Kind, Length, Units, Value, Volume};
use kit_core::Id;

/// Any text at all reads to nothing or to the value whose stored text is exactly that text, so a
/// stored value has one spelling and reading never panics. The one other text read is a length,
/// area or volume in version 1's spelling (whole millimetres, FORMAT.md): exactly that many
/// millimetres, whose version 2 text then reads back to the same value.
#[test]
fn stored_values_have_one_spelling() {
    check!().for_each(|bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        if let Some(value) = Value::from_stored(&text) {
            if value.stored() != text {
                let (tag, mm) = text.split_once(':').unwrap_or_default();
                let mm: i64 = mm.parse().unwrap_or_default();
                let old = match value {
                    Value::Length(_) => {
                        Value::Length(Length::checked_from_mm(mm).unwrap_or(Length::from_steps(0)))
                    }
                    Value::Area(_) => Value::Area(Area::from_mm2(mm)),
                    Value::Volume(_) => Value::Volume(Volume::from_mm3(mm)),
                    _ => Value::Whole(0),
                };
                assert_eq!(value, old, "{text}");
                assert_eq!(format!("{tag}:{mm}"), text);
            }
            assert_eq!(Value::from_stored(&value.stored()), Some(value));
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
