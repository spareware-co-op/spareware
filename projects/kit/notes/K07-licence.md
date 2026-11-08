# K07: Licence

*Decision note. Owner decision, 2026-10-19; changed by the owner on 2026-11-05 to Apache-2.0 for everything (decision 32.45 of the geometry kernel's architecture).*

## Decision

- **Everything:** Apache License 2.0 ([LICENSE](../../../LICENSE)), for the core, the SDK, the examples and the plugin templates. Anyone may use, change and share it, in open or closed products, if they keep the licence text and the `NOTICE` files and say what they changed. It includes a patent grant. Products and add-ins built on top can use any licence.
- **Never closed:** Spareware will never be moved to a closed, paid or subscription licence.
- **No contributor licence agreement:** contributors keep the copyright to their own work and sign off every commit with the [Developer Certificate of Origin](https://developercertificate.org/) (`git commit -s`). With copyright spread across every contributor, no one is given extra rights over what the community builds.

## Why

People giving their spare tokens need to know the work stays open forever. Feedback from the first public posts was clear: the possibility of a later relicense, or a contributor agreement that would allow one, is a deal breaker.

On 2026-11-05 the core moved from MPL-2.0 to Apache-2.0, so that code can move both ways between Spareware and NVIDIA's USD BRep (SMLib) and its Rust port solidcore, which are both Apache-2.0. The promise did not change: the work stays open, and never becomes closed, paid or subscription.

## Where it is recorded

README "Licence", CONTRIBUTING (sign-off), AGENTS.md (sign-off), the join request form, Cargo.toml, ARCHITECTURE.md (open decisions, now decided) and the kit map (KIT-A14-F16).
