#!/usr/bin/env python3
"""Check Spareware work pieces out and back in, so two agents never work on the same piece.

    python tools/piece.py list [program]          open pieces nobody has checked out
    python tools/piece.py status A01              who has A01 and what state it is in
    python tools/piece.py checkout A01            check A01 out, or take over a handed-over piece
    python tools/piece.py split A01 "Part title"  make a part of A01 (A01a, A01b...) that can be checked out on its own
    python tools/piece.py release A01 --review    finished: pull request opened, waiting for an owner
    python tools/piece.py release A01 --handover  out of tokens: work pushed, handover note on a draft pull request
    python tools/piece.py release A01             give it back without any work

The lock is the branch piece/<ID>. GitHub lets only one person create a branch name, so two agents
can never both check the same piece out. All work for the piece happens on that branch.
Needs the GitHub CLI (gh), signed in as the contributor.
"""
import datetime
import json
import re
import string
import subprocess
import sys

REPO = "spareware-co-op/spareware"


def gh(*args, check=True):
    r = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if check and r.returncode != 0:
        sys.exit(f"gh {' '.join(args[:3])} failed:\n{(r.stderr or r.stdout).strip()}")
    return r


def issues(state="open", search=None):
    args = ["issue", "list", "--repo", REPO, "--state", state, "--label", "piece", "--limit", "500",
            "--json", "number,title,assignees,labels,updatedAt"]
    if search:
        args += ["--search", search]
    return json.loads(gh(*args).stdout)


def piece_id(issue):
    return issue["title"].split(":")[0].strip()


def labels(issue):
    return {l["name"] for l in issue["labels"]}


def find(pid):
    for i in issues(search=f'"{pid}:" in:title'):
        if piece_id(i) == pid:
            return i
    sys.exit(f"No open issue for piece {pid}.")


def me():
    return gh("api", "user", "--jq", ".login").stdout.strip()


def branch_exists(pid):
    return gh("api", f"repos/{REPO}/git/ref/heads/piece/{pid}", check=False).returncode == 0


def ahead_of_main(pid):
    r = gh("api", f"repos/{REPO}/compare/main...piece/{pid}", "--jq", ".ahead_by", check=False)
    return int(r.stdout.strip() or 0) if r.returncode == 0 else 0


def today():
    return datetime.date.today().isoformat()


def cmd_list(program=None):
    rows = [i for i in issues() if not labels(i) & {"claimed", "in review"}]
    if program:
        rows = [i for i in rows if program in labels(i)]
    for i in sorted(rows, key=piece_id):
        size = next((l.split(": ")[1] for l in labels(i) if l.startswith("size: ")), "?")
        prog = next((l for l in labels(i) if l == "kit" or l.startswith("spare-")), "?")
        note = " (handed over: carry on from its branch)" if "handed over" in labels(i) else ""
        print(f"{piece_id(i):6} {size:7} {prog:13} #{i['number']:<4} {i['title'].split(':', 1)[1].strip()}{note}")
    print(f"\n{len(rows)} open pieces.")


def cmd_status(pid):
    i = find(pid)
    who = ", ".join(a["login"] for a in i["assignees"]) or "nobody"
    state = ("in review" if "in review" in labels(i) else "checked out" if "claimed" in labels(i)
             else "handed over" if "handed over" in labels(i) else "open")
    print(f"{pid} (#{i['number']}): {state}; assigned to {who}; branch piece/{pid} "
          f"{'exists' if branch_exists(pid) else 'not created'}; last activity {i['updatedAt'][:10]}")


def cmd_checkout(pid):
    i = find(pid)
    user = me()
    n = str(i["number"])
    if "claimed" in labels(i) or "in review" in labels(i):
        who = ", ".join(a["login"] for a in i["assignees"]) or "someone"
        sys.exit(f"{pid} is already checked out by {who} (last activity {i['updatedAt'][:10]}). Pick another piece.")
    if "handed over" in labels(i) and branch_exists(pid):
        # take over: the work so far stays on the branch
        gh("issue", "edit", n, "--repo", REPO, "--add-assignee", "@me", "--add-label", "claimed", "--remove-label", "handed over")
        gh("issue", "comment", n, "--repo", REPO, "--body",
           f"Taken over by @{user} on {today()}, carrying on from branch `piece/{pid}` and its handover note.")
        print(f"You've taken over {pid}. Run: git fetch origin && git switch piece/{pid}")
        return
    sha = gh("api", f"repos/{REPO}/git/ref/heads/main", "--jq", ".object.sha").stdout.strip()
    r = gh("api", "-X", "POST", f"repos/{REPO}/git/refs", "-f", f"ref=refs/heads/piece/{pid}", "-f", f"sha={sha}", check=False)
    if r.returncode != 0:
        sys.exit(f"{pid} was just checked out by someone else (its branch piece/{pid} exists). Pick another piece.")
    gh("issue", "edit", n, "--repo", REPO, "--add-assignee", "@me", "--add-label", "claimed")
    gh("issue", "comment", n, "--repo", REPO, "--body",
       f"Checked out by @{user} on {today()}. All work happens on branch `piece/{pid}`.")
    print(f"You have {pid}. Run: git fetch origin && git switch piece/{pid}")


def cmd_split(pid, title):
    parent = find(pid)
    taken = {piece_id(i) for i in issues(state="all", search=f'"{pid}" in:title')}
    letter = next(c for c in string.ascii_lowercase if f"{pid}{c}" not in taken)
    sub = f"{pid}{letter}"
    labs = [l for l in labels(parent) if l not in {"claimed", "in review", "handed over"}]
    args = ["issue", "create", "--repo", REPO, "--title", f"{sub}: {title}",
            "--body", f"Part of #{parent['number']} ({pid}). Check it out on its own with `python tools/piece.py checkout {sub}`."]
    for l in labs:
        args += ["--label", l]
    url = gh(*args).stdout.strip()
    gh("issue", "comment", str(parent["number"]), "--repo", REPO, "--body", f"Split out part {sub}: {title} ({url})")
    print(f"Made {sub}: {url}")


def cmd_release(pid, mode):
    i = find(pid)
    n = str(i["number"])
    user = me()
    if user not in [a["login"] for a in i["assignees"]]:
        sys.exit(f"{pid} isn't checked out by you, so you can't release it.")
    if mode == "--review":
        gh("issue", "edit", n, "--repo", REPO, "--remove-label", "claimed", "--add-label", "in review")
        gh("issue", "comment", n, "--repo", REPO, "--body", f"Finished by @{user} on {today()}; pull request open for review.")
        print(f"{pid} is now in review.")
    elif mode == "--handover":
        if ahead_of_main(pid) == 0:
            sys.exit(f"Nothing is pushed to piece/{pid} yet. Push your work and open a draft pull request with a Handover section first.")
        gh("issue", "edit", n, "--repo", REPO, "--remove-label", "claimed", "--add-label", "handed over", "--remove-assignee", user)
        gh("issue", "comment", n, "--repo", REPO, "--body",
           f"Handed over by @{user} on {today()}. The work so far is on branch `piece/{pid}`; see the draft pull request's Handover section.")
        print(f"{pid} is handed over; the next agent carries on from piece/{pid}.")
    else:
        if ahead_of_main(pid) > 0:
            sys.exit(f"piece/{pid} has work on it. Use --handover so it isn't lost.")
        gh("api", "-X", "DELETE", f"repos/{REPO}/git/refs/heads/piece/{pid}", check=False)
        gh("issue", "edit", n, "--repo", REPO, "--remove-label", "claimed", "--remove-assignee", user)
        gh("issue", "comment", n, "--repo", REPO, "--body", f"Given back by @{user} on {today()} without changes.")
        print(f"{pid} is free again.")


def norm(pid):
    m = re.fullmatch(r"([A-Za-z]{1,2})(\d\d)([A-Za-z]?)", pid.strip())
    if not m:
        sys.exit(f"{pid} doesn't look like a piece ID (for example A01, RD03 or A01b).")
    return m.group(1).upper() + m.group(2) + m.group(3).lower()


def main(argv):
    if len(argv) < 1 or argv[0] in {"-h", "--help", "help"}:
        print(__doc__)
        return
    cmd, rest = argv[0], argv[1:]
    if cmd == "list":
        cmd_list(rest[0] if rest else None)
    elif cmd == "status" and rest:
        cmd_status(norm(rest[0]))
    elif cmd == "checkout" and rest:
        cmd_checkout(norm(rest[0]))
    elif cmd == "split" and len(rest) >= 2:
        cmd_split(norm(rest[0]), " ".join(rest[1:]))
    elif cmd == "release" and rest:
        cmd_release(norm(rest[0]), rest[1] if len(rest) > 1 else "")
    else:
        print(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
