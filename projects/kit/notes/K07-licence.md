# K07: Licence

*Decision note. Owner decision, 2026-10-19.*

## Decision

- **Core:** Mozilla Public License 2.0 ([LICENSE](../../../LICENSE)). Changes to Spareware's own files must be shared openly; products and add-ins built on top can use any licence.
- **SDK, examples and plugin templates:** Apache License 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE)), so anyone can build add-ins under any licence, with a patent grant.
- **Never relicensed:** Spareware will never be moved to a closed, paid or subscription licence.
- **No contributor licence agreement:** contributors keep the copyright to their own work and sign off every commit with the [Developer Certificate of Origin](https://developercertificate.org/) (`git commit -s`). With copyright spread across every contributor, nobody can relicense what the community builds.

## Why

People giving their spare tokens need to know the work stays open forever. Feedback from the first public posts was clear: the possibility of a later relicense, or a contributor agreement that would allow one, is a deal breaker.

## Where it is recorded

README "Licence", CONTRIBUTING (sign-off), AGENTS.md (sign-off), the join request form, Cargo.toml, ARCHITECTURE.md (open decisions, now decided) and the kit map (KIT-A14-F16).
