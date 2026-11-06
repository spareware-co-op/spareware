#!/usr/bin/env python3
"""Slims jyviko's ssi-corpus (commit b04c8a6093a8b3505d865d1dd874ec3879ccb1f9) into one file.

Usage: python3 -I slim.py <dir holding the 119 case files> > ssi-corpus.json

Each case keeps its id, class, title, coverage tolerance, the two surfaces as given, and the
expected topology. The densely sampled curves of the expected branches and regions (about 190 KB a
case, 31 MB in all) are cut to at most 9 points a branch, evenly spaced by index with both ends, and
a coincident region keeps only how many boundary points it had. The C++ column is the one the
corpus' own issue (solidcore #17) names, see the table in the source below.
"""
import glob, json, os, sys

# SMLib GlobalSurfaceIntersect at 1e-6: the 38 cases it does not pass, from the text of
# tangency-labs/solidcore issue #17 (gaps 1 to 4 and the sub-zone-tolerance group).
CPP_FAIL = {
  # gap 1: isolated tangential contact points missed
  "c3n_parab_apex", "c3n_parab_parab_apex", "c3n_four_points", "c3_sphere_torus_outer",
  "y23_fig1g_three_points", "y23_t2_ex6_loop_and_point", "y23_t6_ex4_tangential_point",
  # gap 2: tiny loops above the zone tolerance missed
  "c7_tiny_loop_parab_1e-4", "c7_tiny_loop_parab_1e-5", "c7_tiny_loop_sphere_1e-4", "c7_tiny_loop_sphere_1e-5",
  # gap 3: curves not split at singular crossings
  "c4_steinmetz", "c4_viviani_figure8", "c4n_viviani_nurbs", "y23_fig1a_two_loops_cross",
  "y23_t6_ex3_two_crossing_curves", "y23_t3_ex4_torus_ellipsoid",
  # gap 4: tangent and high-order contact curves
  "c2n_cylN_on_planeN", "y23_t6_ex1_tangent_line", "y23_t2_ex4_tangential_curve_saddle",
  "y23_t2_ex1_tangent_line", "y23_fig1f_high_order_contact", "c2n_quartic_circle",
  "y23_t2_ex2_tangential_curve", "y23_t2_ex3_loop_two_curves", "y23_fig1d_two_quads",
  "y23_t6_ex6_four_boundary_curves", "c2n_quartic_contact_parabola", "y23_t2_ex5_two_loops",
  # below the 1e-5 zone tolerance, which the issue lists and does not count as defects
  "c7_angle_planes_1e-6", "c7_angle_planes_1e-7", "c7_angle_planes_1e-8",
  "c7_offset_planes_1e-7", "c7_offset_spheres_1e-7",
  "c7_tiny_loop_parab_1e-6", "c7_tiny_loop_parab_1e-7", "c7_tiny_loop_sphere_1e-6", "c7_tiny_loop_sphere_1e-7",
}

def thin(points, n=9):
    if len(points) <= n:
        return points
    return [points[round(i * (len(points) - 1) / (n - 1))] for i in range(n)]

def main(folder):
    out, seen = [], set()
    for path in sorted(glob.glob(os.path.join(folder, "*.json"))):
        d = json.load(open(path))
        e = d["expected"]
        out.append({
            "id": d["id"], "class": d["class"], "title": d["title"], "cov_tol": d["cov_tol"],
            "surfaces": d["surfaces"],
            "branches": [{"kind": b["kind"], "closed": b["closed"], "samples": len(b["points"]),
                          "points": thin(b["points"])} for b in e["branches"]],
            "isolated_points": e["isolated_points"],
            "singular_points": e["singular_points"],
            "coincident_regions": len(e["coincident_regions"]),
            "cpp": "fail" if d["id"] in CPP_FAIL else "pass",
        })
        seen.add(d["id"])
    missing = CPP_FAIL - seen
    assert not missing, missing
    lines = [json.dumps(c, separators=(",", ":"), allow_nan=False) for c in out]
    print("[")
    print(",\n".join(lines))
    print("]")


main(sys.argv[1])
