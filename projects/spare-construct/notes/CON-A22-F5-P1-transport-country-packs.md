# CON-A22-F5-P1, Transport part: New Zealand as a country pack

Part of CON-A22-F5-P1 (country packs, [FEATURES.md](../FEATURES.md)). This note covers Transport only. Status: the first step is done (see "Done in this change"); the rest waits for an owner to accept the order below.

## The question

Transport was built for New Zealand, and New Zealand runs all through it. Jeremy (owner, 28 October 2026): Transport should "branch off what's related to NZ", so that someone can build what is related to their own country. Spareware does not build the other countries.

So: what in Transport belongs to New Zealand, how does it split away from a core that works for any country, and in what order?

## What the rules already say

- [EXTENDING.md](../../../EXTENDING.md), section 2.4: a country pack is data, not code. A new country is a new pack and never a code change in a program. A pack states its sources and its date. Legal content gets a lawyer's check, and a pack needs a reviewer from that country (proposed).
- [ARCHITECTURE.md](../ARCHITECTURE.md), "Traffic and public transport": counts and crashes come "from each country pack's open sources", trip rates "from the country pack", and the compliance checklist "from the country pack".
- CON-A22-F5-P1 builds on the kit's first pack, New Zealand (KIT-A19-F9-P1). Its format must be "tested against Australia's so it is not shaped only by NZ".

## Evidence: where New Zealand is today

The Rust engine `construct-transport` (91,425 lines) is a faithful port of the browser prototype (`demo/transport/js`), proven by truth tests. A read of both found three groups. The line counts are approximate: the grouping was judged module by module from the code and its comments, not proven.

| Group | About | What is in it |
| --- | --- | --- |
| Country-neutral already | 26,000 lines | Graph, assignment, simulation, junction delay, turns, results, importers, editing, land use, the generic parts of feeds, report and compliance helpers |
| Core method carrying NZ data or standards | 31,000 lines | The standards table, economics, project defaults, calibration, the data hub, the assessment tests, crash safety, study-area loading, junction tools, search, some feeds |
| Wholly New Zealand | 34,000 lines | Compliance rules (about 13,000), the ITA report sections (about 10,400), Census (about 5,100), the NZ feeds (about 3,700), the Drury sample (about 1,900) |

The slots a Transport pack needs, with New Zealand's values today:

| Slot | New Zealand today | Where it lives now |
| --- | --- | --- |
| Units, currency, language | Metric, NZ$m, en-NZ | `econ.rs`, `project.rs` |
| Time zone, holidays, school terms | Pacific/Auckland, Mondayised holidays and Matariki, Auckland Anniversary, the Ministry of Education's terms | `feeds/nztime.rs`, `compliance/dates.rs`, `feeds/tms.rs`, `compliance/clocks.rs`, `feeds/sources.rs` (some held twice) |
| Drive side | Left | Turn-sign rules in `graph/turns.rs`, `junction/movement.rs`, `turns/mod.rs`, `calibrate/turnrows.rs` and the simulation |
| Coordinates | WGS84 in every request; no NZTM2000 or NZVD2016 yet | Request parameters |
| Geocoder and basemap | Nominatim limited to `nz`; LINZ basemaps | `search/mod.rs`, `project.rs` |
| Road classes and default trip rates | 5 classes, 14 land uses, value of time NZ$32/h | `project.rs` |
| Feeds: speed limits, counts, crashes, junction controls, road hierarchy, public transport, growth, census and zoning, fleet, hazards | NZTA NSLR, TMS and state highway counts, AT, RAMM and council counts, NZTA CAS, AT signals and signs, AT Future Connect, regional GTFS feeds, AGS23 and MSM, Stats NZ, MoE and AUP, NZTA MVR, Auckland Council flood and coastal layers | `feeds/`, `datahub/`, `studyarea/`, `census/`, `safety/` |
| Standards and criteria | TMDG 2019 (with UK TAG, RMS, ATAP and TTOC already alongside), KiwiRAP, CEC, HRIG | `standards.rs`, `calibrate/criteria.rs`, `report/consts.rs`, `safety/risk.rs` |
| Economics | MBCM v1.7.5, GC 25/01 discount rates, IPM bands | `standards.rs`, `econ.rs` |
| Design years and assessment tests | Consent 5, plan change 10, NoR 10, Fast-track 5 years; the 5% and 10% extent test; the MSM check; the s3.6.1 funding rule | `standards.rs`, `analysis/`, `edit/scenarios.rs` |
| Report | AT ITA Guidelines chapters and texts, RR 422 checklist, the ITA data pack | `report/` |
| Rules and checklists | RMA and Planning Act regimes, FTAA, AUP E27 and E38, precincts, NZTA approvals, 87-item checklist, disclaimers | `compliance/` (rules in Rust, tables in `compliance/data/*.json`) |
| Authorities | Auckland Council, local boards, AT, NZTA, KiwiRail | `report/consts.rs`, `compliance.json` |
| Hand-over exports | SIDRA loader, state highway, MBCM, RLTP, MSM, Safe System Audit and TSDG packs | `junction_tools/`, `compliance/extra/packs/`, `report/pack.rs` |
| Sample project | Drury | `sample/`, `edit/register.rs` |
| Licences | NZTA, AT, Auckland Council and LINZ, CC BY 4.0 | `feeds/sources.rs`, `report/` |

On paper, Australia fills the same slots: left-hand traffic; state road agencies' counts and crash data; Austroads and the state guides for traffic impact assessment; the Australian Transport Assessment and Planning guidelines (ATAP) for economics, which calibration already carries. Each country's laws and agencies differ, so they all sit in the pack. No slot is New Zealand's alone. This is unverified beyond the names; nobody has built a second pack.

What makes the split hard:

- **New Zealand inside core functions.** Count matching has NZTA's 70 m radius and ramp rule and RAMM's shadow rule (`datahub/`). The holiday code tests the region text for "auckland" (`feeds/tms.rs`). The extent test hand-codes AT Future Connect patterns (`analysis/mod.rs`). The drive side is a sign in code.
- **New Zealand in saved data.** Field names such as `tmdg`, `linzKey`, `fc`, `atSiteNo`, `nslr`, `tms` and `aup` are in saved projects. Renaming them needs `migrate()` and re-recorded truth files.
- **The same data held twice.** NZ time, holidays, Matariki, the feed URLs and the ITA link each appear in two or three places.
- **Rules as code.** About 23,000 lines of compliance and report rules are Rust code. Section 2.4 wants rules as data, in the kit's safe expression language (KIT-A16-F13-P1), which is not built yet.
- **Truth tests.** `tools/transport-truth` loads the prototype's files by path and records answers that 74 test files compare against. Every move must keep the values equal and the generators pointing at the new files.

## Options

1. **Packs registered through the code's own extension points**, one folder per country, with everything a pack registers active only on that country's projects. This works today in the prototype, whose features already register through `Registry`.
2. **Packs as data** in the kit's pack format, with rules in the safe expression language. This is section 2.4's target. It needs the kit's pack format (KIT-A19-F9-P1) and the expression language (KIT-A16-F13-P1), and neither exists yet.
3. **A fork per country.** Rejected: every fix would need making many times, and section 2.4 rules it out.

## Recommendation

Option 1 now, as the path to option 2. Wholly-New Zealand parts move into the NZ pack first. New Zealand values inside core methods then become pack data, one slot at a time, in the order below, so each step stays small and truth-tested. The pack's tables then move to the kit's format when it exists. Rules that are code stay in the NZ pack, clearly fenced, until the expression language can hold them. An owner should confirm that interim, because it is not yet "data, not code".

### Done in this change (the prototype)

- `demo/transport/js/registry.js` knows country packs. A pack's files load between `Registry.beginPack({id, name})` and `Registry.endPack()`, and everything they register, hooks included, is active only on projects whose `settings.country` is that pack (the first pack loaded is the default). A pack that registers the same id as a core part replaces it for its country only; other countries keep the core one.
- New Zealand's wholly-NZ feature files moved to `demo/transport/js/packs/nz/`: `feeds.js`, `landuse-plus.js`, `report.js`, `compliance.js` and `compliance-extra.js`, opened by `packs/nz/pack.js`. `tools/transport-truth` and the Rust port's comments point at the new paths; no values changed.
- Inside Spare Construct, the Transport ribbon has a tab named after the project's pack ("New Zealand"), built from whatever the pack registers. Home and Analyse hold only the core.
- [`demo/transport/js/packs/README.md`](../demo/transport/js/packs/README.md) tells someone how to add their country.

### The order for the rest

Each item is a piece for the work board. None is created yet; an owner decides.

| # | Piece | Size |
| --- | --- | --- |
| 1 | Drive side as a pack value: the turn-sign rule, leg codes, movement method and simulation read it | Medium |
| 2 | One home for NZ time, holidays and school terms, in the pack (removes the copies) | Small |
| 3 | Geocoder region, search abbreviations, basemap key and licences from the pack | Small |
| 4 | Project defaults from the pack: value of time, trip rates, road classes, TMDG category; `linzKey` becomes the pack's basemap key, with `migrate()` | Medium |
| 5 | Standards table split: neutral method values in the core, NZ entries (MBCM, IPM, TMDG, design years, extent patterns) as NZ pack data; the extent test reads its patterns from the table | Large |
| 6 | Feed slots: a core interface for each kind of feed (speed limits, counts, crashes, junction controls, road hierarchy, public transport, growth, census and zoning, fleet, hazards), NZ's sources behind it; the count source rules (NZTA, RAMM) as data | Large |
| 7 | Report engine (sections, print, figures, data pack) back to the core; AT ITA chapters and texts stay in the NZ pack | Large |
| 8 | Compliance: tables already JSON move to the pack; rule logic fenced in the NZ pack until the expression language holds it | Large |
| 9 | Sample project per pack; Drury into the NZ pack | Small |
| 10 | The Rust engine split the same way: a core crate and a New Zealand pack (module or crate), with the truth generators writing to the new places | Large |

## Open risks

- Rules as code in the interim breaks section 2.4 until the expression language exists. An owner decides whether that is acceptable.
- Saved-data field names need `migrate()` and new truth recordings; done carelessly, old projects would lose data.
- Moving the report engine back to the core (piece 7) must not change a single report byte for New Zealand. The truth tests catch that.
- The shape is tested against Australia only on paper. The first real second pack will find what this note missed.

## Sources

The code itself, read for this note: `projects/spare-construct/crates/construct-transport/src/`, `projects/spare-construct/demo/transport/js/` and `tools/transport-truth/`. The rules: [EXTENDING.md](../../../EXTENDING.md) section 2.4, [ARCHITECTURE.md](../ARCHITECTURE.md), [FEATURES.md](../FEATURES.md) (CON-A22-F5-P1) and [projects/kit/FEATURES.md](../../kit/FEATURES.md) (KIT-A19-F6-P1, KIT-A19-F9-P1, KIT-A16-F13-P1).
