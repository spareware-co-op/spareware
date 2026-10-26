//! The climate hazard table and the hand-over packs against the JavaScript's recorded answers (expected/compliance-f3-packs.json, from
//! tools/transport-truth/gen-compliance-f3.mjs).

mod compliance_f3_support;
mod compliance_support;
mod support;

use compliance_f3_support::{Mock, check_ok, check_text, load, project, set_clock};
use construct_transport::compliance::extra;
use construct_transport::compliance::extra::climate;
use construct_transport::compliance::extra::packs::{self, Section};
use construct_transport::compliance::jv::{Jx, V};

fn sections_of(v: &V) -> Vec<Section> {
    v.items()
        .iter()
        .map(|s| Section {
            title: s.g("title").s(),
            note: if s.g("note").t() { Some(s.g("note").s()) } else { None },
            head: s.g("head").items().iter().map(|h| h.s()).collect(),
            rows: s.g("rows").items().iter().map(|r| r.items().to_vec()).collect(),
            empty: if s.g("empty").t() { Some(s.g("empty").s()) } else { None },
        })
        .collect()
}

#[test]
fn climate_and_packs_match_the_prototype() {
    set_clock();
    let all = load("f3-packs");
    let cases = all.g("cases").items();
    assert!(cases.len() > 1200, "{} cases", cases.len());
    for case in cases {
        let id = case.g("id").s();
        let args = case.g("args");
        match case.g("call").s().as_str() {
            "climate" => {
                let p = project(case);
                let got = if args.has("sci") { climate::climate(&p, Some(p.g("scenarios").ix(args.g("sci").n() as usize))) } else { climate::climate(&p, None) };
                check_ok(case, &got);
            }
            "climateCSV" => check_text(case, &climate::climate_csv(&project(case))),
            "r1" => check_ok(case, &packs::r1(args.g("v"))),
            "r3" => check_ok(case, &packs::r3(args.g("v"))),
            "signalNodes" => {
                let p = project(case);
                let ids = packs::signal_nodes(p.g("scenarios").ix(args.g("sci").n() as usize));
                check_ok(case, &V::Array(ids.into_iter().map(V::String).collect()));
            }
            "packCSV" => {
                let p = project(case);
                check_text(case, &packs::pack_csv(&p, "A title", &args.g("tool").s(), &sections_of(args.g("sections"))));
            }
            "runDesignLife" => {
                let mut p = project(case);
                let res = packs::run_design_life(&mut p, case.g("opts"));
                let mut o = V::object();
                o.set("res", res);
                o.set("saved", p.g("complianceExtra").g("tsdg").g("result").clone());
                check_ok(case, &o);
            }
            "tsdgPack" => check_text(case, &packs::tsdg_pack(&project(case))),
            "ssaPack" => check_text(case, &packs::ssa_pack(&project(case))),
            "shPack" => check_text(case, &packs::sh_pack(&project(case))),
            "mbcmPack" => check_text(case, &packs::mbcm_pack(&project(case))),
            "rltpSheet" => check_text(case, &packs::rltp_sheet(&project(case))),
            "msmPack" => check_text(case, &packs::msm_pack(&project(case))),
            "anopInputsCSV" => check_text(case, &packs::anop_inputs_csv(&project(case))),
            "handoverManifest" => check_ok(case, &packs::handover_manifest(&project(case))),
            "report" => {
                let p = project(case);
                let rid = case.g("report").s();
                let got = if case.g("ctx").t() { extra::report(&rid, &p, Some(&Mock)) } else { extra::report(&rid, &p, None) };
                check_text(case, &got.unwrap_or_else(|| panic!("{id}: no report section {rid}")));
            }
            "export" => {
                let p = project(case);
                let eid = case.g("export").s();
                let mut got = V::object();
                got.set("build", V::String(extra::export_build(&eid, &p).unwrap_or_else(|| panic!("{id}: no export {eid}"))));
                got.set("enabled", V::Bool(extra::export_enabled(&eid, &p).expect("export")));
                got.set("label", case.g("ok").g("label").clone());
                check_ok(case, &got);
            }
            other => panic!("{id}: unknown call {other}"),
        }
    }
}
