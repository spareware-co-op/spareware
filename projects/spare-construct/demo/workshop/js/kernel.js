/* Spare Construct · Workshop: parts built by the geometry kernel.

   pkg/ holds crate construct-workshop compiled to WebAssembly (server/scripts/build-workshop.sh makes it): the
   kernel's facade, engine-geometry, builds each part's recipe, the same JSON the prototype's builder takes. When it
   loads, the page builds with it and its badge says so; if it cannot load, the prototype's builder carries on. */

import init, { build, edition } from '../pkg/construct_workshop.js';

export async function useKernel(frame) {
  await init();
  const ed = JSON.parse(edition());
  frame.useBuilder((part) => {
    const a = JSON.parse(build(JSON.stringify(part)));
    if (a.failed) throw new Error(a.failed.message);
    return a;
  }, {
    id: 'kernel',
    label: 'Geometry kernel',
    tip: `Built by Spare Kit's geometry kernel (${ed.kernel}, compiled for the browser) on its ${ed.grid} grid. What it cannot build yet waits.`,
  });
}
