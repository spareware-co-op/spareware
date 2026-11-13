# Decision note: contacts that lie on a seam (chapter 12.9 and 12.10)

Status: proposed, for an owner to accept. The code that follows it is on the same pull request and
works either way; only the second question below changes a type chapter 12.2 defines.

## The question

Subdivision cuts a closing domain (a whole turn of a cylinder, cone, sphere or torus) at its ends, so
a contact that lies on a seam is flagged from both sides. Each side's cluster of pairs is read alone
(12.9), with only the half of the evidence on its side. Three things go wrong:

1. A singular point on a seam is read twice, each time with half its half-branches (Steinmetz's
   solid: three singular points, of 2, 2 and 4 half-branches, for two of 4 and 4).
2. A tangent point on a seam is two clusters, each of them a band or a contact of higher order
   (`c3_spheres_external`, `c3_spheres_internal` in the SSI corpus), so it stays undecided.
3. Where a reading's box would have to hold a point at u = 0 and at u = 2 at once, no box of the
   domain does.

## Options

A. **Join the readings afterwards** (what is built for singular points, `across_seams`). Sound for
   point readings whose two enclosures meet after a whole-turn move, since each holds the one point.
   It cannot decide a contact that neither side could read alone, so it does not help (2).

B. **Re-chart** (built, `turned_seams`). The same surfaces are cut half a turn on: a domain of
   (1, 3) instead of (0, 2) is the same surface with the seam at u = 1. The patches of the first
   subdivision are moved into it (those in the first half by a whole turn), the clusters are read
   again, and a reading that holds the first chart's cut comes from the second chart, where nothing
   is cut. This is the "local re-chart" 12.10 already uses at a pole. It costs one more reading of
   the clusters, only when a reading at a seam is left undecided or a tangent curve, and no new
   subdivision.

C. **Cut the domain at the contact.** Choose the cut from the contact. Needs to know the contact
   before it is read.

## Evidence

- Steinmetz (`tests/singular_on_a_seam.rs`): A fixes it, [2, 2, 4] becomes [4, 4].
- `c3_spheres_external`: both clusters read as bands of 102 and 108 steps at u = 0 and u = 2, which
  are one cluster in the other chart.
- B needs `with_domain` on the round kinds and the plane (added here): a kind's formulas go on past
  its domain, so the same surface over another domain is the same points.

## The Reading type

A reading of the second chart that holds the first chart's cut has a box that reaches across it
(u from 1.9995 to 2.0005). The first chart's domain ends at 2, and the part past it is the points
from 0 on. Two ways to state this in 12.2:

1. **A box on a closing parameter may reach past the domain's end** by what the reading reaches; the
   part past the end is the same points as the domain's start. Nothing else about `Reading`
   changes. Callers that index by the box must take the end modulo the turn.
2. **Split the box in two** at the end and report two readings of one contact. Keeps every box in the
   domain, but a contact is then two readings again, which is what this note removes.

## Recommendation

B with option 1. It decides a contact where it lies, adds no type, and puts the one rule about a
closing parameter in the chapter: a box may reach past its end. The owner accepts, rejects or changes
the second question; if rejected, B's readings at a seam go back to A's.
