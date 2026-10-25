//! The fixed text of the SIDRA exports (junctions.js): the loader script and the printable volume grids, between the values they fill in.

/// The SIDRA 11 loader script's text between its 15 filled-in values: the D1 and D12 comment blocks, the not-converged comment, the date, the scenario, the year, the periods, the counts, the setup, the scenario, the convergence text, the notice and the two JSON blocks.
pub(super) const LOADER: [&str; 16] = [
    r#####"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Spare Construct to SIDRA INTERSECTION 11 volume loader (New Zealand setup)
#
# D1 Screening status
"#####,
    r#####"
# D12 Hand-over
"#####,
    r#####"
#
# Requires licensed SIDRA 11, 64-bit Python, pythonnet, .NET 8; close the project in SIDRA first; untested against a live install.
#"#####,
    r#####"
# Made by Spare Construct on "#####,
    r#####" for scenario "#####,
    r#####" ("#####,
    r#####"); periods "#####,
    r#####"; "#####,
    r#####" movements at "#####,
    r#####" sites.
# Volumes are modelled demand in vehicles per hour (light and heavy vehicles), not PCU and not throughput, from a
# Spare Construct sketch model. Spare Construct junction results are HCM 6 / Akcelik-based, indicative and not SIDRA output.
#
# Usage
#   py -3 load_corridor_volumes.py --dry-run                print the planned SIDRA API calls; SIDRA is not needed
#   py -3 load_corridor_volumes.py PROJECT.sipx             add or update sites in an existing SIDRA project
#   py -3 load_corridor_volumes.py PROJECT.sipx --create    create the project first
#   --csv corridor-sidra-volumes.csv                        read the volumes from the CSV instead of the copy below
#
# Update mode: a site in the same folder whose Intersection ID is the Spare Construct site id keeps its lane and phasing
# coding. Its volumes, peak flow factors, unit time for volumes and peak flow period are overwritten; every movement
# between its legs is set to 0 first, so a movement with no traffic now (a banned turn, a changed network) does not
# keep an old volume. New sites get one leg per Spare Construct leg with default NZ geometry, which you must check and code.
# A peak flow factor from a turning count's 15-minute peak hour factor comes with a 15-minute peak flow period.
# Leg and turn codes follow SIDRA drive-on-left conventions: legs S 0, SE 1, E 2, NE 3, N 4, NW 5, W 6, SW 7
# (named by where traffic comes from); turns U, L3, L2, L1, T1, R1, R2, R3.
# Volumes already include growth to the scenario year, so growth_pct is 0.

import argparse
import csv
import json
import os
import sys

SITE_TYPES = {0: 'signals', 1: 'roundabout', 2: 'give-way', 3: 'two-way stop'}
NZ_SETUP = "#####,
    r#####"  # SIDRA site standard: 2 = New Zealand
SCENARIO = "#####,
    r#####"
NOT_CONVERGED = "#####,
    r#####"
NOTICE = "#####,
    r#####"
DATA =json.loads(r'''"#####,
    r#####"''')
LEG_LANES = json.loads(r'''"#####,
    r#####"''')
NUMERIC = ('lat', 'lng', 'site_type', 'origin_leg', 'dest_leg', 'lv_veh_h', 'hv_veh_h', 'pff_pct', 'unit_time_min', 'peak_flow_period_min', 'growth_pct')
RETURNS = {'OpenProject': 'project', 'CreateAndOpenProject': 'project', 'AddSiteFolder': 'folder', 'AddSite': 'site'}


class DryRun(object):
    """Stands in for the SIDRA API: prints each call and assignment instead of making it."""

    def __init__(self, name):
        object.__setattr__(self, '_name', name)

    def __getattr__(self, attr):
        return DryRun(self._name + '.' + attr)

    def __setattr__(self, attr, value):
        print('%s.%s = %r' % (self._name, attr, value))

    def __call__(self, *args):
        print('%s(%s)' % (self._name, ', '.join(repr(a) for a in args)))
        return DryRun(RETURNS.get(self._name.split('.')[-1], 'result'))

    def __getitem__(self, key):
        k = ', '.join(repr(x) for x in key) if isinstance(key, tuple) else repr(key)
        return DryRun('%s[%s]' % (self._name, k))


def read_csv(path):
    with open(path, newline='', encoding='utf-8-sig') as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        for k in NUMERIC:
            if r.get(k, '') != '':
                r[k] = float(r[k]) if k in ('lat', 'lng') else int(float(r[k]))
    return rows


def group(rows):
    """One SIDRA site per Spare Construct site and period, with its legs and movements."""
    sites = {}
    for r in rows:
        key = (r['period'], str(r['site_id']))
        s = sites.get(key)
        if s is None:
            s = sites[key] = {'row': r, 'legs': set(), 'moves': []}
        s['legs'].add(int(r['origin_leg']))
        s['legs'].add(int(r['dest_leg']))
        s['moves'].append(r)
    return sites


def load_api():
    if sys.maxsize <= 2 ** 32:
        sys.exit('Use 64-bit Python: SIDRA 11 is a 64-bit application.')
    try:
        import winreg
        key = winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\SIDRA SOLUTIONS\SIDRA INTERSECTION 11')
        loc = winreg.QueryValueEx(key, 'InstallLocation')[0]
    except OSError:
        sys.exit('SIDRA INTERSECTION 11 was not found in the registry. Install SIDRA 11, or run with --dry-run.')
    try:
        from pythonnet import set_runtime
        from clr_loader import get_coreclr
        set_runtime(get_coreclr(runtime_config=os.path.join(loc, 'SIDRASolutions.SI.API.runtimeconfig.json')))
        import clr
    except ImportError:
        sys.exit('pythonnet is missing. Install it with: py -3 -m pip install pythonnet')
    sys.path.append(loc)
    clr.AddReference(os.path.join(loc, 'SIDRASolutions.SI.API.dll'))
    from SIDRASolutions.SI.API import SIAPI
    return SIAPI()


def existing_sites(project):
    """Sites already in the project, keyed by (folder name, Intersection ID)."""
    found, folders = {}, {}
    if isinstance(project, DryRun):
        return found, folders
    for folder in getattr(project, 'SiteFolders', None) or []:
        folders[str(folder.Name)] = folder
        for site in getattr(folder, 'Sites', None) or []:
            found[(str(folder.Name), str(site.Intersectionid))] = site
    return found, folders


def set_volumes(site, moves):
    for m in moves:
        od = site.MovementVehicleODs[int(m['origin_leg']), int(m['dest_leg'])]
        od.MovementVehicleODMCs[1].Volume = int(m['lv_veh_h'])
        od.MovementVehicleODMCs[2].Volume = int(m['hv_veh_h'])
        od.Peak_flow_factor = int(m['pff_pct'])


def site_legs(site_id, legs=()):
    return sorted(set(int(k) for k in LEG_LANES.get(site_id, {})) | set(int(x) for x in legs))


def clear_volumes(site, legs):
    """Update mode: set every movement between the site's legs to 0 before the new volumes are written."""
    for o in legs:
        for d in legs:
            try:
                od = site.MovementVehicleODs[o, d]
            except Exception:  # no such movement on this site
                continue
            if od is None:
                continue
            for mc in (1, 2):
                try:
                    od.MovementVehicleODMCs[mc].Volume = 0
                except Exception:
                    pass


def main(argv=None, api=None):
    ap = argparse.ArgumentParser(description='Load Spare Construct turning volumes into a SIDRA INTERSECTION 11 project (New Zealand setup).')
    ap.add_argument('project', nargs='?', help='the .sipx project file')
    ap.add_argument('--create', action='store_true', help='create the project first')
    ap.add_argument('--csv', help='read volumes from this CSV (corridor-sidra-volumes.csv) instead of the embedded copy')
    ap.add_argument('--dry-run', action='store_true', help='print the planned SIDRA API calls without SIDRA')
    a = ap.parse_args(argv)
    print(NOTICE)
    rows = read_csv(a.csv) if a.csv else DATA
    if not rows:
        print('No volumes to load. Export again from Spare Construct after running the model.')
        return 1
    if not a.project and not a.dry_run:
        ap.error('give the .sipx project file, or --dry-run')
    if NOT_CONVERGED:
        print('WARNING: ' + NOT_CONVERGED + '. These volumes are not fit to rely on.')
    path = a.project or 'corridor.sipx'
    if api is None:
        api = DryRun('api') if a.dry_run else load_api()
    project = api.CreateAndOpenProject(path, 'Spare Construct ' + SCENARIO) if a.create else api.OpenProject(path)
    if project is None:
        project = getattr(api, 'Project', None)
    if project is None:
        sys.exit('SIDRA could not open %s. Close it in SIDRA and check the path.' % path)
    found, folders = existing_sites(project)
    made = updated = cleared = 0
    groups = group(rows)
    for (period, site_id), s in sorted(groups.items()):
        r = s['row']
        fname = 'Spare Construct %s %s' % (SCENARIO, period)
        folder = folders.get(fname)
        if folder is None:
            folder = folders[fname] = project.AddSiteFolder(fname)
        site = found.get((fname, site_id))
        if site is None:
            site = folder.AddSite(int(r['site_type']), NZ_SETUP)
            site.Name = '%s (%s)' % (r['site_name'], period)
            site.Intersectionid = site_id
            try:
                site.MapCoordinate.Latitude = float(r['lat'])
                site.MapCoordinate.Longitude = float(r['lng'])
            except Exception as e:  # MapCoordinate may be a value type in the API
                print('Could not set the map position of %s: %s' % (site_id, e))
            lanes = LEG_LANES.get(site_id, {})
            for leg in sorted(s['legs']):
                site.AddLegWithLanes(leg, int(lanes.get(str(leg), 1)))
            site.UpdateGeometryData()
            made += 1
        else:
            clear_volumes(site, site_legs(site_id, s['legs']))
            updated += 1
        site.Unittimeforvolumes = int(r['unit_time_min'])
        site.Peakflowperiod = int(r['peak_flow_period_min'])
        set_volumes(site, s['moves'])
    # a site in this export with no traffic at all in a period: clear its old volumes too
    prefix, periods = 'Spare Construct %s ' % SCENARIO, set(str(r['period']) for r in rows)
    for (fname, site_id), site in sorted(found.items(), key=lambda kv: kv[0]):
        period = fname[len(prefix):] if fname.startswith(prefix) else None
        if period in periods and site_id in LEG_LANES and (period, site_id) not in groups:
            clear_volumes(site, site_legs(site_id))
            cleared += 1
    api.SaveProject()
    print('Done: %d sites created, %d updated and %d set to no traffic in %s. Lanes and phasing of existing sites were kept.' % (made, updated, cleared, path))
    return 0


if __name__ == '__main__':
    sys.exit(main())
"#####,
];

/// The volume grids page's text between its filled-in values.
pub(super) const GRIDS: [&str; 11] = [
    r#####"<!doctype html><html lang="en-NZ"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Turning volumes for SIDRA · "#####,
    r#####"</title>
<style>body{font:13px/1.45 system-ui,sans-serif;color:#18201e;background:#fff;margin:24px}h1{font-size:20px;margin:0 0 4px}h2{font-size:15px;margin:22px 0 2px}.meta,.note{color:#56635f;margin:2px 0 8px}
table{border-collapse:collapse;margin:4px 0 10px}th,td{border:1px solid #c9d1cd;padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums}th{background:#f1f4f2;font-weight:600}tr th:first-child{text-align:left}
.warn{color:#a15c07;font-weight:600}section{break-inside:avoid;page-break-inside:avoid}@media print{body{margin:10mm}}
.disc{border:1px solid #c9d1cd;border-left:4px solid #56635f;padding:6px 10px;margin:8px 0 12px;color:#18201e}.disc p{margin:2px 0}
table.pg{border-collapse:collapse;width:100%;margin:0}table.pg>tbody>tr>td,table.pg>tfoot>tr>td{border:0;padding:0;text-align:left;font-variant-numeric:normal}
.pf{border-top:1px solid #c9d1cd;margin-top:14px;padding-top:6px;color:#56635f;font-size:11px}.pf p{margin:2px 0}@media print{.pf{font-size:8pt}}</style></head><body>
<table class="pg" role="presentation"><tfoot><tr><td><footer class="pf"><p>"#####,
    r#####".</p><p><b>D1 Screening status.</b> "#####,
    r#####"</p><p><b>D12 Hand-over.</b> "#####,
    r#####"</p></footer></td></tr></tfoot><tbody><tr><td>
<h1>Turning volumes for SIDRA · "#####,
    r#####"</h1>
<p class="meta">Spare Construct, "#####,
    r#####". Modelled demand in vehicles per hour (LV light, HV heavy vehicles), whole vehicles. Legs are named by where traffic comes from; turns use SIDRA drive-on-left designations.</p>
<div class="disc" role="note"><p><b>D1 Screening status.</b> "#####,
    r#####"</p><p><b>D12 Hand-over.</b> "#####,
    r#####"</p></div>
"#####,
    r#####"
"#####,
    r#####"
</td></tr></tbody></table></body></html>"#####,
];
