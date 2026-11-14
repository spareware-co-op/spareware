"""Build the site's kernel progress page: where the Spare Kit geometry kernel stands, phase by phase.

Usage: python tools/kernel-progress/build.py <output.html> [--offline]
Run from the repository root of a checkout of main with full history (the Site workflow's checkout).
Uses only Python's standard library. Reads GitHub read-only with GH_TOKEN (the Site workflow passes
its own token); with no token, or with --offline, it counts only what main holds.

Every piece on the board (`projects/engines/geometry/architecture/30-build-order.json`, and the
phase and lane tables of chapter 30.4) is put in one of three states:

- on main: main holds it. A piece counts when main's history has a commit or merge for it (a commit
  whose subject starts with its ID, or a merge from a branch named for it), when a crate's status
  in MAP.json says it is built, or when a kernel file is named for it. An owner action counts when
  the decision log (chapter 32) records it and does not say it stays open, or when 30.5 records it
  closed. A gate counts when its decision-log section says "Result: passed". A piece covered in full
  by one on main counts, and so does everything a passed gate needs.
- built, waiting to land: pushed to GitHub but not on main. A piece counts when a merge train
  (`train/...` branches) holds a commit for it or MAP.json there says it is built, when an open pull
  request is titled with its ID, or when a branch `piece/<ID>`, `ready-piece/<ID>` or `join/...`
  naming it has commits main does not.
- not started: everything else.

Gates are not counted as pieces: each phase shows its gate as one line instead.
"""
import datetime
import html
import json
import os
import re
import subprocess
import sys
import urllib.parse
import urllib.request

REPO = os.environ.get("GITHUB_REPOSITORY", "spareware-co-op/spareware")
API = "https://api.github.com"
KERNEL = "projects/engines/geometry"
BOARD_JSON = KERNEL + "/architecture/30-build-order.json"
BOARD_MD = KERNEL + "/architecture/30-build-order.md"
DECISIONS = KERNEL + "/architecture/32-decisions.md"
MAP = KERNEL + "/MAP.json"
HERE = os.path.dirname(os.path.abspath(__file__))

ID = r"G\d{2,3}"
ON_MAIN, WAITING, NOT_STARTED = "main", "waiting", "none"


def base_id(text):
    """`G414c` and `g246` are work on G414 and G246."""
    return "G" + str(int(re.match(r"[Gg](\d+)", text).group(1))).zfill(2)


def ids_in(text):
    """Every piece ID in a piece of prose, with ranges such as `G234 to G237` filled in."""
    out = set()
    for a, b in re.findall(r"\b(" + ID + r")\b(?:[a-z]?\b)?(?: to (" + ID + r")\b)?", text):
        if b:
            out |= {"G" + str(n).zfill(2) for n in range(int(a[1:]), int(b[1:]) + 1)}
        else:
            out.add(a)
    return out


# ---- the board ------------------------------------------------------------------------------------

def read_board(board_json, board_md):
    """Phases in order: number, name, gate ID and pieces (id, title, lane, depends_on, covered_by)."""
    lanes, names, gates = {}, {}, set()
    for line in board_md.splitlines():
        m = re.match(r"### Phase (\d+): (.+?) \(\d+ pieces\)", line)
        if m:
            names[int(m.group(1))] = m.group(2)
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 6 and re.fullmatch(ID, cells[0]):
            lanes[cells[0]] = cells[4]
        # 30.2's table: phase, what it builds, starts after, and the gate, which opens with its ID.
        if len(cells) == 4 and re.match(r"\d+\. ", cells[0]):
            g = re.match(r"(" + ID + r")\b", cells[3])
            if g:
                gates.add(g.group(1))
    phases = {}
    for p in board_json:
        n = int(p["phase"].split()[1])
        ph = phases.setdefault(n, {"n": n, "name": names.get(n, p["phase"]), "gate": None, "pieces": []})
        piece = {"id": p["id"], "title": p["title"], "lane": lanes.get(p["id"], ""),
                 "depends_on": p.get("depends_on", []), "covered_by": p.get("covered_by")}
        if piece["lane"] == "Gate" or piece["id"] in gates:
            ph["gate"] = piece
        else:
            ph["pieces"].append(piece)
    return [phases[n] for n in sorted(phases)]


# ---- what main holds -------------------------------------------------------------------------------

def ids_from_subjects(subjects):
    """Pieces named by commit subjects: `G101: ...`, or a merge from `.../piece/G101` and the like."""
    out = set()
    for s in subjects:
        m = re.match(r"(" + ID + r")[a-z]?\b", s)
        if m:
            out.add(m.group(1))
        m = re.search(r"\bfrom \S*?/(?:piece|ready-piece|signed|stack)/(" + ID + r")", s)
        if m:
            out.add(m.group(1))
    return out


def ids_from_map(map_json):
    """Pieces a crate's status in MAP.json says are built.

    A status of `scaffold only; built by pieces ...` lists pieces still to come, as does any clause
    `built by pieces ...`, so neither counts. In any other clause that says built, the IDs before
    `to come`, `come in`, `come with`, `the rest`, `the other` or `wait` count."""
    out = set()
    for crate in map_json.get("crates", []):
        status = crate.get("status") or ""
        if status.startswith("scaffold only"):
            continue
        for clause in status.split(";"):
            if "built" not in clause or "built by pieces" in clause:
                continue
            done = re.split(r"to come|come in|come with|the rest|the other|\bwait", clause)[0]
            out |= set(re.findall(r"\b(" + ID + r")[a-z]?\b", done))
    return out


def ids_from_file_names(paths):
    """Pieces a kernel file is named for: `tests/attack_g101.rs`, `replay_g86.rs` and the like."""
    out = set()
    for p in paths:
        for m in re.finditer(r"(?:^|[_/.-])[gG](\d{2,3})[a-z]?(?=[_./-]|$)", p):
            out.add("G" + m.group(1).zfill(2))
    return out


def read_decisions(decisions_md):
    """Owner actions the decision log records (and those it says stay open), and passed gates."""
    owner, still_open, gates = set(), set(), {}
    for sec in re.split(r"(?m)^## ", decisions_md)[1:]:
        head, _, body = sec.partition("\n")
        m = re.search(r"\(owner actions? ([^)]*)\)", head)
        if m:
            named = ids_in(m.group(1))
            if re.search(r"stays open|open until", body):
                still_open |= named
            else:
                owner |= named
        g = re.search(r"Gate \d+:.*\((" + ID + r")\)", head)
        if g:
            r = re.search(r"\*\*Result: passed \(([0-9-]{10})", body)
            if r:
                gates[g.group(1)] = r.group(1)
    return owner, still_open, gates


def read_closed_owner_rows(board_md):
    """Owner rows 30.5 records closed: bold-led paragraphs whose label says closed.

    Sentences that name an ID as closed or needing no search close it; a later sentence saying a
    piece keeps work, or that something waits for it, opens it again."""
    closed = set()
    for para in board_md.split("\n\n"):
        m = re.match(r"\*\*([^*]+)\*\*", para.strip())
        if not m or "closed" not in m.group(1):
            continue
        kept = set()
        for sentence in re.split(r"(?<=\.) ", para):
            if re.search(r"\bkeeps?\b|waits? for", sentence):
                kept |= ids_in(sentence)
            elif re.search(r"closed|no search", sentence):
                closed |= ids_in(sentence)
        closed -= kept
    return closed


def on_main(phases, history, map_ids, file_ids, decisions, closed_rows):
    """Every board ID main holds, as defined in the module's docstring."""
    owner, still_open, gates = decisions
    lanes = {p["id"]: p["lane"] for ph in phases for p in ph["pieces"]}
    by_id = {p["id"]: p for ph in phases for p in ph["pieces"] + ([ph["gate"]] if ph["gate"] else [])}
    evidence = history | map_ids | file_ids
    done = set()
    for pid in lanes:
        if pid in still_open:
            continue
        if pid in owner or pid in closed_rows or pid in evidence:
            done.add(pid)
    for pid, p in by_id.items():
        cover = p.get("covered_by")
        if cover and all(c in evidence for c in re.findall(ID, cover)):
            done.add(pid)
    # A passed gate holds every piece it needs, and what they need, as recorded at the gate.
    stack = [g for g in gates if g in by_id]
    while stack:
        g = stack.pop()
        if g in done and g not in gates:
            continue
        done.add(g)
        stack += [d for d in by_id[g]["depends_on"] if d in by_id and d not in done]
    return done


# ---- counting --------------------------------------------------------------------------------------

def count(phases, main_ids, waiting_ids, gates_passed):
    """Per phase and overall: how many pieces are on main, built and waiting, and not started."""
    rows, total = [], {ON_MAIN: 0, WAITING: 0, NOT_STARTED: 0}
    for ph in phases:
        state = {}
        for p in ph["pieces"]:
            pid = p["id"]
            state[pid] = ON_MAIN if pid in main_ids else WAITING if pid in waiting_ids else NOT_STARTED
        c = {s: sum(1 for v in state.values() if v == s) for s in (ON_MAIN, WAITING, NOT_STARTED)}
        for s in c:
            total[s] += c[s]
        gate = ph["gate"]
        rows.append({"n": ph["n"], "name": ph["name"], "gate": gate["id"] if gate else None,
                     "gate_title": gate["title"] if gate else "",
                     "gate_passed": gates_passed.get(gate["id"]) if gate else None,
                     "counts": c, "size": len(ph["pieces"]), "state": state,
                     "titles": {p["id"]: p["title"] for p in ph["pieces"]}})
    size = sum(total.values())
    return {"phases": rows, "total": total, "size": size,
            "pct_main": 100 * total[ON_MAIN] // size if size else 0,
            "pct_built": 100 * (total[ON_MAIN] + total[WAITING]) // size if size else 0}


def next_gate(result):
    """The first gate not passed, and what it waits on, as one plain sentence."""
    for ph in result["phases"]:
        if not ph["gate"] or ph["gate_passed"]:
            continue
        left = [pid for pid, s in ph["state"].items() if s != ON_MAIN]
        lead = f"Next gate: {ph['gate']}, which closes phase {ph['n']} ({ph['name']})."
        if not left:
            return lead + " Every piece of the phase is on main; it waits on its gate review being recorded."
        c = ph["counts"]
        parts = []
        if c[WAITING]:
            parts.append(f"{c[WAITING]} built and waiting to land")
        if c[NOT_STARTED]:
            parts.append(f"{c[NOT_STARTED]} not started")
        named = ", ".join(sorted(left, key=lambda x: int(x[1:]))[:8]) + (" and more" if len(left) > 8 else "")
        many = "pieces" if len(left) > 1 else "piece"
        return f"{lead} It waits on {len(left)} {many} ({' and '.join(parts)}): {named}."
    return "Every gate has passed."


# ---- GitHub, read-only -----------------------------------------------------------------------------

def gh_get(path, token, accept="application/vnd.github+json", data=None):
    req = urllib.request.Request(API + path, data=data, headers={
        "Authorization": "Bearer " + token, "Accept": accept, "User-Agent": "spareware-site-build",
        "X-GitHub-Api-Version": "2022-11-28"})
    with urllib.request.urlopen(req, timeout=60) as r:
        body = r.read()
    return body if accept.endswith("raw") else json.loads(body)


def gh_pages(path, token):
    out, page = [], 1
    while True:
        sep = "&" if "?" in path else "?"
        batch = gh_get(f"{path}{sep}per_page=100&page={page}", token)
        out += batch
        if len(batch) < 100:
            return out
        page += 1


def ahead_of_main(branches, token):
    """Branch name -> how many commits it has that main does not, in batches of GraphQL compares."""
    owner, name = REPO.split("/")
    out = {}
    for i in range(0, len(branches), 50):
        batch = branches[i:i + 50]
        fields = " ".join(f'b{j}: compare(headRef: {json.dumps(b)}) {{ aheadBy }}' for j, b in enumerate(batch))
        q = f'query {{ repository(owner: {json.dumps(owner)}, name: {json.dumps(name)}) {{ ref(qualifiedName: "refs/heads/main") {{ {fields} }} }} }}'
        res = gh_get("/graphql", token, data=json.dumps({"query": q}).encode())
        ref = ((res.get("data") or {}).get("repository") or {}).get("ref") or {}
        for j, b in enumerate(batch):
            out[b] = (ref.get(f"b{j}") or {}).get("aheadBy", 0)
    return out


def waiting_on_github(token):
    """Pieces pushed to GitHub but not on main, and where each count came from."""
    r = urllib.parse.quote
    branches = [b["name"] for b in gh_pages(f"/repos/{REPO}/branches", token)]
    trains = sorted(b for b in branches if b.startswith("train/"))
    named = [b for b in branches if re.match(r"(?:piece|ready-piece)/G\d+[a-z]?$", b)
             or (b.startswith("join/") and re.search(r"(?<![A-Za-z0-9])[gG]\d{2,3}", b))]
    ahead = ahead_of_main(trains + named, token)
    found, sources = set(), {}

    def add(ids, where):
        for i in ids:
            found.add(i)
            sources.setdefault(i, where)

    for t in trains:
        if not ahead.get(t):
            continue
        subjects, page = [], 1
        while True:
            cmp = gh_get(f"/repos/{REPO}/compare/main...{r(t, safe='')}?per_page=100&page={page}", token)
            subjects += [c["commit"]["message"].splitlines()[0] for c in cmp.get("commits", [])]
            if len(cmp.get("commits", [])) < 100:
                break
            page += 1
        add(ids_from_subjects(subjects), t)
        raw = gh_get(f"/repos/{REPO}/contents/{MAP}?ref={r(t, safe='')}", token, "application/vnd.github.raw")
        add(ids_from_map(json.loads(raw)), t)
    for pr in gh_pages(f"/repos/{REPO}/pulls?state=open", token):
        title, labels = pr["title"], {l["name"] for l in pr.get("labels", [])}
        m = re.match(r"(" + ID + r")[a-z]?\b", title)
        if m or "piece" in labels:
            add({base_id(m.group(1))} if m else ids_in(title), f"pull request #{pr['number']}")
    for b in named:
        if ahead.get(b):
            add({base_id(x) for x in re.findall(r"(?<![A-Za-z0-9])[gG]\d{2,3}", b.split("/", 1)[1])}, "branch " + b)
    return found, {"trains": [t for t in trains if ahead.get(t)], "branches": sum(1 for b in named if ahead.get(b))}


# ---- the page --------------------------------------------------------------------------------------

def bar(c, size):
    if not size:
        return ""
    segs = []
    for state, cls in ((ON_MAIN, "s-main"), (WAITING, "s-wait"), (NOT_STARTED, "s-none")):
        if c[state]:
            segs.append(f'<span class="{cls}" style="width:{100 * c[state] / size:.3f}%"></span>')
    return f'<div class="bar" role="img" aria-label="{c[ON_MAIN]} on main, {c[WAITING]} built and waiting to land, {c[NOT_STARTED]} not started">{"".join(segs)}</div>'


def render(result, meta):
    e = html.escape
    t = result["total"]
    phases = []
    for ph in result["phases"]:
        c = ph["counts"]
        if ph["gate"]:
            gate = (f"Gate {e(ph['gate'])} passed on {e(ph['gate_passed'])}." if ph["gate_passed"]
                    else f"Gate {e(ph['gate'])} not passed yet.")
        else:
            gate = ""
        lists = []
        for state, label in ((ON_MAIN, "On main"), (WAITING, "Built, waiting to land"), (NOT_STARTED, "Not started")):
            ids = sorted((i for i, s in ph["state"].items() if s == state), key=lambda x: int(x[1:]))
            if ids:
                items = "".join(f'<li><span class="id">{e(i)}</span> {e(ph["titles"][i])}</li>' for i in ids)
                lists.append(f"<h4>{label} ({len(ids)})</h4><ul>{items}</ul>")
        phases.append(f"""<section class="phase">
  <div class="phase-head"><h3><span class="pn">Phase {ph['n']}</span> {e(ph['name'])}</h3>
  <span class="num">{c[ON_MAIN]} / {ph['size']}</span></div>
  {bar(c, ph['size'])}
  <p class="counts"><span><span class="k k-main"></span>{c[ON_MAIN]} on main</span> <span><span class="k k-wait"></span>{c[WAITING]} built, waiting to land</span> <span><span class="k k-none"></span>{c[NOT_STARTED]} not started.</span> <span>{gate}</span></p>
  <details><summary>The pieces</summary>{''.join(lists)}</details>
</section>""")
    with open(os.path.join(HERE, "page.html"), encoding="utf-8") as f:
        page = f.read()
    fill = {
        "PCT": str(result["pct_main"]),
        "PCT_BUILT": str(result["pct_built"]),
        "ON_MAIN": str(t[ON_MAIN]), "WAITING": str(t[WAITING]), "NOT_STARTED": str(t[NOT_STARTED]),
        "SIZE": str(result["size"]), "PHASES": str(len(result["phases"])),
        "BAR": bar(t, result["size"]),
        "NEXT": e(meta["next"]),
        "PHASE_LIST": "\n".join(phases),
        "BUILT": e(meta["built"]),
        "SOURCE": e(meta["source"]),
    }
    for k, v in fill.items():
        page = page.replace("{{" + k + "}}", v)
    return page


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, encoding="utf-8", check=True).stdout


def build(out_path, offline=False):
    def read(path):
        with open(path, encoding="utf-8") as f:
            return f.read()
    board_md = read(BOARD_MD)
    phases = read_board(json.loads(read(BOARD_JSON)), board_md)
    decisions = read_decisions(read(DECISIONS))
    history = ids_from_subjects(git("log", "--format=%s", "HEAD").splitlines())
    paths = [p for p in git("ls-files", "projects/engines").splitlines() if p.endswith(".rs")]
    main_ids = on_main(phases, history, ids_from_map(json.loads(read(MAP))), ids_from_file_names(paths),
                       decisions, read_closed_owner_rows(board_md))
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token and not offline:
        waiting, seen = waiting_on_github(token)
        source = (f"It counts what is pushed to GitHub: main, the merge trains ({', '.join(seen['trains']) or 'none'}), "
                  f"open pull requests named for a piece, and {seen['branches']} piece and join branches with work main does not have.")
    else:
        waiting, source = set(), "This copy was built without reading GitHub, so it counts only what main holds."
    result = count(phases, main_ids, waiting - main_ids, decisions[2])
    sha = git("rev-parse", "--short=8", "HEAD").strip()
    now = datetime.datetime.now(datetime.timezone.utc)
    meta = {"next": next_gate(result), "source": source,
            "built": f"{now:%d %B %Y}, {now:%H:%M} UTC, from main at {sha}"}
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(render(result, meta))
    return result, meta


def main(argv):
    out = next((a for a in argv if not a.startswith("--")), "kernel-progress.html")
    result, meta = build(out, offline="--offline" in argv)
    t = result["total"]
    print(f"{out}: {result['pct_main']}% on main ({t[ON_MAIN]} of {result['size']} pieces), "
          f"{t[WAITING]} built and waiting, {t[NOT_STARTED]} not started")
    for ph in result["phases"]:
        c = ph["counts"]
        print(f"  phase {ph['n']:>2}: {c[ON_MAIN]:>3} on main, {c[WAITING]:>3} built, {c[NOT_STARTED]:>3} not started"
              f" (gate {ph['gate']}: {'passed ' + ph['gate_passed'] if ph['gate_passed'] else 'not passed'})")
    print(meta["next"])


if __name__ == "__main__":
    main(sys.argv[1:])
