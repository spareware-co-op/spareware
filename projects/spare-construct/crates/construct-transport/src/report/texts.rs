//! The report's fixed texts (packs/nz/report.js): the standing disclaimers, the document's CSS and script, the data pack's
//! column dictionary and the Python reader. Generated from the prototype's source by the harness, not retyped.

/// The version of the standing disclaimers (`DISCLAIMERS.version`).
pub const DISCLAIMERS_VERSION: &str = "2026-10-08";
/// The version date of the standing disclaimers as text.
pub const DISCLAIMERS_VERSION_TEXT: &str = "8 October 2026";
/// The standing disclaimers: id, title and text.
pub const DISCLAIMER_ITEMS: [(&str, &str, &str); 13] = [
    (
        "D1",
        "Screening status",
        "Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.",
    ),
    (
        "D2",
        "Expert evidence",
        "Spare Construct is not an expert witness and does not give opinions. Any person who relies on these outputs in evidence remains responsible for complying with the Code of Conduct for Expert Witnesses (Environment Court Practice Note 2023, section 9), including stating the assumptions and data relied on, the uncertainty in the results, and appropriate sensitivity and uncertainty analyses for any model. This report is DRAFT until adopted and signed by a named, suitably qualified author.",
    ),
    (
        "D3",
        "Junction results",
        "Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.",
    ),
    (
        "D4",
        "Economics",
        "The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA's initial draft (August 2026) and may change after GPS 2027.",
    ),
    (
        "D5",
        "ANOP",
        "ANOP levels of service are indicative Spare Construct estimates. Confirm them in Auckland Transport's ANOP Assessment Tool using AT speed data before relying on them.",
    ),
    (
        "D6",
        "Calibration",
        "Calibration and validation statistics are reported against the NZTA Transport Model Development Guidelines (2019) for the stated model purpose category. Those criteria are not a pass/fail checklist, and meeting them does not by itself make a model fit for purpose. Fitness for purpose is a judgment for the analyst and peer reviewer and, where NZTA is involved, NZTA's modelling specialists.",
    ),
    (
        "D7",
        "Regional consistency",
        "The AT ITA Guidelines 2025 v2 expect detailed modelling to be consistent with regional MSM forecasts where appropriate. Where this report states that MSM consistency was not checked, the forecasts should be reconciled with MSM or Auckland Forecasting Centre outputs before lodgement for any proposal with network-wide effects.",
    ),
    (
        "D8",
        "Safety",
        "Crash histories are taken from NZTA CAS and the most recent months may be incomplete. Spare Construct's safety screening and audit briefing pack support, but do not replace, a Safe System Audit by an independent, qualified specialist, or sight distance assessment to Austroads Guide to Road Design Part 4A.",
    ),
    (
        "D9",
        "Speed limits",
        "This cost benefit disclosure statement follows the structure of clause 3.3 of the Land Transport Rule: Setting of Speed Limits 2024. It is not an MBCM benefit-cost ratio and cannot be used in place of MBCM procedures for NLTF funding.",
    ),
    (
        "D10",
        "Regulatory currency",
        "Statutory references reflect the law and guidance as at [version date] for the pathway and lodgement date shown. Under the Planning Act 2026, consents lodged before 30 October 2026 are processed under the RMA as it stood immediately before 30 September 2026, and those lodged later are processed under the amended RMA as at lodgement. The specified transition date to the new planning system has not been set. This report is not legal or planning advice. Confirm the applicable provisions at lodgement.",
    ),
    (
        "D11",
        "Authorities",
        "Since 7 May 2026 Auckland Council is the road controlling authority for Auckland roads (Local Government (Auckland Council) Act 2009 s47), with local boards exercising specified powers on local and collector roads and Auckland Transport continuing as the transport CCO for public transport. AUP provisions that still refer to Auckland Transport are quoted as written. Confirm the reviewing and approving body for this application.",
    ),
    (
        "D12",
        "Hand-over",
        "This export is prepared for use in [SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook / Safe System Audit / NZTA access application]. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.",
    ),
    (
        "D13",
        "Assumed thresholds",
        "Values marked \"Spare Construct assumption\" are Spare Construct defaults, not published rules. Agree them with the reviewing authority at scoping or replace them with project-specific values.",
    ),
];

/// The document's style sheet.
pub const CSS: &str = r#"
:root{--ink:#18201e;--muted:#56635f;--faint:#6d7b77;--line:#c9d1cd;--line-2:#e4e9e6;--panel:#ffffff;--panel-2:#f3f5f4;--accent:#0d6a4d;--amber:#b77900;--danger:#b42318;--warn:#a15c07;--ok:#1b7a46;
--los-a:#1b8a4f;--los-b:#73ad21;--los-c:#d4ab00;--los-d:#ef8a00;--los-e:#e2531f;--los-f:#b0182a;--bg:#ffffff}
@media not all{:root:not([data-theme=light]){--ink:#e5ebe8;--muted:#9caba6;--faint:#7d8b87;--line:#38433f;--line-2:#29322f;--panel:#161c1b;--panel-2:#1d2523;--accent:#3db08a;--amber:#f2b43c;--danger:#f97066;--warn:#fdb022;--ok:#4cc38a;--bg:#0e1312}}
@page{size:A4;margin:18mm 15mm 24mm 15mm;@bottom-left{content:var(--foot-l);font:7.5pt 'Segoe UI',Arial,sans-serif;color:#56635f}@bottom-right{content:"Page " counter(page) " of " counter(pages);font:7.5pt 'Segoe UI',Arial,sans-serif;color:#56635f}}
@page landscape{size:A4 landscape;margin:15mm 18mm 18mm 18mm}
*{box-sizing:border-box}html{background:var(--bg)}body{margin:0;background:var(--bg);color:var(--ink);font:10pt/1.45 'Segoe UI',Barlow,Arial,sans-serif}
main{max-width:200mm;margin:0 auto;padding:16px}
.landscape{page:landscape}@media screen{.landscape{max-width:none}}
h1.ch{font-size:20pt;margin:0 0 10px;padding-bottom:6px;border-bottom:2px solid var(--accent);color:var(--accent)}
h2{font-size:13.5pt;margin:18px 0 6px}h2 .no,h3 .no{color:var(--accent);margin-right:4px}h3{font-size:11pt;margin:14px 0 4px}h3.sec-h{font-size:12pt;margin:16px 0 5px}
p.tcap{font-weight:600;font-size:9pt;margin:8px 0 3px}.toc li.sub{margin-left:16px;font-size:9.5pt}
.chapter{break-before:page}.chapter.front{break-before:auto}h1,h2,h3,figcaption{break-after:avoid}
p{margin:6px 0}.muted{color:var(--muted)}.warn{color:var(--warn)}.bad{color:var(--danger)}.ok{color:var(--ok)}
.note{color:var(--muted);font-size:9pt}.note.warn{color:var(--warn)}
table.rt{border-collapse:collapse;width:100%;font-size:8.5pt;margin:4px 0}table.rt th{background:var(--panel-2);text-align:left;font-weight:600;border-bottom:1.5px solid var(--line);padding:3px 5px;vertical-align:bottom}
table.rt td{border-bottom:1px solid var(--line-2);padding:2.5px 5px;vertical-align:top}table.rt .n{text-align:right;font-variant-numeric:tabular-nums}
table.rt tr.tot td{background:var(--panel-2)}table.rt tr.ytd td{color:var(--muted);font-style:italic}table.rt tr.miss td{background:rgba(180,35,24,.07)}table.rt tr.chg td{background:rgba(227,154,0,.08)}
table.t{border-collapse:collapse;width:100%;font-size:8.5pt}table.t th,table.t td{border-bottom:1px solid var(--line-2);padding:2.5px 5px;text-align:left}table.t th{background:var(--panel-2)}table.t .n,table.t td.n{text-align:right}
thead{display:table-header-group}tr,figure,.disc,.author{break-inside:avoid}figure{margin:10px 0}
figure.tbl{break-inside:auto}figure.tbl figcaption,figure.fig figcaption{font-weight:600;font-size:9pt;margin:2px 0 4px}.tno{color:var(--accent)}
figure.fig img{width:100%;height:auto;border:1px solid var(--line);display:block}.fig-ph{border:1px dashed var(--line);padding:30px 14px;text-align:center;color:var(--muted);font-size:9pt;min-height:60mm;display:flex;align-items:center;justify-content:center}
.src{font-size:7.5pt;color:var(--muted);margin:2px 0}.sup{font-size:8.5pt;font-weight:600;color:var(--warn);margin:2px 0 4px}.disc{font-size:7.8pt;color:var(--muted);border-left:3px solid var(--line);padding:3px 8px;margin:6px 0}
.author{border-left:3px solid var(--accent);padding:2px 10px;margin:8px 0}.author.empty{border-left-color:var(--amber);background:rgba(227,154,0,.08);color:var(--warn);font-size:9pt;padding:6px 10px}
.tag{font-size:7pt;font-weight:500;text-transform:uppercase;letter-spacing:.05em;color:var(--muted);border:1px solid var(--line);border-radius:3px;padding:0 4px;margin-left:8px;vertical-align:middle}
.lv{display:inline-block;padding:0 4px;border-radius:3px;font-size:8pt}.lv.on{outline:1.5px solid var(--accent)}.lv-P{font-weight:700}.lv-U{color:var(--muted)}
.cover{min-height:240mm;display:flex;flex-direction:column;justify-content:center}.kicker{text-transform:uppercase;letter-spacing:.12em;color:var(--accent);font-weight:600}
.title{font-size:28pt;line-height:1.1;margin:8px 0}.subtitle{font-size:14pt;color:var(--muted);margin-bottom:18px}
table.meta{border-collapse:collapse;margin:12px 0;font-size:10pt}table.meta th{text-align:left;padding:3px 16px 3px 0;color:var(--muted);font-weight:500}table.meta td{padding:3px 0}
.stamp{font-weight:700;margin:12px 0}.stamp.draft{color:var(--danger)}.stamp.ok{color:var(--ok)}
.cover-sec{break-after:page}.toc{padding-left:18px}.toc ol{padding-left:18px;margin:2px 0 6px}.toc a{color:inherit;text-decoration:none}
ul.bul{margin:6px 0;padding-left:20px}ul.bul li{margin:3px 0}code{font-family:Consolas,'IBM Plex Mono',monospace;font-size:8pt}
.rp-watermark{position:fixed;top:38%;left:0;right:0;text-align:center;font-size:120pt;font-weight:800;letter-spacing:.1em;color:rgba(180,35,24,.09);transform:rotate(-28deg);pointer-events:none;z-index:0}
.bar{position:sticky;top:0;z-index:5;display:flex;gap:8px;align-items:center;padding:8px 16px;background:var(--panel-2);border-bottom:1px solid var(--line);font-size:9.5pt}
.bar button,.copy{font:inherit;font-size:8.5pt;padding:3px 10px;border:1px solid var(--line);border-radius:5px;background:var(--panel);color:var(--ink);cursor:pointer}.bar button.primary{background:var(--accent);color:#fff;border-color:var(--accent)}
.copy{float:right;font-size:7.5pt;padding:1px 6px}.doc-foot{margin-top:24px;padding-top:8px;border-top:1px solid var(--line);font-size:7.5pt;color:var(--muted)}.doc-foot p{margin:3px 0}.toast{position:fixed;bottom:16px;left:50%;transform:translateX(-50%);background:var(--ink);color:var(--bg);padding:8px 14px;border-radius:6px;font-size:9pt}
.mini-chart{width:100%;font-size:9px}.mini-chart text{fill:var(--muted)}figure.chart figcaption{font-size:8.5pt;color:var(--muted)}.hud-row{display:flex;gap:12px}
@media print{.noprint,.bar,.copy,.toast{display:none!important}main{max-width:none;padding:0}body{font-size:9.5pt}a{color:inherit;text-decoration:none}}"#;

/// The script that runs inside the report window (print, save, copy a table).
pub const DOC_SCRIPT: &str = r#"(function(){
  function toast(t){var d=document.createElement('div');d.className='toast';d.textContent=t;document.body.appendChild(d);setTimeout(function(){d.remove()},3200)}
  function rows(t){return Array.prototype.map.call(t.rows,function(r){return Array.prototype.map.call(r.cells,function(c){return (c.innerText||c.textContent||'').replace(/[\t\r\n]+/g,' ').trim()}).join('\t')}).join('\r\n')}
  document.addEventListener('click',function(e){
    var c=e.target.closest('[data-copy]');
    if(c){var t=c.closest('figure').querySelector('table');if(!t)return;var s=rows(t),n=t.tBodies[0]?t.tBodies[0].rows.length:t.rows.length,done=function(){toast('Copied '+n+' rows. Paste into Excel or Word.')};
      try{if(window.ClipboardItem&&navigator.clipboard&&navigator.clipboard.write){navigator.clipboard.write([new ClipboardItem({'text/html':new Blob([t.outerHTML],{type:'text/html'}),'text/plain':new Blob([s],{type:'text/plain'})})]).then(done,function(){navigator.clipboard.writeText(s).then(done)})}else{navigator.clipboard.writeText(s).then(done)}}catch(err){toast('Your browser did not allow copying. Select the table and copy it instead.')}return}
    if(e.target.closest('[data-print]')){Promise.all(Array.prototype.map.call(document.images,function(i){return i.decode?i.decode().catch(function(){}):null})).then(function(){requestAnimationFrame(function(){window.print()})});return}
    if(e.target.closest('[data-save]')){var h='<!doctype html>\n'+document.documentElement.outerHTML,a=document.createElement('a');a.href=URL.createObjectURL(new Blob([h],{type:'text/html'}));a.download=document.body.getAttribute('data-file')||'ita-report.html';document.body.appendChild(a);a.click();setTimeout(function(){URL.revokeObjectURL(a.href);a.remove()},1000)}
  });
})();"#;

/// The content security policy of the report document.
pub const CSP: &str = "default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'; script-src 'sha256-PlIsln8wxWqGEn3AucRO7R1kNQXRifIwv7ua/46Olvw='; base-uri 'none'; form-action 'none'";

/// The Python reader that goes in the data pack.
pub const PY: &str = r#"# Load Spare Construct ITA data pack CSVs in Python (standard library only).
# Every CSV is UTF-8 with a BOM, and its first row is Disclaimer D1: skip it.
# pandas: pd.read_csv(path, encoding='utf-8-sig', skiprows=1)
import csv, sys

def load(path):
    with open(path, encoding='utf-8-sig', newline='') as f:
        rows = list(csv.reader(f))
    if rows and rows[0] and rows[0][0].startswith('#'):
        rows = rows[1:]
    head, body = rows[0], rows[1:]
    return [dict(zip(head, r)) for r in body]

if __name__ == '__main__':
    path = sys.argv[1] if len(sys.argv) > 1 else 'links.csv'
    data = load(path)
    print(f'{len(data)} rows from {path}')
    for row in data[:10]:
        print(row)
"#;

/// The data pack's files and what each holds (`FILES`), in the prototype's order.
pub const FILES: [(&str, &str); 18] = [
    (
        "README.txt",
        "Column dictionary with units, scenario list, model version and data vintage",
    ),
    (
        "scenarios.csv",
        "One row per scenario and period with its year, role and headline results",
    ),
    (
        "links.csv",
        "Directional link results per scenario and period",
    ),
    (
        "junction_summary.csv",
        "Junction results per scenario and period",
    ),
    (
        "junction_movements.csv",
        "Junction approach results per scenario and period",
    ),
    (
        "turning_movements.csv",
        "Turning movements per junction, scenario and period",
    ),
    (
        "trip_generation.csv",
        "Vehicle trip generation per zone, land use and period",
    ),
    (
        "distribution.csv",
        "Gateway distribution of development traffic",
    ),
    (
        "counts_calibration.csv",
        "Every count used, with the modelled value and GEH",
    ),
    (
        "calibration_summary.csv",
        "Calibration checks against TMDG 2019 per period",
    ),
    ("route_times.csv", "Bus route times per scenario and period"),
    (
        "corridor-sidra-volumes.csv",
        "SIDRA hand-over: movement volumes per junction and period (no disclaimer row, so the loader reads it as is; D1 and D12 are in this README and in the loader script)",
    ),
    (
        "load_corridor_volumes.py",
        "SIDRA INTERSECTION 11 loader script for corridor-sidra-volumes.csv (carries D1 and D12)",
    ),
    (
        "read_datapack.py",
        "Reads any data pack CSV in Python, skipping the D1 row",
    ),
    ("crashes.csv", "Every reported crash loaded from CAS"),
    ("crash_sites.csv", "Crash history per assessed site (HRIG)"),
    (
        "provenance.csv",
        "Data sources, query dates, records and licences",
    ),
    (
        "assumptions.csv",
        "Standards and Spare Construct assumptions",
    ),
];

/// The data pack's column dictionary (`COLS`): each CSV with its columns and their units, in the prototype's order.
pub const COLS: [(&str, &[(&str, &str)]); 14] = [
    (
        "scenarios.csv",
        &[
            ("scenario_id", "Spare Construct scenario id"),
            ("scenario", "name"),
            ("code", "S0 to S3 where set"),
            (
                "role",
                "existing, baseline, proposal, mitigation, sensitivity, permitted",
            ),
            ("year", "modelled year"),
            ("period", "AM or PM peak hour"),
            ("trips_veh_h", "veh/h"),
            ("vkt", "veh·km/h"),
            ("vht", "veh·h/h"),
            ("delay_veh_h", "veh·h/h"),
            ("avg_speed_kmh", "km/h"),
            ("co2_t_h", "t/h"),
            ("iterations", "count"),
            ("gap", "relative gap (fraction)"),
            ("converged", "true or false"),
            ("run_at", "ISO date-time"),
            ("stale", "true if the inputs changed after the run"),
        ],
    ),
    (
        "links.csv",
        &[
            ("scenario_id", ""),
            ("period", "AM or PM"),
            ("link_id", ""),
            ("dir", "AB or BA"),
            ("road_name", ""),
            ("from_node", "node id"),
            ("to_node", "node id"),
            ("class", "motorway, ramp, arterial, collector, local"),
            ("fc_gt", "Future Connect general traffic"),
            ("fc_pt", "Future Connect public transport"),
            ("fc_freight", "Future Connect freight"),
            ("fc_cycle", "Future Connect cycle"),
            ("aup_arterial", "true, false or blank"),
            ("lanes", "lanes in this direction"),
            ("speed_limit_kmh", "km/h"),
            ("speed_source", "NSLR or coded"),
            ("length_m", "m"),
            ("flow_veh_h", "veh/h"),
            ("flow_pcu_h", "pcu/h"),
            ("hv_pct", "% (model assumption)"),
            ("capacity_pcu_h", "pcu/h including junction effects"),
            ("mid_block_capacity_pcu_h", "pcu/h"),
            ("cap_source", "class default or user"),
            ("vc", "ratio"),
            ("speed_kmh", "km/h"),
            ("speed_pct_limit", "%"),
            ("anop_gt_los", "blank (ANOP tool)"),
            ("junction_delay_s", "s"),
            ("without_flow_veh_h", "veh/h in the without scenario"),
            ("delta_veh_h", "veh/h"),
            ("delta_pct", "%"),
            ("threshold_pct", "extent threshold %"),
            ("in_study_area", "true or false"),
        ],
    ),
    (
        "junction_summary.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("site_id", "node or junction-site id"),
            ("junction", "name"),
            ("control", "priority, stop, roundabout, signals"),
            ("cycle_s", "s"),
            ("vol_veh_h", "veh/h"),
            ("worst_dos", "ratio"),
            (
                "delay_s",
                "s (average for signals and roundabouts, worst movement otherwise)",
            ),
            ("los", "A to F"),
            ("los_basis", ""),
            ("max_q95_m", "m"),
            ("max_q95_leg", "approach"),
        ],
    ),
    (
        "junction_movements.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("site_id", ""),
            ("junction", ""),
            ("approach", "compass side traffic arrives from"),
            ("road", ""),
            ("link_id", ""),
            ("volume_veh_h", "veh/h"),
            ("capacity_veh_h", "veh/h"),
            ("dos", "ratio"),
            ("delay_s", "s"),
            ("los", "A to F"),
            ("q95_veh", "veh"),
            ("q95_m", "m"),
            ("psc_pct", "% practical spare capacity"),
            ("storage_m", "approach link length m"),
        ],
    ),
    (
        "turning_movements.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("node_id", ""),
            ("junction", ""),
            ("from_arc", "arc key"),
            ("to_arc", "arc key"),
            ("movement", "L, T, R or U"),
            ("leg", "leg name"),
            ("turn", "SIDRA turn designation"),
            ("movement_id", "e.g. S-R2"),
            ("volume_veh_h", "veh/h"),
            ("pcu_h", "pcu/h"),
            ("development_veh_h", "veh/h from development zones"),
        ],
    ),
    (
        "trip_generation.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("zone_id", ""),
            ("zone", ""),
            ("class", "existing, committed, development, external"),
            ("land_use", ""),
            ("quantity", "in the unit"),
            ("unit", ""),
            ("rate_veh_per_unit_h", "veh trips per unit per hour"),
            ("in_pct", "%"),
            ("gross_veh_h", "veh/h"),
            ("reduction_pct", "%"),
            ("buildout_pct", "%"),
            ("net_in_veh_h", "veh/h"),
            ("net_out_veh_h", "veh/h"),
        ],
    ),
    (
        "distribution.csv",
        &[
            ("scenario_id", ""),
            ("gateway_id", ""),
            ("gateway", ""),
            ("am_in_pct", "%"),
            ("am_out_pct", "%"),
            ("pm_in_pct", "%"),
            ("pm_out_pct", "%"),
            ("am_veh_h", "veh/h"),
            ("pm_veh_h", "veh/h"),
        ],
    ),
    (
        "counts_calibration.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("count_id", ""),
            ("source", "NZTA, AT or yours"),
            ("ref", "site reference"),
            ("name", ""),
            ("road", ""),
            ("count_date", "ISO date"),
            ("duration", ""),
            ("adt_vpd", "veh/day"),
            ("observed_veh_h", "veh/h"),
            ("modelled_veh_h", "veh/h"),
            ("geh", ""),
            ("peak_basis", "how the peak hour was derived"),
            ("link_id", "matched link"),
            ("dir", "matched direction"),
        ],
    ),
    (
        "calibration_summary.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("check", "id"),
            ("criterion", ""),
            ("target", ""),
            ("value", "number"),
            ("status", ""),
            ("source", ""),
            ("verified", "true or false"),
        ],
    ),
    (
        "route_times.csv",
        &[
            ("scenario_id", ""),
            ("period", ""),
            ("route_id", ""),
            ("route", ""),
            ("length_m", "m"),
            ("time_s", "s"),
            ("speed_kmh", "km/h"),
        ],
    ),
    (
        "crashes.csv",
        &[
            (
                "cas_objectid",
                "CAS OBJECTID; blank when Spare Construct did not keep it",
            ),
            ("year", ""),
            ("severity", "F, S, M or N"),
            ("fatal_count", "people killed; blank when not loaded"),
            (
                "serious_count",
                "people seriously injured; blank when not loaded",
            ),
            (
                "minor_count",
                "people with minor injuries; blank when not loaded",
            ),
            ("speed_limit", "km/h"),
            (
                "traffic_control",
                "CAS trafficControl; blank when not loaded",
            ),
            ("urban", "CAS urban; blank when not loaded"),
            ("ped", "true or false"),
            ("cyclist", "true or false"),
            ("motorcyclist", "true or false"),
            ("light", "CAS light; blank when not loaded"),
            ("weather", "CAS weatherA; blank when not loaded"),
            ("location1", ""),
            ("location2", ""),
            ("site_id", "nearest assessed site within 50 m"),
            ("dist_m", "m"),
            ("lat", "WGS84"),
            ("lng", "WGS84"),
        ],
    ),
    (
        "crash_sites.csv",
        &[
            ("site_id", ""),
            ("type", "junction name"),
            ("control", ""),
            ("speed_limit", "km/h"),
            ("n_f", "crashes"),
            ("n_s", "crashes"),
            ("n_m", "crashes"),
            ("n_n", "crashes"),
            ("injury", "crashes"),
            ("dsi_equiv_5yr", "DSI equivalents"),
            ("collective_band", "HRIG"),
            ("aadt_legs", "vpd, busiest first"),
            ("pof", "product of flows"),
            ("personal_risk", "DSI per 100 million vehicles"),
            ("personal_band", "HRIG"),
            ("reliable", "true when 4 or more injury crashes"),
        ],
    ),
    (
        "provenance.csv",
        &[
            ("source", ""),
            ("dataset", ""),
            ("query_date", "ISO date"),
            ("records", ""),
            ("licence", ""),
        ],
    ),
    (
        "assumptions.csv",
        &[
            ("group", ""),
            ("item", ""),
            ("source_edition", ""),
            ("basis", "Spare Construct assumption or published source"),
            ("verified", "true or false"),
        ],
    ),
];

/// The Engine standards the assumptions register lists: name, edition, checked against source and whether it is a Spare Construct assumption, in the prototype's order.
pub const ENGINE_ASSUMPTIONS: [(&str, &str, bool, bool); 23] = [
    (
        "HCM 6 control delay",
        "Highway Capacity Manual, 6th edition (2016)",
        true,
        false,
    ),
    (
        "SIDRA Delay (roundabouts as signals)",
        "SIDRA Intersection LoS method \"Delay (SIDRA)\"",
        false,
        false,
    ),
    (
        "RTA NSW delay bands (option only)",
        "RTA NSW Guide to Traffic Generating Developments (2002), Table 4.2",
        false,
        false,
    ),
    (
        "Link volume/capacity bands",
        "Spare Construct assumption",
        false,
        true,
    ),
    (
        "Practical degree of saturation",
        "SIDRA Intersection defaults",
        false,
        false,
    ),
    (
        "Peak flow factor",
        "TTOC-06 Traffic Signals Modelling Guidelines; SIDRA default convention",
        false,
        false,
    ),
    (
        "Back of queue",
        "HCM 2000 Appendix G (signals, Akçelik form); HCM 6 eq. 20-68 (unsignalised)",
        false,
        false,
    ),
    (
        "Assignment convergence",
        "UK TAG M3.1 (May 2026), Table 4",
        true,
        false,
    ),
    (
        "Scenario signal to noise",
        "UK TAG M3.1 (May 2026)",
        false,
        false,
    ),
    (
        "Gap needed for turn volume exports",
        "Spare Construct assumption",
        false,
        true,
    ),
    (
        "NZTA Monetised Benefits and Costs Manual",
        "v1.7.5",
        false,
        false,
    ),
    ("MBCM update factors", "July 2025", false, false),
    (
        "Investment Prioritisation Method 2024–27 efficiency rating",
        "2024–27 NLTP (final)",
        false,
        false,
    ),
    (
        "Investment Prioritisation Method 2027–30 efficiency rating",
        "Initial draft, August 2026",
        false,
        false,
    ),
    (
        "Background traffic growth",
        "MBCM v1.7.5 arithmetic growth; NZ ITA practice",
        false,
        false,
    ),
    (
        "Passenger car units per bus",
        "docs/spec/gtfs-bus (Spare Construct research spec)",
        false,
        false,
    ),
    (
        "Critical gaps and follow-up headways",
        "HCM 6 ch. 20 (TWSC) base values, two-lane major road; mirrored for left-hand traffic",
        false,
        false,
    ),
    (
        "Roundabout lane capacity",
        "HCM 6 ch. 22 lane-based entry capacity equations; NZ markings left lane L+T, right lane T+R",
        false,
        false,
    ),
    (
        "Extent of effects (5% / 10% test)",
        "AT ITA Guidelines 2025 v2, s4.2.1.1",
        false,
        true,
    ),
    (
        "Difference plot thresholds",
        "Spare Construct assumption",
        false,
        true,
    ),
    (
        "NZTA Transport Model Development Guidelines",
        "2019",
        false,
        false,
    ),
    (
        "MSM consistency threshold",
        "Spare Construct assumption",
        false,
        true,
    ),
    (
        "Design year",
        "NZTA RR 422 s5.5; AT ITA Guidelines 2025 v2 s3.6",
        false,
        false,
    ),
];
