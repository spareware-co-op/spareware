# ezpz's test problems, in sketch v1

Each `.sketch` file here is one of the problems in ezpz's `test_cases` folder, written in `sketch v1`
by hand from its `problem.md`, and `massive_parallel_system` by a script that writes what upstream's
`gen_big_problem.py` writes for its 600 lines: the same entities, constraints, fixed values and
starting guesses, with every length at 1024 grid steps to ezpz's unit. `tests/solve.rs` solves them
all.

- Source: ezpz (Zoo), <https://github.com/KittyCAD/ezpz/tree/915882cc731da31042ce494c89b10334ee57837a/test_cases>
- Licence: MIT; the notice is `LICENSE` beside this file, exactly as upstream ships it at that commit,
  copyright line included.
- Changed for Spare Kit: the problems are rewritten in `sketch v1` and scaled to grid steps. No
  answers or code are taken.
