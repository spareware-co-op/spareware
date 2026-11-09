//! The 87-item regulatory checklist (packs/nz/compliance.js section 14): which items apply to the pathway, what Spare Construct finds for the
//! items it computes, and the author's own record for the rest.

use super::access::{access_check, operators, sh_check};
use super::authority::authorities;
use super::clocks::clocks;
use super::data::c as table;
use super::docs::{sign_state, stamp_csv};
use super::e27::{e27, ita_scope, scoping_memo, sh_involved};
use super::engine;
use super::fds::fds_check;
use super::jv::{Jx, V, csv, st};
use super::record::{active_sc, in_auckland, num, pathway_entry, regime, state};
use super::rfi;
use super::rx::rx;
use std::cell::OnceCell;

/// The project facts the checklist items read; the expensive checks are worked out on first use.
pub struct X<'a> {
    /// The project.
    pub p: &'a V,
    /// The compliance record.
    pub c: V,
    /// The active scenario.
    pub sc: &'a V,
    /// The regime.
    pub r: V,
    /// The pathway entry.
    pub pe: Option<&'static V>,
    /// The site.
    pub site: V,
    /// The pathway key.
    pub pathway: String,
    /// In Auckland.
    pub auck: bool,
    /// Lodged in the transition regime.
    pub trans: bool,
    /// A transport precinct applies.
    pub precinct: bool,
    e27: OnceCell<V>,
    ita: OnceCell<V>,
    shc: OnceCell<V>,
    auth: OnceCell<V>,
    sign: OnceCell<V>,
    lint: OnceCell<V>,
    status: OnceCell<V>,
    memo: OnceCell<V>,
    acc: OnceCell<Vec<V>>,
    fds: OnceCell<V>,
    ops: OnceCell<V>,
    clocks: OnceCell<V>,
}

impl<'a> X<'a> {
    /// `X` for a project.
    pub fn new(p: &'a V) -> X<'a> {
        let c = state(p);
        let sc = active_sc(p);
        let r = regime(p);
        let pathway = c.g("ctx").g("pathway").s();
        let site = super::authority::site_info(p);
        X {
            p,
            pe: pathway_entry(c.g("ctx").g("pathway")),
            trans: r.g("side").eq_s("transition"),
            precinct: site.g("transportPrecinct").t(),
            auck: in_auckland(p),
            pathway,
            site,
            r,
            c,
            sc,
            e27: OnceCell::new(),
            ita: OnceCell::new(),
            shc: OnceCell::new(),
            auth: OnceCell::new(),
            sign: OnceCell::new(),
            lint: OnceCell::new(),
            status: OnceCell::new(),
            memo: OnceCell::new(),
            acc: OnceCell::new(),
            fds: OnceCell::new(),
            ops: OnceCell::new(),
            clocks: OnceCell::new(),
        }
    }
    /// The E27 screening.
    pub fn e27(&self) -> &V {
        self.e27.get_or_init(|| e27(self.p))
    }
    /// The ITA scope.
    pub fn ita(&self) -> &V {
        self.ita.get_or_init(|| ita_scope(self.p))
    }
    /// The state highway check.
    pub fn shc(&self) -> &V {
        self.shc.get_or_init(|| sh_check(self.p, self.sc))
    }
    /// The authorities.
    pub fn auth(&self) -> &V {
        self.auth.get_or_init(|| authorities(self.p, &V::Null))
    }
    /// The sign-off state.
    pub fn sign(&self) -> &V {
        self.sign.get_or_init(|| sign_state(self.p))
    }
    /// The pre-lodgement check.
    pub fn lint(&self) -> &V {
        self.lint.get_or_init(|| rfi::lint(self.p))
    }
    /// The AM model status.
    pub fn status(&self) -> &V {
        self.status
            .get_or_init(|| engine::model_status(self.p, "AM"))
    }
    /// The scoping memo.
    pub fn memo(&self) -> &V {
        self.memo.get_or_init(|| scoping_memo(self.p, self.sc))
    }
    /// The access checks.
    pub fn acc(&self) -> &Vec<V> {
        self.acc.get_or_init(|| {
            self.c
                .g("access")
                .items()
                .iter()
                .map(|a| access_check(self.p, a, self.sc))
                .collect()
        })
    }
    /// The FDS check.
    pub fn fds(&self) -> &V {
        self.fds.get_or_init(|| fds_check(self.p))
    }
    /// The affected operators.
    pub fn ops(&self) -> &V {
        self.ops.get_or_init(|| operators(self.p))
    }
    /// The statutory clocks.
    pub fn clocks(&self) -> &V {
        self.clocks.get_or_init(|| clocks(self.p))
    }
    fn sh(&self) -> bool {
        sh_involved(self.p, self.sc).g("involved").t()
    }
    fn ita_likely(&self) -> bool {
        let s = self.ita();
        s.g("levelIndex").n() >= 2.0
            || s.g("at").jlen() > 0
            || self
                .pe
                .is_some_and(|e| e.g("kind").eq_s("pc") || e.g("kind").eq_s("nor"))
    }
}

/// `applies(item, X)`: whether a checklist item applies to the project's pathway.
pub fn applies(item: &V, x: &X) -> bool {
    let k = x.pe.map_or(String::new(), |e| e.g("kind").s());
    let id = item.g("id").s();
    let w = item.g("pathway").s();
    let (rc, pc, nor, ftaa) = (k == "rc", k == "pc", k == "nor", k == "ftaa");
    let rmaish = rc || pc || nor || ftaa;
    let has_p = x.pe.is_some();
    let t = |re: &str, s: &str| rx(re, "").test(s);
    if t(r"\(Auckland\)", &w) && !x.auck {
        return false;
    }
    match item.g("group").s().as_str() {
        "X" => {
            if t("RMA and FTAA", &w) {
                rmaish
            } else if t("modelled evidence", &w) {
                true
            } else if t("ITAs in Auckland", &w) {
                x.auck && (!has_p || rmaish) && x.ita_likely()
            } else {
                true
            }
        }
        "RC" => {
            if t("lodged from 30 Oct 2026", &w) {
                (rc || nor || (ftaa && t("NoR", &w)))
                    && x.trans
                    && (!t("District land use", &w) || x.pathway == "rc-landuse")
            } else if t("Urban subdivision", &w) {
                x.pathway == "rc-subdivision"
            } else if t("^ITAs", &w) {
                (!has_p || rmaish)
                    && x.ita_likely()
                    && (!t("to vest", &w) || x.c.g("ctx").g("vest").t())
            } else if t("Resource consent", &w) {
                rc || (ftaa && x.pathway == "ftaa-consent")
            } else {
                true
            }
        }
        "PS" => x.pathway == "precinct" || x.precinct,
        "PC" => {
            if t("Private", &w) {
                x.pathway == "pc-private"
            } else {
                pc
            }
        }
        "NOR" => x.pathway == "nor" || x.pathway == "outline-plan",
        "FT" => {
            if t("NoR", &w) {
                x.pathway == "ftaa-nor"
            } else if id == "FT-06" {
                ftaa && x.sh()
            } else {
                ftaa
            }
        }
        "SH" => x.sh(),
        "RCA" => x.auck && rmaish && (id != "RCA-02" || x.c.g("ctx").g("vest").t()),
        "FUND" => k == "fund",
        "SPD" => k == "speed",
        "DC" => k == "dc",
        "TOU" => k == "tou",
        "EV" => {
            if id == "EV-01" {
                rc || pc
            } else {
                rmaish
            }
        }
        _ => true,
    }
}

fn r(state: &str, text: impl Into<String>) -> Option<(String, String)> {
    Some((state.to_string(), text.into()))
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The automatic evaluator of a checklist item: `(state, text)`, None when Spare Construct has no automatic status for it.
pub fn auto(id: &str, x: &X) -> Option<(String, String)> {
    let p = x.p;
    match id {
        "X-01" => {
            let code = x.r.g("code");
            if code.eq_s("unset") || code.eq_s("undated") {
                r("todo", x.r.g("text").s())
            } else {
                r(
                    "done",
                    format!("{}: {}", x.r.g("pathwayLabel").s(), x.r.g("text").s()),
                )
            }
        }
        "X-02" => r(
            if x.r.g("date").t() { "done" } else { "todo" },
            if x.auth().g("transitional").t() {
                "Authority table ready; confirm whether a cl 8(2) direction ended the transitional period."
            } else {
                "Authority table and affected-operator list ready."
            },
        ),
        "X-03" => {
            let pr = x.c.g("proportion");
            if pr.g("confirmed").t() {
                r(
                    "done",
                    format!(
                        "Proportionality statement confirmed{}.",
                        if pr.g("by").t() {
                            format!(" by {}", pr.g("by").s())
                        } else {
                            String::new()
                        }
                    ),
                )
            } else {
                r(
                    "todo",
                    "Drafted from the screening metrics; the author must confirm it.",
                )
            }
        }
        "X-04" => {
            let sg = x.sign();
            if sg.g("signed").t() {
                let n = sg
                    .g("valid")
                    .items()
                    .iter()
                    .find(|s| s.g("role").eq_s("prepared"))
                    .map_or(String::new(), |s| s.g("name").s());
                r("done", format!("Signed off by {n}."))
            } else {
                r(
                    if sg.g("prompt").t() { "warn" } else { "todo" },
                    if sg.g("prompt").t() {
                        sg.g("prompt").s()
                    } else {
                        "Not signed off: exports carry the DRAFT watermark.".into()
                    },
                )
            }
        }
        "X-05" => {
            let l: Vec<String> = x
                .lint()
                .items()
                .iter()
                .filter(|l| rx(r"count-|rates$", "").test(&l.g("id").s()))
                .map(|l| l.g("text").s())
                .collect();
            if !l.is_empty() {
                r("warn", l.join("; "))
            } else {
                r(
                    "done",
                    "Data sources are dated (data vintage table in Appendix G).",
                )
            }
        }
        "X-06" => {
            if x.lint().items().iter().any(|l| l.g("id").eq_s("sens")) {
                r("todo", "No sensitivity scenario yet.")
            } else {
                r("done", "A sensitivity scenario exists.")
            }
        }
        "X-07" => {
            let s = x.status();
            r(
                if s.g("status").eq_s("validated") {
                    "done"
                } else {
                    "warn"
                },
                format!(
                    "{}. TMDG category {}{}.",
                    s.g("label").s(),
                    s.g("category").s(),
                    if s.g("justification").t() {
                        ""
                    } else {
                        " (justification not given)"
                    }
                ),
            )
        }
        "X-08" => {
            if p.g("data").g("msm").g("links").t() {
                r("done", "MSM growth loaded for comparison.")
            } else {
                r(
                    "warn",
                    engine::standards_table().g("msm").g("notChecked").s(),
                )
            }
        }
        "X-09" => {
            let m = x.memo();
            let n = m.items().iter().filter(|i| i.g("locked").t()).count();
            r(
                if n == m.jlen() { "done" } else { "todo" },
                format!("{} of {} scoping items agreed and locked.", n, m.jlen()),
            )
        }
        "RC-01" => {
            let (e, site) = (x.e27(), &x.site);
            if site.g("looked").t() || site.g("zone").t() {
                let z = if site.g("zone").t() {
                    site.g("zone").g("name").s()
                } else {
                    "zone not known".into()
                };
                let pr = if site.g("precinct").t() {
                    format!(" \u{b7} {}", site.g("precinct").s())
                } else {
                    String::new()
                };
                let pc = if e.g("pc120").t() {
                    format!(" \u{b7} {}", e.g("pc120").g("text").s())
                } else {
                    " \u{b7} PC120 layer not read".into()
                };
                r(
                    if e.g("pc120").t() && e.g("pc120").g("exemptDiffers").t() {
                        "warn"
                    } else {
                        "done"
                    },
                    format!("{z}{pr}{pc}"),
                )
            } else {
                r("todo", "Look up the site (AUP zone, precinct and PC120).")
            }
        }
        "RC-02" => {
            let e = x.e27();
            if e.g("redirect").t() {
                r("na", format!("{}.", e.g("redirect").g("text").s()))
            } else if e.g("exempt").t() {
                r("done", format!("{}.", e.g("exempt").g("text").s()))
            } else {
                r(
                    "done",
                    if e.g("tier").t() {
                        format!("E27 thresholds exceeded: tier {}.", e.g("tier").s())
                    } else {
                        "No E27.6.1 threshold exceeded.".into()
                    },
                )
            }
        }
        "RC-03" => {
            let e = x.e27();
            if x.c.g("ctx").g("activityStatus").t() {
                r(
                    "done",
                    format!(
                        "{} Busiest hour {} veh/h.",
                        e.g("catchAll").g("why").s(),
                        num(e.g("quantities").g("peak"), 0)
                    ),
                )
            } else {
                r(
                    "todo",
                    "Set the activity status to test E27.6.1(1)(b) and E27.9(5).",
                )
            }
        }
        "RC-04" => {
            let e = x.e27();
            if e.g("tier").t() {
                r(
                    "done",
                    format!(
                        "{}.",
                        if e.g("tier").n() == 1.0 {
                            table("E27.tier1Scope").s()
                        } else {
                            table("E27.tier2Scope").s()
                        }
                    ),
                )
            } else {
                r(
                    "na",
                    "No E27 trigger: no matters of discretion from E27.6.1.",
                )
            }
        }
        "RC-05" => r(
            "todo",
            format!(
                "{} objectives and policies apply; the planner writes the evaluation.",
                x.e27()
                    .g("matrix")
                    .items()
                    .iter()
                    .filter(|m| m.g("applies").t())
                    .count()
            ),
        ),
        "RC-07" => {
            let n = p.g("register").jlen();
            if n > 0 {
                r(
                    "done",
                    format!("{n} register rows, each with an inclusion reason."),
                )
            } else {
                r("todo", "The committed-development register is empty.")
            }
        }
        "RC-08" => {
            if p.g("assessment").t() {
                let b: usize = p
                    .g("scenarios")
                    .items()
                    .iter()
                    .map(|s| {
                        engine::scenario_check(p, s)
                            .items()
                            .iter()
                            .filter(|c| c.g("level").eq_s("block"))
                            .count()
                    })
                    .sum();
                r(
                    if b > 0 { "warn" } else { "done" },
                    if b > 0 {
                        format!("{b} blocking scenario check{}.", plural(b))
                    } else {
                        "Scenario matrix created; funding gating passes.".into()
                    },
                )
            } else {
                r("todo", "Create the assessment scenario set.")
            }
        }
        "RC-09" => {
            if x.sc.g("extent").t() {
                r(
                    "done",
                    format!(
                        "Extent stored: {} sites, {} links.",
                        x.sc.g("extent").g("sites").jlen(),
                        x.sc.g("extent").g("links").jlen()
                    ),
                )
            } else {
                r(
                    "todo",
                    "Run the extent test (5% / 10%) and store the extent.",
                )
            }
        }
        "RC-12" => {
            let c = p.g("data").g("loaded").g("cas");
            if c.t() && c.g("at").t() {
                r(
                    "done",
                    format!(
                        "CAS loaded: {}.",
                        if c.g("label").t() {
                            c.g("label").s()
                        } else {
                            format!("{}\u{2013}{}", c.g("from").sn(), c.g("to").sn())
                        }
                    ),
                )
            } else {
                r("todo", "Load CAS crashes for the study area.")
            }
        }
        "RC-17" => {
            if x.c.g("access").jlen() == 0 {
                return r("todo", "Add the proposed access points.");
            }
            let acc = x.acc();
            let nc = acc.iter().filter(|a| a.g("unchecked").jlen() > 0).count();
            let cnt = |w: &str| acc.iter().filter(|a| a.g("worst").eq_s(w)).count();
            r(
                if acc.iter().any(|a| !a.g("worst").eq_s("ok")) {
                    "warn"
                } else {
                    "done"
                },
                format!(
                    "{} access point{}: {} non-complying, {} restricted discretionary{}.",
                    acc.len(),
                    plural(acc.len()),
                    cnt("NC"),
                    cnt("RD"),
                    if nc > 0 {
                        format!(
                            "; {nc} not fully checked (AUP controls not loaded or not checked by hand)"
                        )
                    } else {
                        String::new()
                    }
                ),
            )
        }
        "RC-18" => {
            if x.c.g("access").jlen() == 0 {
                return r(
                    "todo",
                    "Add the proposed access points with frontage, crossings, widths and queues.",
                );
            }
            let re = rx(r"Table|E27\.6\.4\.3", "");
            r(
                if x.acc().iter().any(|a| {
                    a.g("flags")
                        .items()
                        .iter()
                        .any(|f| re.test(&f.g("rule").s()))
                }) {
                    "warn"
                } else {
                    "done"
                },
                "Crossing number, width, passing bays and queues checked from the entered values.",
            )
        }
        "RC-22" => {
            let f = x.fds();
            r(
                if f.g("status").eq_s("unknown") {
                    "todo"
                } else if f.g("outOfSequence").t() || f.g("status").eq_s("removal") {
                    "warn"
                } else {
                    "done"
                },
                f.g("text").s(),
            )
        }
        "RC-24" => {
            let o = x.ops();
            if o.g("compared").t() {
                let t: Vec<String> = o
                    .g("rows")
                    .items()
                    .iter()
                    .filter(|r| r.g("affected").t())
                    .map(|r| r.g("name").s())
                    .collect();
                r(
                    "done",
                    if t.is_empty() {
                        "No operator over the threshold.".to_string()
                    } else {
                        t.join("; ")
                    },
                )
            } else {
                r("todo", o.g("note").s())
            }
        }
        "SH-03" => {
            let n = x
                .shc()
                .g("motorway")
                .items()
                .iter()
                .filter(|m| !m.g("confirmed").t())
                .count();
            if n > 0 {
                r(
                    "warn",
                    format!(
                        "{n} zone{} connected straight to a motorway: GRPA s84 special authorisation, or move the connector (confirm in Compliance \u{203a} State highways).",
                        plural(n)
                    ),
                )
            } else {
                None
            }
        }
        "SH-01" => {
            let t = x.shc().g("triggers");
            r(
                if t.jlen() > 0 { "warn" } else { "done" },
                if t.jlen() > 0 {
                    format!(
                        "{}: seek NZTA written approval (s95E(3)(a)).",
                        t.items()
                            .iter()
                            .map(|v| v.s())
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                } else {
                    "No NZTA trigger case found.".into()
                },
            )
        }
        "SH-04" => {
            let t = x.shc().g("tech");
            if t.jlen() > 0 {
                r(
                    "done",
                    t.items()
                        .iter()
                        .map(|t| {
                            format!(
                                "{}: SISD {} m, {}",
                                t.g("name").s(),
                                t.g("sisd").g("m").s(),
                                t.g("type").g("text").s()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            } else {
                r("na", "No state highway access.")
            }
        }
        "SH-05" => {
            let i = x.ita();
            r(
                "done",
                if i.g("nzta").t() {
                    format!(
                        "{}{}",
                        i.g("nzta").g("text").s(),
                        if i.g("depth").t() {
                            format!(" {}", i.g("depth").g("text").s())
                        } else {
                            String::new()
                        }
                    )
                } else {
                    table("NZTA_T1.order").s()
                },
            )
        }
        "SH-06" => {
            let s = x.shc();
            let f = s
                .g("rows")
                .items()
                .iter()
                .filter(|r| r.g("pavement").t() && r.g("pavement").g("flagged").t())
                .count();
            r(
                if f > 0 { "warn" } else { "done" },
                format!(
                    "{}{}",
                    if f > 0 {
                        format!(
                            "{f} state highway link{} with heavy vehicles up more than 20%: pavement assessment.",
                            plural(f)
                        )
                    } else {
                        "No state highway link with heavy vehicles up more than 20%.".into()
                    },
                    if s.g("devHvPct").nullish() && s.g("rows").jlen() > 0 {
                        " Total-traffic proxy (one HV share for all traffic): enter the HV share of the development traffic."
                    } else {
                        ""
                    }
                ),
            )
        }
        "SH-07" => {
            if x.shc().g("noise").t() {
                r(
                    "warn",
                    "Noise-sensitive activity within 100 m of a state highway: acoustic assessment.",
                )
            } else {
                r(
                    "done",
                    "No noise-sensitive activity within 100 m of a state highway found.",
                )
            }
        }
        "SPD-04" => r(
            "done",
            "Local and collector roads route to the local board in the authority resolver.",
        ),
        "FUND-03" => {
            let mb = p.g("settings").g("econ").g("method").eq_s("mbcm");
            r(
                if mb { "done" } else { "warn" },
                if mb {
                    "MBCM v1.7.5 settings block printed with the economics."
                } else {
                    "This project uses the older uniform discounting; adopt the MBCM defaults."
                },
            )
        }
        "EV-01" => {
            let re = rx(r"s92|cl2[13]|lodged", "");
            let n = x
                .clocks()
                .items()
                .iter()
                .filter(|c| re.test(&c.g("kind").s()))
                .count();
            if n > 0 {
                r(
                    "done",
                    format!("{n} further-information clock{} tracked.", plural(n)),
                )
            } else {
                r(
                    "todo",
                    "Record s92 or cl 23 requests to track their deadlines.",
                )
            }
        }
        "EV-02" => {
            if x.clocks()
                .items()
                .iter()
                .any(|c| c.g("kind").eq_s("hearing"))
            {
                r("done", "Hearing evidence dates calculated.")
            } else {
                r(
                    "todo",
                    "Enter the hearing date for the s103B evidence dates.",
                )
            }
        }
        _ => None,
    }
}

/// `checklist(P)`: every checklist item with whether it applies, its status (the author's record wins over the automatic one), the finding and the author's note, by and date.
pub fn checklist(p: &V) -> V {
    let x = X::new(p);
    let c = &x.c;
    let groups = table("CK_GROUPS");
    let states = ["done", "todo", "warn", "na"];
    V::Array(
        table("CHECKLIST")
            .items()
            .iter()
            .map(|it| {
                let ok = applies(it, &x);
                let id = it.g("id").s();
                let man = if c.g("checks").has(&id) && c.g("checks").g(&id).is_obj() {
                    c.g("checks").g(&id).clone()
                } else {
                    V::object()
                };
                let st_ = if ok { auto(&id, &x) } else { None };
                let manual = man.has("state")
                    && man.g("state").t()
                    && states.contains(&man.g("state").s().as_str());
                let state_s = if !ok {
                    "na".to_string()
                } else if manual {
                    man.g("state").s()
                } else if let Some(s) = &st_ {
                    s.0.clone()
                } else {
                    "todo".to_string()
                };
                let mut o = it.clone();
                o.set(
                    "groupLabel",
                    if groups.g(&it.g("group").s()).t() {
                        groups.g(&it.g("group").s()).clone()
                    } else {
                        it.g("group").clone()
                    },
                );
                o.set("applies", V::Bool(ok));
                o.set("state", st(state_s));
                o.set("auto", V::Bool(st_.is_some() && !manual));
                o.set("text", st(st_.map_or(String::new(), |s| s.1)));
                let own = |k: &str| {
                    if man.has(k) {
                        if man.g(k).t() {
                            man.g(k).s()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                };
                o.set("note", st(own("note")));
                o.set("by", st(own("by")));
                o.set("date", st(own("date")));
                o
            })
            .collect(),
    )
}

/// `checklistCSV(P)`: the checklist as a stamped CSV.
pub fn checklist_csv(p: &V, now: &V) -> String {
    let _ = now;
    let mut rows: Vec<Vec<V>> = vec![
        [
            "ID",
            "Pathway",
            "Item",
            "Source",
            "Spare Construct fills",
            "Applies",
            "Status",
            "Spare Construct finding",
            "Author note",
            "By",
            "Date",
            "Evidence",
        ]
        .iter()
        .map(|s| st(*s))
        .collect(),
    ];
    for r in checklist(p).items() {
        rows.push(vec![
            r.g("id").clone(),
            r.g("pathway").clone(),
            r.g("item").clone(),
            r.g("source").clone(),
            r.g("fills").clone(),
            st(if r.g("applies").t() { "yes" } else { "no" }),
            r.g("state").clone(),
            r.g("text").clone(),
            r.g("note").clone(),
            r.g("by").clone(),
            r.g("date").clone(),
            r.g("evidence").clone(),
        ]);
    }
    stamp_csv(&csv(&rows), p, None)
}
