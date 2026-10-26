"""geom-bench's variant timings as tables (piece G42; chapter 23.3 of the kernel architecture).

Usage: python report.py <folder> > report.md

Reads every result in <folder>: `<engine>-<build>.json`, as browsers.mjs writes it, or
`<engine>-<build>.txt`, the lines geom-bench-guest's `time` command printed natively, in Wasmtime
or in Node, with an optional first line `# version <text>`. Each timing line is
`<benchmark> <nanoseconds per input> <passes per round>`.

Writes Markdown: for every engine and build, the exact path of every predicate in its three
variants (the shipped width-aware path, plain i128 and width-aware limbs) on typical and extreme
inputs, with the fastest marked; a summary of the fastest variant per predicate and engine; and
the 32-bit and 64-bit limb integers side by side. Python's standard library only.
"""

import json
import pathlib
import sys

VARIANTS = [("shipped", ""), ("plain", "_plain"), ("limbs", "_limbs")]
CLASSES = ["typical", "extreme"]


def load(folder):
    runs = {}
    for path in sorted(pathlib.Path(folder).iterdir()):
        stem, suffix = path.stem, path.suffix
        if suffix not in (".json", ".txt") or "-" not in stem:
            continue
        engine, build = stem.rsplit("-", 1)
        if suffix == ".json":
            data = json.loads(path.read_text(encoding="utf-8"))
            lines, version = data.get("lines", []), data.get("version", "")
            supported = data.get("supported", True)
            errors = data.get("errors", [])
        else:
            text = path.read_text(encoding="utf-8").splitlines()
            version = ""
            if text and text[0].startswith("# version "):
                version = text[0][len("# version "):]
                text = text[1:]
            lines, supported, errors = text, True, []
            if text == ["unsupported"]:
                lines, supported = [], False
        times = {}
        for line in lines:
            parts = line.split()
            if len(parts) >= 2:
                try:
                    times[parts[0]] = float(parts[1])
                except ValueError:
                    errors.append(line)
        runs[(engine, build)] = {"version": version, "supported": supported, "times": times,
                                 "errors": errors}
    return runs


def predicates(runs):
    names = set()
    for run in runs.values():
        for bench in run["times"]:
            if bench.startswith("generated/") and "_value" in bench:
                names.add(bench.split("/")[1].split("_value")[0])
    return sorted(names)


def fmt(ns):
    return "–" if ns is None else (f"{ns:.1f}" if ns < 100 else f"{ns:.0f}")


def main():
    runs = load(sys.argv[1])
    preds = predicates(runs)
    out = []
    out.append("Nanoseconds per call; median of five rounds after a warm-up (geom-bench-guest's "
               "`time`). The fastest variant of each row is in bold.\n")
    best = {}
    for (engine, build), run in sorted(runs.items()):
        title = f"{engine}, {build} build" + (f" ({run['version']})" if run["version"] else "")
        out.append(f"### {title}\n")
        if not run["supported"]:
            out.append("The engine does not validate this build: unsupported.\n")
            continue
        out.append("| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |")
        out.append("| --- | --- | ---: | ---: | ---: | --- |")
        for p in preds:
            for c in CLASSES:
                cells = {v: run["times"].get(f"generated/{p}_value{s}/{c}") for v, s in VARIANTS}
                known = {v: t for v, t in cells.items() if t is not None}
                if not known:
                    continue
                fastest = min(known, key=known.get)
                if c == "typical":
                    best[(p, engine, build)] = fastest
                row = [f"**{fmt(t)}**" if v == fastest else fmt(t) for v, t in cells.items()]
                out.append(f"| {p} | {c} | {' | '.join(row)} | {fastest} |")
        out.append("")
        pairs = [("wide/i256_mul/typical", "wide32/w8_product/typical"),
                 ("wide/i384_mul/typical", "wide32/w12_product/typical"),
                 ("wide/i256_add/typical", "wide32/w8_add/typical"),
                 ("wide/i384_add/typical", "wide32/w12_add/typical")]
        rows = [(a, b, run["times"].get(a), run["times"].get(b)) for a, b in pairs]
        rows = [r for r in rows if r[2] is not None or r[3] is not None]
        if rows:
            out.append("| 64-bit limbs | ns | 32-bit limbs | ns |")
            out.append("| --- | ---: | --- | ---: |")
            for a, b, ta, tb in rows:
                out.append(f"| `{a}` | {fmt(ta)} | `{b}` | {fmt(tb)} |")
            out.append("")
        if run["errors"]:
            out.append(f"{len(run['errors'])} error line(s): " + "; ".join(run["errors"][:5]) + "\n")
    engines = sorted({(e, b) for (_, e, b) in best})
    if engines:
        out.append("### Fastest variant on typical inputs\n")
        out.append("| Predicate | " + " | ".join(f"{e} ({b})" for e, b in engines) + " |")
        out.append("| --- | " + " | ".join("---" for _ in engines) + " |")
        for p in preds:
            out.append(f"| {p} | " + " | ".join(best.get((p, e, b), "–") for e, b in engines) + " |")
        out.append("")
    print("\n".join(out))


if __name__ == "__main__":
    main()
