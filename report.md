# Reference timings: the night of 2026-11-15

Commit `7693eaf38dcef2d8bcc6fbc49ea2eeb053ad03e1`, 2026-11-14 21:19 UTC to 2026-11-14 21:42 UTC, every job ran. Intel(R) Core(TM) i7-3770 CPU @ 3.40GHz on its performance cores (0, 1, 2, 3, 4, 5, 6, 7), Microsoft Windows 10 Pro 10.0.19045; rustc 1.99.0 (b940084d7 2026-09-28); rounds of 150 ms. Timings report and never block: G452 makes a confirmed miss block (32.42).

| Engine | Version | Timings | Failures | Disturbed | Not sampled |
| --- | --- | ---: | ---: | ---: | ---: |
| native | x86_64-pc-windows-gnullvm, llvm-mingw 20261020-ucrt-ubuntu-22.04-x86_64, statically linked | 1416 | 0 | 1 | 102 |

## The kernel's shares (23.1, accepted in 32.16)

| Share | Target | Limit | Engine | Threads | Benchmark | This night | Verdict | Last 7 nights |
| --- | --- | --- | --- | ---: | --- | ---: | --- | --- |
| open | Open the 230,000-element model to first view in under 3 s; a 20-building block in under 5 s | 1.00 s | native | 8 | `scenario/open/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| open_block | Open the 230,000-element model to first view in under 3 s; a 20-building block in under 5 s | 1.50 s | native | 8 | `scenario/open_block/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| door_move | Move a door in under 16 ms | 4.00 ms | native | 8 | `scenario/door_move/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| level_change | Change a level height touching 2,000 walls in under 1 s | 600 ms | native | 8 | `scenario/level_change/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| navigate | Navigate 1,000,000 instances at 60 fps | not a time: the kernel is never in the frame loop | native | 8 | `scenario/navigate/full` | - | not timed | - |
| plan_first | Plan view: first draw under 500 ms, under 16 ms after an edit | 250 ms | native | 8 | `scenario/plan_first/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| plan_edit | Plan view: first draw under 500 ms, under 16 ms after an edit | 4.00 ms | native | 8 | `scenario/plan_edit/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| instance_memory | Memory per placed instance: under 256 bytes plus shared type geometry | not a time: bytes, not time: G451's instance-memory scenario counts them | native | 8 | `scenario/instance_memory/full` | - | not timed | - |
| save | Save in under 100 ms, never blocking | not a time: saving appends to the operation log; encoding runs in background compaction, timed by save_local and save_level | native | 8 | `scenario/save/full` | - | not timed | - |
| save_local | Save in under 100 ms, never blocking | 10.0 ms | native | 8 | `scenario/save_local/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| save_level | Save in under 100 ms, never blocking | 100 ms | native | 8 | `scenario/save_level/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| replay | Another user sees an edit in under 200 ms on a LAN | 4.00 ms | native | 8 | `scenario/replay/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| print | Print a 100-sheet set to PDF in under 60 s | 40.0 s | native | 8 | `scenario/print/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| trivial_op | Computational design | 5.00 µs | native | 1 | `scenario/trivial_op/full` | - | not measured yet: no such benchmark (piece G451 adds it) | - |
| browser_door_move | In a browser | estimate: up to 4.00 ms | chrome | 1 | `scenario/door_move_small/full` | - | chrome not run | - |
| browser_door_move | In a browser | estimate: up to 4.00 ms | firefox | 1 | `scenario/door_move_small/full` | - | firefox not run | - |
| browser_door_move | In a browser | estimate: up to 4.00 ms | webkit | 1 | `scenario/door_move_small/full` | - | webkit not run | - |
| browser_level_change | In a browser | estimate: 1.00 s to 6.00 s | chrome | 1 | `scenario/level_change/full` | - | chrome not run | - |
| browser_level_change | In a browser | estimate: 1.00 s to 6.00 s | firefox | 1 | `scenario/level_change/full` | - | firefox not run | - |
| browser_level_change | In a browser | estimate: 1.00 s to 6.00 s | webkit | 1 | `scenario/level_change/full` | - | webkit not run | - |
| browser_open | In a browser | estimate: up to 1.00 s | chrome | 1 | `scenario/open/full` | - | chrome not run | - |
| browser_open | In a browser | estimate: up to 1.00 s | firefox | 1 | `scenario/open/full` | - | firefox not run | - |
| browser_open | In a browser | estimate: up to 1.00 s | webkit | 1 | `scenario/open/full` | - | webkit not run | - |
| browser_plan_first | In a browser | estimate: up to 250 ms | chrome | 1 | `scenario/plan_first/full` | - | chrome not run | - |
| browser_plan_first | In a browser | estimate: up to 250 ms | firefox | 1 | `scenario/plan_first/full` | - | firefox not run | - |
| browser_plan_first | In a browser | estimate: up to 250 ms | webkit | 1 | `scenario/plan_first/full` | - | webkit not run | - |
| browser_plan_edit | In a browser | estimate: up to 4.00 ms | chrome | 1 | `scenario/plan_edit/full` | - | chrome not run | - |
| browser_plan_edit | In a browser | estimate: up to 4.00 ms | firefox | 1 | `scenario/plan_edit/full` | - | firefox not run | - |
| browser_plan_edit | In a browser | estimate: up to 4.00 ms | webkit | 1 | `scenario/plan_edit/full` | - | webkit not run | - |

## Changes since the night before

This is the first night recorded.

## How each group scales

Medians over each group's benchmarks: the time per input at 8 threads over the time at 1, on each engine (1.00 is perfect scaling), and each browser's time at 1 thread over the native time.

| Group | native 8/1 |
| --- | --- |
| `attributes` | 2.43 |
| `ball` | 2.04 |
| `batch` | 2.03 |
| `body_record` | 2.11 |
| `brent` | 1.37 |
| `bvh` | 1.87 |
| `cbor` | 1.86 |
| `crossing` | 1.89 |
| `curve` | 1.96 |
| `dd` | 1.74 |
| `dual` | 1.63 |
| `eft` | 1.94 |
| `encoder` | 2.23 |
| `exact_sum` | 1.80 |
| `exactness` | 1.85 |
| `facade` | 2.06 |
| `format` | 2.27 |
| `generated` | 1.83 |
| `grid` | 1.85 |
| `growth` | 1.72 |
| `helper` | 2.38 |
| `history` | 2.29 |
| `interval` | 1.78 |
| `isolate` | 1.98 |
| `linalg` | 2.22 |
| `make` | 2.04 |
| `math` | 1.74 |
| `math_types` | 1.68 |
| `name_record` | 2.22 |
| `names` | 2.20 |
| `newton` | 1.88 |
| `nurbs` | 1.63 |
| `op_record` | 2.33 |
| `open` | 1.99 |
| `perturb` | 1.88 |
| `plane` | 1.88 |
| `predicate` | 1.88 |
| `prune` | 2.14 |
| `quadrature` | 1.80 |
| `query` | 1.92 |
| `real` | 1.89 |
| `record` | 1.96 |
| `resolve` | 1.97 |
| `rng` | 1.65 |
| `sign` | 1.81 |
| `surface` | 1.90 |
| `topo` | 2.40 |
| `unproven` | 1.69 |
| `wide` | 1.74 |
| `wide32` | 1.64 |

## Disturbed timings

Timings taken while the machine was busier than the job's own threads explain by more than 10 points: 1. They, and the timings no busy sample covers, are left out of the changes above.

| Engine | Threads | Benchmark | Time | Busy |
| --- | ---: | --- | ---: | ---: |
| native | 1 | `growth/wide_mul/l8` | 36.1 ns | 26% |

## Failures

Benchmarks not timed: 0.

