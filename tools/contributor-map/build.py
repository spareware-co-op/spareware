"""Build the site's contributor map: every merged pull request, joined to the projects it changed.

Usage: python tools/contributor-map/build.py <output.html>
Needs the GitHub CLI (`gh`) signed in, or GH_TOKEN set (the site workflow passes its token), and a
checkout with full history, so that a project folder renamed since (spare-arch became
spare-construct) counts under the project it is today. Uses only Python's standard library; the
page loads d3 (ISC) from cdnjs. Run from the repository root.
"""
import collections
import json
import os
import re
import subprocess
import sys

REPO = os.environ.get("GITHUB_REPOSITORY", "spareware-co-op/spareware")
HERE = os.path.dirname(os.path.abspath(__file__))
QUERY = """query($owner:String!,$name:String!,$cursor:String){ repository(owner:$owner,name:$name){
 pullRequests(states:MERGED,first:40,after:$cursor,orderBy:{field:CREATED_AT,direction:ASC}){
  pageInfo{hasNextPage endCursor}
  nodes{ number title mergedAt author{login} files(first:100){nodes{path additions deletions}} } } } }"""


def run(*args):
    return subprocess.run(args, capture_output=True, text=True, encoding="utf-8", check=True).stdout


def merged_prs():
    owner, name = REPO.split("/")
    out, cursor = [], None
    while True:
        args = ["gh", "api", "graphql", "-f", "query=" + QUERY, "-f", "owner=" + owner, "-f", "name=" + name]
        if cursor:
            args += ["-f", "cursor=" + cursor]
        page = json.loads(run(*args))["data"]["repository"]["pullRequests"]
        for n in page["nodes"]:
            out.append({"n": n["number"], "t": n["title"], "at": n["mergedAt"],
                        "by": (n["author"] or {}).get("login", "ghost"),
                        "files": [(f["path"], f["additions"] + f["deletions"]) for f in n["files"]["nodes"]]})
        if not page["pageInfo"]["hasNextPage"]:
            return out
        cursor = page["pageInfo"]["endCursor"]


def renamed_projects(current):
    """Old project folder -> the folder most of its files moved to, for folders gone from main."""
    moves = collections.defaultdict(collections.Counter)
    log = run("git", "log", "--format=", "--name-status", "-M", "--diff-filter=R", "--", "projects/")
    for line in log.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[1].startswith("projects/") and parts[2].startswith("projects/"):
            old, new = parts[1].split("/")[1], parts[2].split("/")[1]
            if old != new and old not in current:
                moves[old][new] += 1
    return {old: c.most_common(1)[0][0] for old, c in moves.items()}


def app_colours():
    """Each app's light colour from design/DESIGN.md's table, keyed by its name."""
    table = open("design/DESIGN.md", encoding="utf-8").read()
    return {m.group(1): m.group(2) for m in re.finditer(r"^\| (Spare [A-Za-z ]+?) \| (#[0-9A-Fa-f]{6}) ", table, re.M)}


def main(out_path):
    current = {d for d in os.listdir("projects") if os.path.isdir(os.path.join("projects", d))}
    renames = renamed_projects(current)
    colours = app_colours()
    by_slug = {name.lower().replace(" ", "-"): name for name in colours}
    by_slug.update({"kit": "Spare Kit", "engines": "Engines"})

    def place(path):
        p = path.split("/")
        if p[0] == "projects" and len(p) > 2:
            slug = renames.get(p[1], p[1])
            if slug == "_template":
                return ("Repository", "project template")
            if slug == "engines" and p[2] == "geometry":
                if len(p) > 4 and p[3] == "crates":
                    return ("Geometry kernel", p[4])
                return ("Geometry kernel", p[3] if len(p) > 4 else "kernel docs")
            prog = by_slug.get(slug, " ".join(w.capitalize() for w in slug.split("-")))
            if len(p) > 3 and p[2] == "crates":
                return (prog, p[3])
            return (prog, p[2] if len(p) > 3 else "docs")
        if p[0] == ".github":
            return ("Repository", "CI workflows")
        return ("Repository", "top-level docs" if len(p) == 1 else p[0])

    prs = merged_prs()
    area = collections.defaultdict(lambda: collections.defaultdict(lambda: {"prs": set(), "lines": 0}))
    for pr in prs:
        for path, lines in pr["files"]:
            cell = area[place(path)][pr["by"]]
            cell["prs"].add(pr["n"])
            cell["lines"] += lines
    progs = collections.defaultdict(list)
    for (prog, ar), people in area.items():
        progs[prog].append({"name": ar, "children": [
            {"name": who, "prs": len(c["prs"]), "lines": c["lines"], "list": sorted(c["prs"])} for who, c in people.items()]})
    tree = {"name": "Spareware", "children": [
        {"name": prog, "children": sorted(kids, key=lambda k: k["name"])} for prog, kids in sorted(progs.items())]}
    colours.update({"Spare Kit": "var(--spare-red-tint)", "Geometry kernel": "var(--spare-red-tint)",
                    "Engines": "var(--spare-red-tint)", "Repository": "var(--sink)"})
    meta = {"merged": len(prs), "first": min(p["at"] for p in prs)[:10], "last": max(p["at"] for p in prs)[:10],
            "people": collections.Counter(p["by"] for p in prs).most_common(),
            "titles": {p["n"]: p["t"] for p in prs}, "repo": REPO}
    data = json.dumps({"tree": tree, "meta": meta, "colours": colours}, ensure_ascii=False).replace("</", "<\\/")
    page = open(os.path.join(HERE, "page.html"), encoding="utf-8").read()
    script = open(os.path.join(HERE, "graph.js"), encoding="utf-8").read().replace("/*DATA*/null", data)
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(page.replace("<!--GRAPH-->", "<script>\n" + script + "\n</script>"))
    print(f"{out_path}: {len(prs)} merged PRs, {len(tree['children'])} projects; renamed folders {renames}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "contributors.html")
