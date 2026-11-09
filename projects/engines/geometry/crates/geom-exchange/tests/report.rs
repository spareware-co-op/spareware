//! Tests of `geom_exchange::report` (piece G413): the healing tolerance proposed from a file's
//! declared precision, and import and export reports that keep every entry in the order recorded.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_exchange::ExchangeError;
use geom_exchange::report::{
    BodyReport, ExportReport, Failed, Format, Given, HealAction, HealKind, HealTolerance,
    ImportReport, Rounding, SourceKey, UnitSource, Units, Unsupported,
};
use geom_exchange::unit::{self, AngleUnit, SiPrefix};
use geom_exchange::write::{self, Deviation};
use geom_math::{Box3, CrsId, Decimal, FrameRecord, LinearUnit};
use geom_num::grid::{GridStep, P3};
use geom_num::{Exactness, Steps};

fn d(t: &str) -> Decimal {
    Decimal::parse(t).unwrap()
}

fn units() -> Units {
    Units {
        length: unit::metre(Some(SiPrefix::Milli)).unwrap(),
        length_source: UnitSource::Declared,
        angle: AngleUnit::Degree,
        angle_source: UnitSource::Named,
    }
}

fn frame() -> FrameRecord {
    FrameRecord::map(CrsId::Epsg(2193), GridStep::Q0, [0, 0, 0]).unwrap()
}

#[test]
fn the_healing_tolerance_is_proposed_within_its_range() {
    let m = LinearUnit::METRE;
    let mm = unit::metre(Some(SiPrefix::Milli)).unwrap();
    let proposed = |p: Option<&str>, u| {
        HealTolerance::propose(p.map(d), u, GridStep::Q0)
            .unwrap()
            .proposed()
    };
    // None declared: the default, 640 steps (0.1 mm at q0).
    assert_eq!(proposed(None, m), Steps(640));
    // IFC's common 1e-5 m is 64 steps: raised to 640.
    assert_eq!(proposed(Some("1e-5"), m), Steps(640));
    // 0.5 mm is 3,200 steps: proposed as declared.
    assert_eq!(proposed(Some("0.5"), mm), Steps(3_200));
    // 0.01 m is 64,000 steps: capped at 6,400 (1 mm).
    assert_eq!(proposed(Some("0.01"), m), Steps(6_400));
    // Beyond the grid's range: the cap.
    assert_eq!(proposed(Some("1e9"), m), Steps(6_400));
    assert_eq!(
        HealTolerance::propose(Some(d("0")), m, GridStep::Q0),
        Err(ExchangeError::NotPositive)
    );
    assert_eq!(
        HealTolerance::propose(Some(d("-1e-5")), m, GridStep::Q0),
        Err(ExchangeError::NotPositive)
    );
}

#[test]
fn the_tolerance_used_is_the_default_until_the_caller_chooses() {
    let t = HealTolerance::propose(Some(d("0.5")), LinearUnit::METRE, GridStep::Q0).unwrap();
    assert_eq!(t.used(), Steps(640));
    let accepted = t.with_used(t.proposed()).unwrap();
    assert_eq!(accepted.used(), Steps(6_400));
    assert_eq!(t.with_used(Steps(1)).unwrap().used(), Steps(1));
    assert_eq!(
        t.with_used(Steps(0)),
        Err(ExchangeError::OutOfRange { count: 0 })
    );
    assert_eq!(
        t.with_used(Steps(6_401)),
        Err(ExchangeError::OutOfRange { count: 6_401 })
    );
}

#[test]
fn an_import_report_keeps_every_entry_in_order() {
    let heal = HealTolerance::propose(None, LinearUnit::METRE, GridStep::Q0).unwrap();
    let mut r = ImportReport::new(
        [7; 32],
        Format::Ifc,
        units(),
        frame(),
        Some(d("1e-5")),
        heal,
    );
    let inch = unit::conversion_based(d("25.4"), units().length).unwrap();
    let written = write::length(1, inch, GridStep::Q0).unwrap();
    for (i, key) in [
        SourceKey::Instance(12),
        SourceKey::Persistent("2O2Fr$t4X7Zf8NOew3FLOH".to_owned()),
    ]
    .into_iter()
    .enumerate()
    {
        r.record_rounding(Rounding {
            item: key.clone(),
            attribute: format!("Coordinates[{i}]"),
            given: Given::Decimal(d("0.00001")),
            count: 0,
            deviation: Some(written.deviation()),
        })
        .unwrap();
        r.record_healing(HealAction {
            item: key.clone(),
            kind: HealKind::VerticesMerged,
        })
        .unwrap();
        r.record_unsupported(Unsupported {
            entity: "IFCFIXEDREFERENCESWEPTAREASOLID".to_owned(),
            item: key.clone(),
            remedy: "export it as a B-rep".to_owned(),
        })
        .unwrap();
        r.record_failed(Failed {
            item: key.clone(),
            reason: "a face that does not close".to_owned(),
        })
        .unwrap();
        r.record_body(BodyReport {
            item: key,
            class: Exactness::Exact,
            tolerant: false,
            bounds: Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 1).unwrap()).unwrap(),
            deviation: Deviation::ZERO,
        })
        .unwrap();
    }
    assert_eq!(r.roundings().len(), 2);
    assert_eq!(r.roundings()[1].attribute, "Coordinates[1]");
    assert_eq!(r.healing()[0].item, SourceKey::Instance(12));
    assert_eq!(r.unsupported().len(), 2);
    assert_eq!(r.failed().len(), 2);
    assert_eq!(
        r.bodies()[1].item,
        SourceKey::Persistent("2O2Fr$t4X7Zf8NOew3FLOH".to_owned())
    );
    // The same events give the same report.
    let copy = r.clone();
    assert_eq!(copy, r);
}

#[test]
fn an_export_report_records_what_did_not_write_exactly() {
    let inch =
        unit::conversion_based(d("25.4"), unit::metre(Some(SiPrefix::Milli)).unwrap()).unwrap();
    let mut r = ExportReport::new(Format::Step, units(), frame(), Some(d("1e-6")), false);
    for n in [127_i64, 1, 2] {
        let w = write::length(n, inch, GridStep::Q0).unwrap();
        if !w.is_exact() {
            r.record_rounding(Rounding {
                item: SourceKey::Instance(n.unsigned_abs()),
                attribute: "Coordinates[0]".to_owned(),
                given: Given::Decimal(w.decimal()),
                count: n,
                deviation: Some(w.deviation()),
            })
            .unwrap();
        }
    }
    // 127 steps is 1/1,280 inch exactly; 1 and 2 steps are not.
    assert_eq!(r.roundings().len(), 2);
    assert_eq!(r.roundings()[0].count, 1);
    assert_eq!(r.failed().len(), 0);
    assert!(!r.accepted_invalid_rounding);
    // An item that could not be written is listed, with its reason.
    let lost = Failed {
        item: SourceKey::Instance(9),
        reason: "a body with no faces".to_owned(),
    };
    r.record_failed(lost.clone()).unwrap();
    assert_eq!(r.failed(), [lost].as_slice());
}

#[test]
fn every_error_says_what_happened() {
    use geom_math::DecimalError;
    use geom_num::NumError;
    let all = [
        ExchangeError::UnitOutOfRange,
        ExchangeError::InexactAngleUnit,
        ExchangeError::OutOfRange { count: -7 },
        ExchangeError::NotPositive,
        ExchangeError::RoundsToZero {
            finer: Some(GridStep::Q1),
        },
        ExchangeError::RoundsToZero { finer: None },
        ExchangeError::NotFinite,
        ExchangeError::RealOutOfRange,
        ExchangeError::Decimal(DecimalError::TooManyDigits),
        ExchangeError::ZeroDirection,
        ExchangeError::ScaleOutOfRange,
        ExchangeError::Numeric(NumError::BudgetExceeded),
        ExchangeError::OutOfMemory,
    ];
    let texts: Vec<String> = all.iter().map(ToString::to_string).collect();
    for (e, t) in all.iter().zip(&texts) {
        assert!(!t.is_empty(), "{e:?}");
    }
    // Each says something of its own.
    let mut unique = texts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), texts.len());
    assert!(texts[2].contains("-7"));
    // The numbers layer's out-of-range carries its count across.
    assert_eq!(
        ExchangeError::from(NumError::OutOfRange { value: 42 }),
        ExchangeError::OutOfRange { count: 42 }
    );
    assert_eq!(
        ExchangeError::from(DecimalError::ExponentRange),
        ExchangeError::Decimal(DecimalError::ExponentRange)
    );
}
