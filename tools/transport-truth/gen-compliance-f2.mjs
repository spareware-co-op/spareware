// Part 'f2': Compliance+ options and alternatives, plan change mode, the Fast-track package, speed limits and the cost benefit disclosure
// statement, ANOP (compliance-extra.js sections 8 to 10) with their CSV and text exports and report sections
// -> expected/compliance-f2-{options,ftaa,speed,anop}.json and fixtures/data/compliance-f2-*.json
//
// A case: { id, call, args?, fixture, sets, ok | error }. The Rust tests (tests/compliance_f2_*.rs) build the project from the fixture and the
// sets, call the port with the same args and compare. Projects with a mutated network are saved as fixtures after the engine ran on them.
import { part, write, record, saveFixture, withResults, loadCompliance } from './gen-compliance-lib.mjs';
import { runOptions } from './gen-compliance-f2-options.mjs';
import { runFtaa } from './gen-compliance-f2-ftaa.mjs';
import { runSpeed } from './gen-compliance-f2-speed.mjs';
import { runAnop } from './gen-compliance-f2-anop.mjs';

// The report context the tests use: tables come back wrapped so the report lane's block is visible in the text
export const rctx = { table: (title, html, o) => `<TABLE title="${title}" note="${o && o.note}">${html}</TABLE>` };

part('f2', async () => {
  const c = loadCompliance();
  const X = c.ComplianceExtra, C = c.Compliance;
  const cases = { options: [], ftaa: [], speed: [], anop: [] };
  // add(group, id, call, fixture, sets, f, args): records f(P, c) on the project built from the fixture and the sets
  const add = async (group, id, call, fixture, sets, f, args) => {
    const r = await record(c, fixture, sets, f);
    cases[group].push({ id, call, ...(args !== undefined ? { args } : {}), ...r });
  };
  const env = { c, X, C, add, rctx, saveFixture, withResults, cases };
  await runOptions(env);
  await runFtaa(env);
  await runSpeed(env);
  await runAnop(env);
  for (const [g, list] of Object.entries(cases)) write(`expected/compliance-f2-${g}.json`, { cases: list });
});
