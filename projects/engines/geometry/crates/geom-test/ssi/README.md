# Intersection acceptance harness

Chapter 12.17's gate for surface/surface intersection, built by piece G160. The code is
`geom-test`'s `ssi` module; this folder holds what it runs with.

- `ceilings.json`: the most flagged regions, and the largest total size, any case of a class may
  have. A class 12.17 lets flag no region stands at zero, and the tool refuses anything else. The
  others stand at zero until the first method's run sets them, with its reason; after that they may
  only fall, or rise with a reason.
- `score.sh`: runs the corpus's own scorer in the container the nightly workflow pins by digest.
- `gates.sh`: holds what it scored to the gate.

The kernel has no surface/surface method yet, so the nightly run answers with stand-ins: the corpus's
truth, and the truth perturbed. That proves the harness, and says nothing about the kernel. When a
method arrives, it answers through `Output::of_intersection`, and the truth's lines in `gates.sh`
become the method's.

## Credit

The ssi-corpus is <https://github.com/jyviko/ssi-corpus>, MIT or Apache-2.0, fetched at a pinned
commit and checked against the corpus register's digest; it is not stored here. Its 25 `y23_*` cases
derive from Yang, Jia and Yan, "Topology Guaranteed B-Spline Surface/Surface Intersection", ACM
Transactions on Graphics 42(6), Article 211, 2023, CC BY 4.0. Every report carries this credit, and
each such case's `source` field.

The failure catalogue (`src/ssi/catalogue.rs`) is ours: new inputs and closed-form truth, rebuilt from
the failure's description alone. No fixture of any outside project is read.
