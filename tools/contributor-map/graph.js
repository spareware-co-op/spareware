const DATA = /*DATA*/null;
const REPO = "https://github.com/" + (DATA.meta.repo || "spareware-co-op/spareware") + "/pull/";
const fmt = d3.format(",");
const colourOf = name => (DATA.colours && DATA.colours[name]) || "var(--sink)";
let by = "prs";
const open = new Set();

// Flatten the tree: projects, their parts, and per-person totals at each level.
const programs = DATA.tree.children.map((p, i) => {
  const areas = p.children.map(a => ({ name: a.name, people: a.children.map(c => ({ name: c.name, prs: c.prs, lines: c.lines, list: c.list })) }));
  const pp = new Map();
  areas.forEach(a => a.people.forEach(c => { const q = pp.get(c.name) || { name: c.name, prs: new Set(), lines: 0 };
    c.list.forEach(n => q.prs.add(n)); q.lines += c.lines; pp.set(c.name, q); }));
  const people = [...pp.values()].map(q => ({ name: q.name, prs: q.prs.size, lines: q.lines, list: [...q.prs].sort((a, b) => b - a) }));
  return { id: "p:" + p.name, name: p.name, tone: colourOf(p.name), areas, people };
});
const personTotals = new Map();
programs.forEach(p => p.people.forEach(c => { const q = personTotals.get(c.name) || { prs: new Set(), lines: 0 };
  c.list.forEach(n => q.prs.add(n)); q.lines += c.lines; personTotals.set(c.name, q); }));

const svg = d3.select("#graph"), stage = svg.append("g");
const gEdges = stage.append("g"), gNodes = stage.append("g");
const zoom = d3.zoom().scaleExtent([0.2, 6]).on("zoom", e => stage.attr("transform", e.transform));
svg.call(zoom).on("dblclick.zoom", null);
const nodes = new Map();
function nodeFor(id, make) { if (!nodes.has(id)) nodes.set(id, make()); return nodes.get(id); }
function weight(o) { return by === "prs" ? o.prs : o.lines; }

function build() {
  const N = [], L = [];
  const maxProgram = d3.max(programs, p => d3.sum(p.people, weight)) || 1;
  const maxPerson = d3.max([...personTotals.values()], v => by === "prs" ? v.prs.size : v.lines) || 1;
  personTotals.forEach((t, name) => {
    const n = nodeFor("u:" + name, () => ({ id: "u:" + name, kind: "person", name }));
    n.r = 10 + 14 * Math.sqrt((by === "prs" ? t.prs.size : t.lines) / maxPerson); N.push(n);
  });
  programs.forEach(p => {
    const n = nodeFor(p.id, () => ({ id: p.id, kind: "program", name: p.name, tone: p.tone, ref: p }));
    n.r = 6 + 18 * Math.sqrt(d3.sum(p.people, weight) / maxProgram); N.push(n);
    if (open.has(p.id)) {
      p.areas.forEach(a => {
        const an = nodeFor(p.id + "/" + a.name, () => ({ id: p.id + "/" + a.name, kind: "area", name: a.name, tone: p.tone, ref: a, program: p, x: n.x, y: n.y }));
        an.r = 4 + 8 * Math.sqrt(d3.sum(a.people, weight) / maxProgram); N.push(an);
        L.push({ source: p.id, target: an.id, kind: "part", w: 1 });
        a.people.forEach(c => L.push({ source: an.id, target: "u:" + c.name, kind: "work", w: weight(c), data: c, where: a.name }));
      });
    } else p.people.forEach(c => L.push({ source: p.id, target: "u:" + c.name, kind: "work", w: weight(c), data: c, where: p.name }));
  });
  return { N, L };
}

let sim, edgesAll, nodesAll;
function render(keepView) {
  const { N, L } = build();
  const maxW = d3.max(L.filter(l => l.kind === "work"), l => l.w) || 1;
  const width = d3.scaleSqrt().domain([0, maxW]).range([0.6, 9]);
  const key = l => (l.source.id || l.source) + ">" + (l.target.id || l.target);
  const edges = gEdges.selectAll("line").data(L, key);
  edges.exit().remove();
  edgesAll = edges.enter().append("line").attr("class", "edge").merge(edges)
    .attr("stroke-width", l => l.kind === "part" ? 1 : width(l.w))
    .attr("stroke-dasharray", l => l.kind === "part" ? "3 3" : null);
  edgesAll.selectAll("title").remove();
  edgesAll.filter(l => l.kind === "work").append("title")
    .text(l => `${l.data.name} in ${l.where}: ${l.data.prs} PR${l.data.prs === 1 ? "" : "s"}, ${fmt(l.data.lines)} lines`);
  const ns = gNodes.selectAll("g.node").data(N, n => n.id);
  ns.exit().remove();
  const enter = ns.enter().append("g").attr("class", n => "node " + n.kind).attr("tabindex", 0).attr("role", "button");
  enter.append("circle");
  enter.append("text");
  nodesAll = enter.merge(ns);
  nodesAll.attr("aria-label", n => n.name);
  nodesAll.select("circle").attr("r", n => n.r).attr("style", n => n.kind === "person" ? null : `fill: ${n.tone}`);
  nodesAll.select("text").text(n => n.name).attr("x", n => n.r + 5).attr("y", 5);
  nodesAll.on("click", (e, n) => pick(n))
    .on("keydown", (e, n) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(n); } })
    .on("mouseenter", (e, n) => hover(n)).on("mouseleave", () => hover(null));
  nodesAll.call(d3.drag()
    .on("start", (e, n) => { if (!e.active) sim.alphaTarget(.2).restart(); n.fx = n.x; n.fy = n.y; })
    .on("drag", (e, n) => { n.fx = e.x; n.fy = e.y; })
    .on("end", (e, n) => { if (!e.active) sim.alphaTarget(0); n.fx = null; n.fy = null; }));
  if (sim) sim.stop();
  sim = d3.forceSimulation(N)
    .force("link", d3.forceLink(L).id(n => n.id).distance(l => l.kind === "part" ? 55 : 190).strength(l => l.kind === "part" ? 1 : 0.15))
    .force("charge", d3.forceManyBody().strength(n => n.kind === "area" ? -60 : -420))
    .force("collide", d3.forceCollide().radius(n => n.r + (n.kind === "area" ? 8 : 26)))
    .force("x", d3.forceX(0).strength(0.03)).force("y", d3.forceY(0).strength(0.03))
    .on("tick", tick);
  if (!keepView) { sim.stop(); for (let i = 0; i < 300; i++) sim.tick(); tick(); fit(); }
  else sim.alpha(0.6).restart();
}
function tick() {
  edgesAll.attr("x1", l => l.source.x).attr("y1", l => l.source.y).attr("x2", l => l.target.x).attr("y2", l => l.target.y);
  nodesAll.attr("transform", n => `translate(${n.x},${n.y})`);
}
function hover(n) {
  if (!n) { edgesAll.classed("hot", false).classed("dim", false); nodesAll.classed("dim", false); return; }
  const near = new Set([n.id]);
  const touches = l => l.source.id === n.id || l.target.id === n.id;
  edgesAll.each(l => { if (touches(l)) { near.add(l.source.id); near.add(l.target.id); } });
  edgesAll.classed("hot", touches).classed("dim", l => !touches(l));
  nodesAll.classed("dim", m => !near.has(m.id));
}
function fit() {
  const box = document.getElementById("map"), w = box.clientWidth, h = box.clientHeight;
  const ns = [...nodes.values()].filter(n => n.x !== undefined && nodesAll.data().includes(n));
  if (!ns.length || !w || !h) return;
  const x0 = d3.min(ns, n => n.x - n.r - 10), x1 = d3.max(ns, n => n.x + n.r + 140);
  const y0 = d3.min(ns, n => n.y - n.r - 14), y1 = d3.max(ns, n => n.y + n.r + 14);
  const k = Math.min(2.5, 0.94 * Math.min(w / (x1 - x0), h / (y1 - y0)));
  svg.attr("viewBox", `0 0 ${w} ${h}`);
  svg.transition().duration(400).call(zoom.transform,
    d3.zoomIdentity.translate(w / 2, h / 2).scale(k).translate(-(x0 + x1) / 2, -(y0 + y1) / 2));
}
function pick(n) {
  if (n.kind === "program") { open.has(n.id) ? open.delete(n.id) : open.add(n.id); render(true); showProgram(n.ref); }
  else if (n.kind === "area") showArea(n.ref, n.program);
  else showPerson(n.name);
}
function panel(title, kind, rows, list) {
  document.getElementById("here").textContent = title;
  document.getElementById("kind").textContent = kind;
  const tb = document.querySelector("#people tbody"); tb.replaceChildren();
  rows.forEach(r => { const tr = document.createElement("tr");
    tr.innerHTML = `<td></td><td class="n"></td><td class="n"></td>`;
    tr.children[0].textContent = r.name;
    tr.children[1].textContent = `${r.prs} PR${r.prs === 1 ? "" : "s"}`;
    tr.children[2].textContent = `${fmt(r.lines)} lines`;
    tb.appendChild(tr); });
  const box = document.getElementById("prs"); box.replaceChildren();
  document.getElementById("prs-head").textContent = `Pull requests (${list.length})`;
  list.slice(0, 200).forEach(n => { const d = document.createElement("div");
    const a = document.createElement("a"); a.href = REPO + n; a.target = "_blank"; a.rel = "noopener"; a.textContent = "#" + n;
    d.appendChild(a); d.append(" " + (DATA.meta.titles[n] || "")); box.appendChild(d); });
}
const sortRows = rows => rows.sort((a, b) => weight(b) - weight(a));
const prsOf = people => { const s = new Set(); people.forEach(c => c.list.forEach(n => s.add(n))); return [...s].sort((a, b) => b - a); };
function showProgram(p) { panel(p.name, "Project", sortRows([...p.people]), prsOf(p.people)); }
function showArea(a, p) { panel(a.name, `Part of ${p.name}`, sortRows([...a.people]), prsOf(a.people)); }
function showPerson(name) {
  const rows = programs.map(p => { const c = p.people.find(c => c.name === name); return c && { name: p.name, prs: c.prs, lines: c.lines }; }).filter(Boolean);
  panel(name, "Contributor, by project", sortRows(rows), [...personTotals.get(name).prs].sort((a, b) => b - a));
}
function showAll() {
  const rows = [...personTotals.entries()].map(([name, t]) => ({ name, prs: t.prs.size, lines: t.lines }));
  panel("Spareware", "Whole project", sortRows(rows), Object.keys(DATA.meta.titles).map(Number).sort((a, b) => b - a));
}
document.getElementById("by-prs").addEventListener("click", () => setBy("prs"));
document.getElementById("by-lines").addEventListener("click", () => setBy("lines"));
document.getElementById("fit").addEventListener("click", fit);
document.getElementById("collapse").addEventListener("click", () => { open.clear(); render(false); showAll(); });
function setBy(v) {
  by = v;
  document.getElementById("by-prs").setAttribute("aria-pressed", v === "prs");
  document.getElementById("by-lines").setAttribute("aria-pressed", v === "lines");
  render(true);
}
const m = DATA.meta;
document.getElementById("stats").innerHTML = `<span><b>${m.merged}</b> merged PRs</span><span><b>${programs.length}</b> projects</span><span><b>${m.people.length}</b> contributors</span><span><b>${m.first}</b> to <b>${m.last}</b></span>`;
document.getElementById("foot").textContent = "Contributors: " + m.people.map(([p, n]) => `${p} (${n} PRs)`).join(", ") + ". Rebuilt from GitHub on every merge to main by tools/contributor-map/build.py.";
let fitTimer; new ResizeObserver(() => { clearTimeout(fitTimer); fitTimer = setTimeout(fit, 150); }).observe(document.getElementById("map"));
render(false);
showAll();
