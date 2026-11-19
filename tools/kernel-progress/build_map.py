"""Build the site's kernel map page: how the Spare Kit geometry kernel fits together, as one tree.

Usage: python tools/kernel-progress/build_map.py <output.html> [--offline]
Run from the repository root, as build.py is (the Site workflow runs both). Standard library only.

The tree, top to bottom: the programs that use the kernel; the interface they call; our crates, by
layer; the two engines underneath (our exact core, and solidcore behind our wrapper) with every
operation under the engine that answers it today and the class of its answer; and the file formats.

Nothing on the page is typed by hand. It reads:

- `MAP.json`: every crate, its layer and its job. Each crate appears once: layer 13 is the
  interface, layer 12 the formats, the development crates are the testing tools, and every other
  layer is drawn in the crates' tier, highest first.
- chapter 27's layer table: the layers' names.
- engine-geometry's `generated/mcp-tools.json`: the operations programs call today, by family,
  with the exactness classes each states.
- `operations.json` and the progress page's states (build.py): every operation, and whether our core
  answers it (ours, on main), solidcore does (done by solidcore), or nothing yet (held back, or not
  working yet).
- chapter 9: the exactness classes, in the kernel's own words.
- geom-solidcore's `Cargo.toml` and tests: the pin, the stored cases, the named faults and the
  tolerances its answers are held to.
- `map.json`, beside this file: the plain words for each crate and layer, and the few links the
  kernel's files do not hold (it says which). A crate, operation, class, family or tolerance that
  the repository has and `map.json` does not cover fails the build, and so does a line in
  `map.json` for something the repository no longer has.
"""
import html
import json
import os
import re
import sys
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build  # noqa: E402

KERNEL = build.KERNEL
LAYERS_MD = KERNEL + "/architecture/27-crates-folders.md"
CLASSES_MD = KERNEL + "/architecture/09-representations.md"
TOOLS = "projects/engines/crates/engine-geometry/generated/mcp-tools.json"
WRAPPER_TOML = KERNEL + "/geom-solidcore/Cargo.toml"
MAP_DATA = os.path.join(build.HERE, "map.json")

# Where map.json's `placement` puts a layer: the interface tier, the formats tier, or (every other
# layer) the crates' tier. The development crates' layer is the one MAP.json gives no number.
DEV_LAYER = "Development"
CORE, SOLIDCORE, NONE = "core", "solidcore", "none"


class MapError(Exception):
    """The repository and map.json disagree; every problem found is listed."""


# ---- reading the repository ----------------------------------------------------------------------

def read_classes(chapter9):
    """Chapter 9's exactness classes in order: [(name, definition)], from its list under
    "Every body carries an exactness class"."""
    block = chapter9.split("**Every body carries an exactness class:**", 1)
    if len(block) < 2:
        raise MapError(["chapter 9 has no list of exactness classes"])
    out = []
    for line in block[1].lstrip("\n").splitlines():
        m = re.match(r"- \*\*([A-Z][a-z]+)(?:\([^)]*\))?:\*\* (.+)", line)
        if not m:
            break
        out.append((m.group(1), m.group(2).strip()))
    if not out:
        raise MapError(["chapter 9's list of exactness classes is empty"])
    return out


def read_layer_names(chapter27):
    """Chapter 27's layer table: layer number (as text) -> its name."""
    out = {}
    table = chapter27.split("| Layer, lowest first |", 1)
    if len(table) < 2:
        raise MapError(["chapter 27 has no layer table"])
    for line in table[1].splitlines()[2:]:
        m = re.match(r"\| (\d+)\. ([^|]+?) \|", line)
        if not m:
            break
        out[m.group(1)] = m.group(2).strip()
    return out


def read_tools(tools_json):
    """The interface's operations: [{name, family, classes}]."""
    out = []
    for t in tools_json.get("tools", []):
        meta = t.get("_meta", {}).get("org.spareware/kernel", {})
        out.append({"name": t["name"], "family": meta.get("family", ""), "classes": meta.get("exactness", [])})
    return out


def read_solidcore(cargo_toml, cases_json, faults_json):
    """The wrapper's pin and what its tests hold it to."""
    t = tomllib.loads(cargo_toml)
    dep = t.get("dependencies", {}).get("solidcore", {})
    return {"pin": dep.get("rev", ""), "fork": dep.get("git", ""), "licence": t.get("package", {}).get("license", ""),
            "wrapper": t.get("package", {}).get("name", ""), "cases": len(cases_json.get("cases", [])),
            "faults": len({build.fault_label(f)[0] for f in faults_json.get("faults", [])}),
            "fault_cases": len(faults_json.get("faults", [])), "tolerances": cases_json.get("tolerances", {})}


# ---- the tree ------------------------------------------------------------------------------------

def layer_key(layer):
    return (1, 0) if not layer.isdigit() else (0, -int(layer))


def kernel_map(map_json, layer_names, tools, ops, data, classes, solidcore, files):
    """The whole tree as plain data, or MapError naming every gap between the repository and map.json.

    `ops` is build.operation_states' result; `files` maps each app's source path to its text."""
    problems = []
    class_names = [c for c, _ in classes]
    crates = map_json.get("crates", [])
    plain = data.get("crates", {})
    names = [c["name"] for c in crates]
    for n in names:
        if n not in plain:
            problems.append(f"crate {n} is in MAP.json but has no plain words in map.json")
    for n in plain:
        if n not in names:
            problems.append(f"map.json has words for crate {n}, which MAP.json does not list")
    layers = {}
    for c in crates:
        layer = c.get("layer", "")
        if layer != DEV_LAYER and layer not in layer_names:
            problems.append(f"crate {c['name']} is in layer {layer!r}, which chapter 27's layer table does not name")
        if layer not in data.get("layers", {}):
            problems.append(f"layer {layer!r} has no plain words in map.json")
        layers.setdefault(layer, []).append({"name": c["name"], "plain": plain.get(c["name"], ""), "job": c.get("job", "")})
    for lay in layers.values():
        lay.sort(key=lambda c: c["name"])

    # The apps, checked against the plan that names them.
    apps = data.get("apps", {})
    program = json.loads(files.get(apps.get("program", ""), "{}") or "{}").get("name", "")
    if not program:
        problems.append(f"the program file {apps.get('program')!r} gives no program name")
    for a in apps.get("items", []):
        if a.get("find", "\0") not in files.get(a.get("source", ""), ""):
            problems.append(f"app {a.get('name')}: {a.get('source')} no longer says {a.get('find')!r}")

    # Classes: the kernel's words, and map.json's gloss for each.
    gloss = data.get("classes", {}).get("gloss", {})
    for c in class_names:
        if c not in gloss:
            problems.append(f"chapter 9's class {c} has no plain gloss in map.json")
    for c in gloss:
        if c not in class_names:
            problems.append(f"map.json glosses a class {c} that chapter 9 does not have")

    # The interface: each family of its operations belongs to one operation piece.
    rows = {r["piece"]: r for r in ops["rows"]}
    link = data.get("interface", {})
    families = {}
    for t in tools:
        if t["family"] not in link:
            problems.append(f"the interface's operation {t['name']} is of family {t['family']!r}, which map.json links to no operation")
        for c in t["classes"]:
            if c not in class_names:
                problems.append(f"the interface's operation {t['name']} states a class {c} that chapter 9 does not have")
        families.setdefault(t["family"], []).append(t)
    for fam, piece in link.items():
        if piece not in rows:
            problems.append(f"map.json links the interface's family {fam} to {piece}, which is not an operation")
        if fam not in families:
            problems.append(f"map.json links an interface family {fam} that the interface does not have")
    by_piece = {}
    for fam, ts in families.items():
        if fam in link:
            by_piece.setdefault(link[fam], []).extend(ts)

    # Every operation under the engine that answers it, with its class.
    ours = data.get("classes", {}).get("ours", {})
    for pid in ours:
        if pid not in rows:
            problems.append(f"map.json gives a class for {pid}, which is not an operation")
    sc = data.get("engines", {}).get("solidcore", {})
    if sc.get("class") not in class_names:
        problems.append(f"map.json gives solidcore's answers the class {sc.get('class')!r}, which chapter 9 does not have")
    words = data.get("engines", {}).get("tolerances", {})
    tolerances = []
    for k, v in solidcore["tolerances"].items():
        if k not in words:
            problems.append(f"solidcore's tolerance {k} has no words in map.json")
        else:
            tolerances.append(words[k].format(v))
    engines = {CORE: [], SOLIDCORE: [], NONE: []}
    for r in ops["rows"]:
        op = {"piece": r["piece"], "name": r["name"], "family": r["family"], "state": r["state"]}
        if r["state"] == build.OURS:
            if r["piece"] in by_piece:
                stated = {c for t in by_piece[r["piece"]] for c in t["classes"]}
                op["classes"] = [c for c in class_names if c in stated]
                op["from"] = "the interface's own list of what each of its operations gives"
                op["calls"] = sorted(t["name"] for t in by_piece[r["piece"]])
            elif r["piece"] in ours:
                # One class ("class"), or several ("classes") where the plan gives an operation's
                # answers different classes case by case; shown in chapter 9's order, as above.
                entry = ours[r["piece"]]
                given = entry["classes"] if "classes" in entry else [entry.get("class")]
                op["classes"] = [c for c in class_names if c in given]
                op["from"] = entry.get("from", "")
                for c in given:
                    if c not in class_names:
                        problems.append(f"map.json gives {r['piece']} the class {c!r}, which chapter 9 does not have")
            else:
                problems.append(f"{r['piece']} is answered by our core, but neither the interface nor map.json says its class")
                op["classes"] = []
            engines[CORE].append(op)
        elif r["state"] == build.SC_DONE:
            op["classes"] = [sc.get("class")]
            op["from"] = sc.get("why", "")
            engines[SOLIDCORE].append(op)
        else:
            op["classes"] = []
            if r["state"] == build.SC_HELD:
                op["why"] = build.held_reason(r)
            elif r["landing"]:
                op["why"] = "Built by us, waiting to land."
            engines[NONE].append(op)
    if sum(len(v) for v in engines.values()) != len(ops["rows"]):
        problems.append("an operation was placed under no engine, or under two")

    place = data.get("placement", {})
    iface_layer, format_layer = place.get("interface", ""), place.get("formats", "")
    for tier, lay in (("interface", iface_layer), ("formats", format_layer)):
        if lay not in layer_names:
            problems.append(f"map.json places the {tier} at layer {lay!r}, which chapter 27's layer table does not name")
    tiers = [lay for lay in sorted(layers, key=layer_key) if lay not in (iface_layer, format_layer)]
    tree = {
        "program": program, "apps": apps.get("items", []), "apps_note": apps.get("note", ""),
        "interface": {"layer": iface_layer, "title": data.get("layers", {}).get(iface_layer, ""),
                      "crates": layers.get(iface_layer, []),
                      "families": {f: sorted(t["name"] for t in ts) for f, ts in sorted(families.items())},
                      "count": len(tools)},
        "layers": [{"layer": lay, "name": layer_names.get(lay, ""), "plain": data.get("layers", {}).get(lay, ""),
                    "crates": layers[lay]} for lay in tiers],
        "engines": engines, "engine_words": data.get("engines", {}),
        "solidcore": dict(solidcore, tolerance_words=tolerances),
        "formats": {"layer": format_layer, "name": layer_names.get(format_layer, ""),
                    "plain": data.get("layers", {}).get(format_layer, ""), "crates": layers.get(format_layer, [])},
        "classes": [{"name": c, "kernel": d, "plain": gloss.get(c, "")} for c, d in classes],
    }
    placed = [c["name"] for c in tree["interface"]["crates"] + tree["formats"]["crates"]]
    placed += [c["name"] for lay in tree["layers"] for c in lay["crates"]]
    if sorted(placed) != sorted(names):
        problems.append("a crate of MAP.json is drawn twice or not at all")
    if problems:
        raise MapError(problems)
    return tree


# ---- the page ------------------------------------------------------------------------------------

def crate_box(c):
    e = html.escape
    return (f'<div class="box" title="{e(c["job"])}"><span class="for">{e(c["plain"])}</span>'
            f'<span class="nm">{e(c["name"])}</span></div>')


def class_badges(classes):
    if not classes:
        return '<span class="cls c-none">No answer yet</span>'
    return "".join(f'<span class="cls c-{html.escape(c.lower())}">{html.escape(c)}</span>' for c in classes)


def op_item(op):
    e = html.escape
    note = ""
    if op.get("calls"):
        note += f' <span class="why">Programs call it as {e(", ".join(op["calls"]))}.</span>'
    if op.get("why"):
        note += f' <span class="why">{e(op["why"])}</span>'
    where = f' title="Class from: {e(op["from"])}"' if op.get("from") else ""
    return (f'<li data-piece="{e(op["piece"])}" data-engine="{e(op.get("engine", ""))}"{where}><span class="badges">'
            f'{class_badges(op["classes"])}</span> <span class="opn">{e(op["name"])}</span> '
            f'<span class="id">{e(op["piece"])}</span>{note}</li>')


def op_list(ops, engine):
    for op in ops:
        op["engine"] = engine
    return "<ul class=\"ops\">" + "".join(op_item(o) for o in sorted(ops, key=lambda o: int(o["piece"][1:]))) + "</ul>"


def render_map(tree, meta):
    e = html.escape
    w = tree["engine_words"]
    sc = tree["solidcore"]
    apps = "".join(f'<div class="box app"><span class="for">{e(a["for"])}</span><span class="nm big">{e(a["name"])}</span></div>'
                   for a in tree["apps"])
    fams = "".join(f"<li><b>{e(f)}</b> ({len(n)}): {e(', '.join(n))}</li>" for f, n in tree["interface"]["families"].items())
    layers = "".join(
        f'<section class="layer"><h3><span class="pn">Layer {e(lay["layer"]) if lay["layer"].isdigit() else ""}</span> '
        f'{e(lay["plain"])}</h3>'
        + (f'<p class="why">{e(lay["name"])}</p>' if lay["name"] else "")
        + f'<div class="boxes">{"".join(crate_box(c) for c in lay["crates"])}</div></section>'
        for lay in tree["layers"])
    eng = tree["engines"]
    held = [o for o in eng[NONE] if o["state"] == build.SC_HELD]
    not_yet = [o for o in eng[NONE] if o["state"] != build.SC_HELD]
    by_family = {}
    for o in not_yet:
        by_family.setdefault(o["family"] or "Other", []).append(o)
    not_yet_html = "".join(f"<h4>{e(f)} ({len(os_)})</h4>{op_list(os_, NONE)}" for f, os_ in sorted(by_family.items()))
    classes = "".join(f'<li><span class="cls c-{e(c["name"].lower())}">{e(c["name"])}</span> {e(c["plain"])}. '
                      f'<span class="why">In the kernel\'s words: &ldquo;{e(c["kernel"])}&rdquo;</span></li>'
                      for c in tree["classes"])
    fill = {
        "PROGRAM": e(tree["program"]),
        "APPS": apps,
        "APPS_NOTE": e(tree["apps_note"]),
        "IFACE_TITLE": e(tree["interface"]["title"]),
        "IFACE_CRATES": "".join(crate_box(c) for c in tree["interface"]["crates"]),
        "IFACE_COUNT": str(tree["interface"]["count"]),
        "IFACE_FAMILIES": fams,
        "LAYERS": layers,
        "CORE_NAME": e(w["core"]["name"]), "CORE_FOR": e(w["core"]["for"]),
        "CORE_COUNT": str(len(eng[CORE])), "CORE_OPS": op_list(eng[CORE], CORE),
        "SC_NAME": e(w["solidcore"]["name"]), "SC_FOR": e(w["solidcore"]["for"]),
        "SC_COUNT": str(len(eng[SOLIDCORE])), "SC_OPS": op_list(eng[SOLIDCORE], SOLIDCORE),
        "SC_WRAPPER": e(sc["wrapper"]), "SC_PIN": e(sc["pin"][:9]), "SC_PIN_FULL": e(sc["pin"]),
        "SC_FORK": e(sc["fork"]), "SC_LICENCE": e(sc["licence"]),
        "SC_CASES": str(sc["cases"]), "SC_FAULTS": str(sc["faults"]), "SC_FAULT_CASES": str(sc["fault_cases"]),
        "SC_CLASS": e(w["solidcore"]["class"]), "SC_CLASS_KEY": e(w["solidcore"]["class"].lower()),
"SC_WHY": e(w["solidcore"]["why"]),
        "SC_TOLERANCES": e("; ".join(sc["tolerance_words"])),
        "NONE_NAME": e(w["none"]["name"]), "NONE_FOR": e(w["none"]["for"]),
        "NONE_COUNT": str(len(eng[NONE])), "HELD_COUNT": str(len(held)), "NOT_YET_COUNT": str(len(not_yet)),
        "HELD_OPS": op_list(held, NONE), "NOT_YET_OPS": not_yet_html,
        "CLASSES": classes,
        "FORMATS_TITLE": e(tree["formats"]["plain"]), "FORMATS_NAME": e(tree["formats"]["name"]),
        "FORMATS": "".join(crate_box(c) for c in tree["formats"]["crates"]),
        "BUILT": e(meta["built"]),
    }
    with open(os.path.join(build.HERE, "map.html"), encoding="utf-8") as f:
        page = f.read()
    for k, v in fill.items():
        page = page.replace("{{" + k + "}}", v)
    left = re.findall(r"\{\{[A-Z_]+\}\}", page)
    if left:
        raise MapError([f"the page template has a gap nothing fills: {', '.join(sorted(set(left)))}"])
    return page


def build_map(out_path, offline=False):
    read = build.read
    _, meta = build.gather(offline)
    data = json.loads(read(MAP_DATA))
    apps = data.get("apps", {})
    paths = {apps.get("program", "")} | {a.get("source", "") for a in apps.get("items", [])}
    files = {p: read(p) for p in paths if p and os.path.exists(p)}
    tree = kernel_map(json.loads(read(build.MAP)), read_layer_names(read(LAYERS_MD)), read_tools(json.loads(read(TOOLS))),
                      meta["ops"], data, read_classes(read(CLASSES_MD)),
                      read_solidcore(read(WRAPPER_TOML), json.loads(read(build.CASES)), json.loads(read(build.FAULTS))), files)
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(render_map(tree, meta))
    return tree


def main(argv):
    out = next((a for a in argv if not a.startswith("--")), "kernel-map.html")
    try:
        tree = build_map(out, offline="--offline" in argv)
    except MapError as err:
        print("The kernel map cannot be built:", file=sys.stderr)
        for p in err.args[0]:
            print("  " + p, file=sys.stderr)
        sys.exit(1)
    eng = tree["engines"]
    print(f"{out}: {len(eng[CORE])} operations answered by our core, {len(eng[SOLIDCORE])} by solidcore, "
          f"{len(eng[NONE])} not answered yet; {sum(len(l['crates']) for l in tree['layers'])} crates in "
          f"{len(tree['layers'])} layers, {len(tree['interface']['crates'])} in the interface, "
          f"{len(tree['formats']['crates'])} for files")


if __name__ == "__main__":
    main(sys.argv[1:])
