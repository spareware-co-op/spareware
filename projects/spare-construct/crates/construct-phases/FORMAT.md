# Phases and design options: field spellings, version 1

These are the plain values a record holds for phases and design options; [`src/phases.rs`](src/phases.rs), [`src/options.rs`](src/options.rs) and [`src/filter.rs`](src/filter.rs) read and write them. Like the rest of [construct-model's format](../construct-model/FORMAT.md), they are JSON, every ID is a kit ID, and reading checks what it reads. **We may stop writing an old version. We never stop reading it.**

## A project's phases

A JSON array, earliest phase first. The order is the meaning, so it is written and read in that order.

| Field | Meaning |
| --- | --- |
| `id` | The phase's permanent ID. It never changes, and no two phases share one |
| `name` | The name people see. Not empty, and no two phases share one |

## A project's design options

A JSON array of option sets in the order they were added.

| Field | Meaning |
| --- | --- |
| `id`, `name` | The set's permanent ID and its name |
| `options` | `{id, name}` for each option, in the order added. Never empty |
| `primary` | The ID of one of `options`: what a view shows for this set unless it chooses another |

An ID is used once among all sets and options.

## An element's staging fields

| Field | Meaning |
| --- | --- |
| `created` | The ID of the phase it was made in |
| `demolished` | The ID of the phase it was taken down in, or `null`. Never before `created` in the project's phase order; the same phase means temporary |
| `design_option` | `null` for the main model, otherwise `{set, option}`: the one option it belongs to |

Moving phases can break the order rule for stored elements, so the rule is checked again whenever a view asks.

## A view's settings for this rule

| Field | Meaning |
| --- | --- |
| `phase` | The ID of the phase the view shows |
| `filter` | One of `show_all`, `show_new`, `show_existing`, `show_demolished`, `show_existing_and_demolished`, `show_existing_and_new`, `show_new_and_demolished` |
| `choices` | `{set, option}` for each set the view picks an option of; a set not listed shows its primary |
