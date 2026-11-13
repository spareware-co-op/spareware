//! Adaptive subdivision and the parallel task tree: steps 1 and 4 of 11.3, around the local work.
//!
//! What it does: [`arrange`] takes the polygons of an arrangement and classifies every piece. A
//! box is split in two along its longest axis until it holds few polygons; polygons across the
//! split are clipped exactly, and the upper box's reference point is walked up from the lower's,
//! adding up the polygons it crosses, so each leaf can be solved on its own: [`local_cuts`], then
//! [`classify`]. A box whose every test is settled by its reference vector alone, whatever the
//! polygons in it do, is dropped (step 4). [`arrange_with`] is also told which entries are solids:
//! no two of their faces cross, and their winding is 1 just behind every face and 0 just in front,
//! EMBER's no-self-intersection and no-nested-parts flags (its section 4.5.1, 11.3 step 4). A box
//! whose polygons all bound one solid entry is a leaf at once, however many it holds, and is
//! solved without cuts or tracing.
//!
//! [`arrange_reusing`] does the same, taking each leaf's answer from an earlier call's [`Solved`]
//! where that call solved a box with exactly the same polygons, vector and bounds, and returns what
//! it solved for the next (11.6, the arrangement cache): a door moved in a wall is solved again
//! only in the boxes about its old and new places, since a box's splits depend on the box alone.
//! It keeps the tree of divided boxes too (G139): a box at the same place in the tree holding the
//! same polygons, under the same vector, divides as it did, so it takes its division, and every box
//! below it is that call's box again, divided and solved as it was, without clipping.
//!
//! Boxes split on odd half steps. Every polygon that comes from grid data lies on a whole step or
//! is tilted, so no polygon lies in a split plane, and every polygon ending on a piece reaches it
//! inside one leaf. A plane that does hold a polygon, such as a committed plane, is passed over for
//! the next odd half step.
//!
//! Guarantees:
//!
//! - **The same pieces whatever the threads.** Each level of boxes holding many polygons is one
//!   phase of the parallel helper; a box holding few, a fixed number, is then one task of another,
//!   its whole subtree, as is each leaf found on the way, in tree order. So the output is the same bits
//!   on any number of threads, and a failure the one a walk of those units in tree order meets
//!   first (23.4). [`arrange_reusing`] keeps each box's division and each leaf's answer by itself,
//!   so there every level is a phase and every leaf a task; the boxes, the pieces and the work
//!   are the same either way, as both divide and solve a box by the same steps.
//! - **No recursion.** The top levels are walked level by level and each subtree on an explicit
//!   stack, so the tree's depth never threatens a WebAssembly stack (principle 13); the kept tree
//!   is printed, compared and dropped on explicit stacks too. A box holding polygons that a split no longer separates,
//!   such as many faces on one plane, stops as a leaf instead of dividing down to the grid.
//! - **A reused box is the box solved again.** A leaf is keyed by SHA-256, under its own domain
//!   prefix, of everything its answer depends on, so a cache from any other inputs only misses:
//!   the pieces are the same bits as without it, and a reused leaf charges the work it cost, so
//!   the work count is the same too (tested across random edits).
//! - **Early outs are mechanical.** A box is dropped only when every test takes the same answer
//!   for every count of every entry with a polygon in it (11.3, step 4). A box with a cutter in it
//!   is kept, and with no tests nothing is dropped.
//! - **A solid's leaf is what tracing would give.** In a leaf whose polygons all bound one solid
//!   entry, no polygon crosses another, so each is one piece; the solid's count is 0 in front of
//!   each and 1 behind, and every other entry's is the reference's, as nothing of theirs is in
//!   the box. Each piece is the first of its plane, as no face of a solid overlaps another. Such a
//!   box is not split, so its pieces are fewer than tracing's leaves give, with the same vectors
//!   at every point (tested against the traced arrangement and the blocks' own counts). The caller
//!   vouches for the flag: a wrong one gives wrong pieces.
//!
//! Method: chapter 11.3's five steps (`projects/engines/geometry/architecture/11-booleans.md`),
//! after EMBER's sections 4.1, 4.2 and 4.5 (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>); Kleene's three-valued logic for the early outs,
//! in [`crate::winding`]; the phases of chapter 23.4.
//!
//! Recursion:
//!
//! - Types: `Child`, `Divided`, `Halves`, `Shape`.
//!   Bound: 127.
//!   Why: the kept tree nests one `Divided` per level of boxes, and a path down the tree splits
//!   at most 126 times: a box of the frame is at most 2^42 + 2 half steps wide on each axis, with
//!   its faces on odd half steps, a split leaves each part at most half the width plus one, and
//!   after 42 splits along an axis the width is 2, which no odd half step lies strictly inside.
//!   Test: `the_kept_tree_clones_compares_prints_and_drops_a_hundred_levels_deep`.
//! - Cycle: `<Divided as Drop>::drop`.
//!   Bound: 2.
//!   Why: the drop walks the tree below its box on a stack, and each box it holds last gives up
//!   its halves before it goes, so the drop that box starts finds no split, holds nothing and
//!   drops no other box.
//!   Test: `the_kept_tree_clones_compares_prints_and_drops_a_hundred_levels_deep`.
//!
//! Cost: O(n) clips per level for n polygons across the level's boxes, and at most 126 levels:
//! a box of the frame halves at most 42 times along each axis (the recursion statement above).
//! Each leaf costs what [`local_cuts`] and [`classify`] cost on its polygons, and a solid's leaf one
//! copy of each. A polygon's corner points are made once and carried into every box it enters
//! whole; only a split's two parts make theirs. Charges the meter one unit per polygon a split
//! clips, and one per polygon of a solid's leaf.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt;

use geom_num::grid::Coord;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_op::hash::Sha256;
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::convex::Piece;
use crate::cut::{Clipped, clip_at, local_cuts_with, meetings};
use crate::order::{Meeting, Span};
use crate::plane::canonical;
#[cfg(doc)]
use crate::trace::classify;
use crate::trace::{Cell, Classified, Polygon, TraceError, classify_with, reference_above};
use crate::winding::{Entry, Winding, WindingTest};

/// How many polygons a leaf holds at most, unless a split no longer separates them: the size at
/// which local cuts, quadratic in the polygons, cost about what another level of boxes would.
pub const LEAF_POLYGONS: usize = 32;

/// One box at a time: a box's cost varies too much for a chunk of them to mean anything.
const ONE_BOX: ChunkPlan = ChunkPlan::fixed::<1>();

/// The domain of a box's key: the bytes no other SHA-256 in the kernel starts with.
const BOX_KEY_V1: &[u8] = b"SKG arrangement box v1\0";

/// Every polygon's corner points, once.
const POINTS: Phase = Phase::new("arrangement: corner points");

/// The box around the polygons.
const AROUND: Phase = Phase::new("arrangement: the box around");

/// Polygons, a few hundred a task: a corner's rounding is a few dozen instructions.
const CORNER_PLAN: ChunkPlan = ChunkPlan::fixed::<256>();

/// One level of the tree.
const SPLITS: Phase = Phase::new("arrangement: split boxes");

/// The leaves, and the subtrees below the level-by-level splits.
const LEAVES: Phase = Phase::new("arrangement: solve leaves");

/// The polygons a box may hold and be one task, its whole subtree: a box holding more is split
/// level by level, one phase a level, so the heavy boxes are shared out and the deep levels cost
/// no phase of their own. A fixed count, so the work does not depend on the threads.
const SUBTREE_POLYGONS: usize = 128;

/// A box of the tree.
struct Node {
    /// Its place in tree order: the bits of its path from the top bit down, 1 for the upper box.
    key: u128,
    /// How many splits are above it.
    depth: u32,
    cell: Cell,
    /// The vector at its reference point.
    reference: Winding,
    /// Its polygons. A call that keeps its tree (G139) shares them with the tree, and with the
    /// earlier call's where the box is that call's; a call that keeps none is their only holder,
    /// and a split moves them into its halves.
    polygons: Arc<Vec<Polygon>>,
    /// The index in the input of each polygon.
    sources: Vec<usize>,
    /// Each polygon's corner points, as `crate::cut::meetings` makes them: made once, and kept
    /// by every box a polygon passes into whole. A box taken from an earlier call's tree has none:
    /// its answer is taken too, and [`solved_in`] makes them if ever it is not.
    points: Vec<Vec<Option<Meeting>>>,
    /// What a call that keeps its tree holds of the box; nothing in one that keeps none.
    kept: Trail,
}

/// A box's part in [`arrange_reusing`]'s walk (G139).
#[derive(Default)]
struct Trail {
    /// The earlier call's box at this place, if it had one.
    old: Option<Arc<Divided>>,
    /// Whether it is that box: the same box, vector and polygons, so its division is that box's.
    held: bool,
    /// Its place in the list of boxes this call divides, if it divides it: a box below a held one
    /// is the held box's, and is not listed again.
    slot: Option<usize>,
}

/// A divided box, kept for a later call (G139): the box, the vector at its reference point, the
/// polygons it held, what dividing it gave and the work that charged.
///
/// Printed, compared and dropped on explicit stacks, never a call a level, so a tree as deep as
/// its bound fits the WebAssembly stack (principle 13); its `Debug` text is the derived form's.
#[derive(Eq)]
struct Divided {
    cell: Cell,
    reference: Winding,
    polygons: Arc<Vec<Polygon>>,
    shape: Shape,
    work: u64,
}

/// What dividing a box gave.
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    /// Dropped: every test is settled in it.
    Dropped,
    /// A leaf, its answer kept under this key.
    Leaf([u8; 32]),
    /// Split: the lower box and the upper box.
    Split(Box<Halves>),
}

/// The two boxes a split makes, by name, so the recursion check reads their types.
#[derive(Debug, PartialEq, Eq)]
struct Halves {
    lower: Child,
    upper: Child,
}

/// A box split off its parent: its polygons' places in the parent's, and its division.
#[derive(Debug, PartialEq, Eq)]
struct Child {
    from: Vec<usize>,
    divided: Arc<Divided>,
}

impl PartialEq for Divided {
    /// Whether the two trees are the same, box by box: what `#[derive(PartialEq)]` answers, on a
    /// stack of the pairs of boxes still to compare.
    fn eq(&self, other: &Self) -> bool {
        let mut stack: Vec<(&Divided, &Divided)> = alloc::vec![(self, other)];
        while let Some((a, b)) = stack.pop() {
            if a.cell != b.cell
                || a.reference != b.reference
                || a.polygons != b.polygons
                || a.work != b.work
            {
                return false;
            }
            match (&a.shape, &b.shape) {
                (Shape::Dropped, Shape::Dropped) => {}
                (Shape::Leaf(a), Shape::Leaf(b)) if a == b => {}
                (Shape::Split(a), Shape::Split(b)) => {
                    for (a, b) in [(&a.lower, &b.lower), (&a.upper, &b.upper)] {
                        if a.from != b.from {
                            return false;
                        }
                        stack.push((&a.divided, &b.divided));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

impl Drop for Divided {
    /// Drops the tree below the box on a stack: a box this drop holds last gives up its halves
    /// before it goes, so the drop that box starts finds no split and walks nothing; a box held
    /// elsewhere too only loses this holder.
    fn drop(&mut self) {
        let mut shape = core::mem::replace(&mut self.shape, Shape::Dropped);
        let mut stack: Vec<Arc<Divided>> = Vec::new();
        loop {
            if let Shape::Split(halves) = shape {
                let Halves { lower, upper } = *halves;
                stack.push(lower.divided);
                stack.push(upper.divided);
            }
            let Some(next) = stack.pop() else {
                return;
            };
            shape = match Arc::into_inner(next) {
                Some(mut last) => core::mem::replace(&mut last.shape, Shape::Dropped),
                None => Shape::Dropped,
            };
        }
    }
}

impl fmt::Debug for Divided {
    /// The text `#[derive(Debug)]` writes for the tree, plain or pretty (`{:#?}`), written from a
    /// stack of its parts. The plain form passes the formatter to each field outside the tree, as
    /// the derived form does, so every flag is kept; the pretty form prints those fields with
    /// `{:#?}`, as no stable way gives a field the formatter's other flags under an indent.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printer = Printer {
            pretty: f.alternate(),
            f,
            depth: 0,
            on_newline: false,
            open: Vec::new(),
        };
        let mut stack = alloc::vec![Part::Box(self)];
        while let Some(part) = stack.pop() {
            printer.put(part, &mut stack)?;
        }
        Ok(())
    }
}

/// A part of a tree's `Debug` text, in the order the derived form writes it.
#[derive(Clone, Copy)]
enum Part<'a> {
    /// A box, which stands for its own parts.
    Box(&'a Divided),
    /// A struct's or a tuple variant's name, which opens it.
    Open(&'static str),
    /// The start of a struct's field of this name.
    Field(&'static str),
    /// The start of a tuple variant's field.
    Item,
    /// The end of a field.
    End,
    /// The end of a struct.
    CloseStruct,
    /// The end of a tuple variant.
    CloseTuple,
    /// A variant without fields.
    Name(&'static str),
    /// A field whose type is not the tree's.
    Value(Value<'a>),
}

/// A field of a box whose type is not the tree's, printed by its own `Debug`.
#[derive(Clone, Copy)]
enum Value<'a> {
    Cell(&'a Cell),
    Reference(&'a Winding),
    Polygons(&'a [Polygon]),
    Key(&'a [u8; 32]),
    From(&'a [usize]),
    Work(u64),
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Cell(v) => fmt::Debug::fmt(v, f),
            Value::Reference(v) => fmt::Debug::fmt(v, f),
            Value::Polygons(v) => fmt::Debug::fmt(v, f),
            Value::Key(v) => fmt::Debug::fmt(v, f),
            Value::From(v) => fmt::Debug::fmt(v, f),
            Value::Work(v) => fmt::Debug::fmt(v, f),
        }
    }
}

/// The parts of box `d`, in order: its children stand as [`Part::Box`].
fn parts(d: &Divided) -> Vec<Part<'_>> {
    let mut out = alloc::vec![
        Part::Open("Divided"),
        Part::Field("cell"),
        Part::Value(Value::Cell(&d.cell)),
        Part::End,
        Part::Field("reference"),
        Part::Value(Value::Reference(&d.reference)),
        Part::End,
        Part::Field("polygons"),
        Part::Value(Value::Polygons(d.polygons.as_slice())),
        Part::End,
        Part::Field("shape"),
    ];
    match &d.shape {
        Shape::Dropped => out.push(Part::Name("Dropped")),
        Shape::Leaf(key) => out.extend([
            Part::Open("Leaf"),
            Part::Item,
            Part::Value(Value::Key(key)),
            Part::End,
            Part::CloseTuple,
        ]),
        Shape::Split(halves) => {
            out.extend([Part::Open("Split"), Part::Item, Part::Open("Halves")]);
            for (name, child) in [("lower", &halves.lower), ("upper", &halves.upper)] {
                out.extend([
                    Part::Field(name),
                    Part::Open("Child"),
                    Part::Field("from"),
                    Part::Value(Value::From(child.from.as_slice())),
                    Part::End,
                    Part::Field("divided"),
                    Part::Box(&child.divided),
                    Part::End,
                    Part::CloseStruct,
                    Part::End,
                ]);
            }
            out.extend([Part::CloseStruct, Part::End, Part::CloseTuple]);
        }
    }
    out.extend([
        Part::End,
        Part::Field("work"),
        Part::Value(Value::Work(d.work)),
        Part::End,
        Part::CloseStruct,
    ]);
    out
}

/// Spaces for the pretty form's indent, written a run at a time.
const SPACES: &str = "                                                                ";

/// Writes a tree's parts as the formatting builders of `core::fmt` do (`debug_struct`,
/// `debug_tuple`): the plain form `Name { a: 1, b: 2 }` and `Name(x)`, the pretty form one field
/// a line, each line four spaces in for every field it is inside.
struct Printer<'f, 'g> {
    f: &'f mut fmt::Formatter<'g>,
    pretty: bool,
    /// How many fields the text is inside, below the box printed: the pretty form only.
    depth: usize,
    /// Whether the last text written ended a line.
    on_newline: bool,
    /// For each struct or tuple variant open, whether it has a field yet.
    open: Vec<bool>,
}

impl Printer<'_, '_> {
    /// Writes `part`, or puts a box's parts on `stack`, the first on top.
    fn put<'a>(&mut self, part: Part<'a>, stack: &mut Vec<Part<'a>>) -> fmt::Result {
        match part {
            Part::Box(d) => {
                stack.extend(parts(d).into_iter().rev());
                Ok(())
            }
            Part::Open(name) => {
                self.open.push(false);
                self.text(name)
            }
            Part::Field(name) => {
                self.begin(" {\n", " { ")?;
                self.text(name)?;
                self.text(": ")
            }
            Part::Item => self.begin("(\n", "("),
            Part::End if self.pretty => {
                self.text(",\n")?;
                self.depth = self.depth.saturating_sub(1);
                Ok(())
            }
            Part::End => Ok(()),
            Part::CloseStruct => match self.open.pop() {
                Some(true) => self.text(if self.pretty { "}" } else { " }" }),
                _ => Ok(()),
            },
            Part::CloseTuple => match self.open.pop() {
                Some(true) => self.text(")"),
                _ => Ok(()),
            },
            Part::Name(name) => self.text(name),
            // Made whole before it is indented: a formatter that wrote into the printer itself
            // would make its writing a cycle through `core::fmt`.
            Part::Value(v) if self.pretty => self.text(&alloc::format!("{v:#?}")),
            Part::Value(v) => fmt::Debug::fmt(&v, self.f),
        }
    }

    /// The start of a field: `pretty` before the first in the pretty form, then one field deeper;
    /// `plain` before the first in the plain form, and a comma before each other.
    fn begin(&mut self, pretty: &str, plain: &str) -> fmt::Result {
        let first = self
            .open
            .last_mut()
            .is_none_or(|has| !core::mem::replace(has, true));
        if self.pretty {
            if first {
                self.text(pretty)?;
            }
            self.depth = self.depth.saturating_add(1);
            Ok(())
        } else {
            self.text(if first { plain } else { ", " })
        }
    }

    /// Writes `s`, each line that starts in it indented to the depth.
    fn text(&mut self, s: &str) -> fmt::Result {
        let mut rest = s;
        while !rest.is_empty() {
            // A line runs to just past its newline, or to the end; a newline is one byte, so the
            // split is on a character boundary.
            let newline = rest.as_bytes().iter().position(|b| *b == b'\n');
            let end = newline.map_or(rest.len(), |at| at.saturating_add(1));
            let (line, after) = rest.split_at(end);
            if self.on_newline {
                let mut left = self.depth.saturating_mul(4);
                while left > 0 {
                    let run = left.min(SPACES.len());
                    self.f.write_str(SPACES.get(..run).unwrap_or_default())?;
                    left = left.saturating_sub(run);
                }
            }
            self.on_newline = newline.is_some();
            self.f.write_str(line)?;
            rest = after;
        }
        Ok(())
    }
}

/// A box this call divides, until the tree is built: what [`Divided`] holds, its shape waiting on
/// its children's and its key.
struct Pending {
    cell: Cell,
    reference: Winding,
    polygons: Arc<Vec<Polygon>>,
    work: u64,
    shape: PendingShape,
}

/// A [`Pending`] box's shape: a held box's division, or one this call made.
enum PendingShape {
    Held(Arc<Divided>),
    Dropped,
    /// A leaf, its key found when the leaves are solved.
    Leaf(Option<[u8; 32]>),
    /// Split: each child's polygons' places, and its place in the list.
    Split([(Vec<usize>, usize); 2]),
}

/// A box's answer, with its polygons numbered within the box, and the work it took.
#[derive(Debug, PartialEq, Eq)]
struct Stored {
    pieces: Vec<Classified>,
    work: u64,
}

/// Solved boxes, by the hash of everything each answer depends on, for [`arrange_reusing`]. A box a
/// change did not touch hashes the same and is not solved again. An entry that matches nothing
/// is ignored, so a cache from other inputs costs nothing and changes nothing. With them, the tree
/// of divided boxes, under the leaf size and tests it was divided for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Solved {
    boxes: BTreeMap<[u8; 32], Arc<Stored>>,
    tree: Option<Arc<Divided>>,
    leaf: usize,
    tests: Vec<WindingTest>,
}

impl Solved {
    /// How many boxes it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// How many of its boxes `other` holds too.
    #[must_use]
    pub fn shared_with(&self, other: &Solved) -> usize {
        self.boxes
            .keys()
            .filter(|key| other.boxes.contains_key(*key))
            .count()
    }

    /// How many boxes of its tree are `other`'s, the very divisions and not copies: the boxes a
    /// call took from the other's tree without dividing them again.
    #[must_use]
    pub fn divisions_shared_with(&self, other: &Solved) -> usize {
        let mut count = 0usize;
        let mut stack: Vec<(&Arc<Divided>, Option<&Arc<Divided>>)> = Vec::new();
        if let Some(mine) = &self.tree {
            stack.push((mine, other.tree.as_ref()));
        }
        while let Some((mine, theirs)) = stack.pop() {
            let same = theirs.is_some_and(|theirs| Arc::ptr_eq(mine, theirs));
            count = count.saturating_add(usize::from(same));
            if let Shape::Split(children) = &mine.shape {
                let theirs = match theirs.map(|t| &t.shape) {
                    Some(Shape::Split(c)) => [Some(&c.lower.divided), Some(&c.upper.divided)],
                    _ => [None, None],
                };
                stack.push((&children.lower.divided, theirs[0]));
                stack.push((&children.upper.divided, theirs[1]));
            }
        }
        count
    }
}

/// What a level does to a box.
enum Outcome {
    /// Every test is settled in it.
    Dropped,
    /// It stays whole.
    Leaf,
    /// It splits as the plan says.
    Split(Box<Plan>),
}

/// What a level of [`arrange_reusing`] did to a box: took a held box's division, or divided it at
/// this work.
enum Step {
    Held,
    Fresh(Outcome, u64),
}

/// How a box splits: found on the pool from the box borrowed, then carried out on the box itself,
/// so that a polygon that goes whole into one half is moved there, with its points, not copied.
struct Plan {
    /// The axis the split is across, and the half step it is at.
    axis: usize,
    at: i64,
    /// The vector at the upper box's reference point.
    reference: Winding,
    /// Where each polygon goes, in the box's order.
    places: Vec<Place>,
}

impl Plan {
    /// Each half's polygons' places in the box's, the lower half's and then the upper's, in the
    /// order [`put`] fills the halves: what a kept tree holds of a split (G139).
    fn origins(&self) -> [Vec<usize>; 2] {
        let (mut below, mut above) = (Vec::new(), Vec::new());
        for (at, place) in self.places.iter().enumerate() {
            match place {
                Place::Above => above.push(at),
                Place::Below => below.push(at),
                Place::Split(_) => {
                    above.push(at);
                    below.push(at);
                }
            }
        }
        [below, above]
    }
}

/// Where a split puts one polygon.
enum Place {
    /// Whole into the upper box.
    Above,
    /// Whole into the lower box.
    Below,
    /// Its part above, with that part's points, and its part below, with its points.
    Split(Box<[(Polygon, Vec<Option<Meeting>>); 2]>),
}

/// The polygons, sources and points of one half, in the box's order.
type Half = (Vec<Polygon>, Vec<usize>, Vec<Vec<Option<Meeting>>>);

/// Put one polygon of a split where `place` says: `whole` gives the polygon and its points, read
/// only where it goes whole.
fn put(
    (below, above): (&mut Half, &mut Half),
    (place, source): (Place, usize),
    whole: impl FnOnce() -> (Polygon, Vec<Option<Meeting>>),
) {
    let push = |half: &mut Half, (polygon, points): (Polygon, Vec<Option<Meeting>>)| {
        half.0.push(polygon);
        half.1.push(source);
        half.2.push(points);
    };
    match place {
        Place::Above => push(above, whole()),
        Place::Below => push(below, whole()),
        Place::Split(parts) => {
            let [up, down] = *parts;
            push(above, up);
            push(below, down);
        }
    }
}

/// The lower box and the upper box of `node`'s split by `plan`, each polygon moved into its half:
/// the box is its polygons' only holder in a call that keeps no tree, and where it is not they
/// are copied.
fn split(node: Node, plan: Plan) -> (Node, Node) {
    let head = (node.key, node.depth, node.cell, node.reference);
    let (mut below, mut above) = (Half::default(), Half::default());
    let polygons = Arc::try_unwrap(node.polygons).unwrap_or_else(|shared| shared.to_vec());
    let each = polygons.into_iter().zip(node.sources).zip(node.points);
    for (((polygon, source), points), place) in each.zip(plan.places) {
        put((&mut below, &mut above), (place, source), || {
            (polygon, points)
        });
    }
    halves(head, (plan.axis, plan.at, plan.reference), (below, above))
}

/// [`split`] of a box that is only borrowed: a polygon that goes whole is copied.
fn split_copied(node: &Node, plan: Plan) -> (Node, Node) {
    let head = (node.key, node.depth, node.cell, node.reference.clone());
    let (mut below, mut above) = (Half::default(), Half::default());
    let each = node.polygons.iter().zip(&node.sources).zip(&node.points);
    for (((polygon, source), points), place) in each.zip(plan.places) {
        put((&mut below, &mut above), (place, *source), || {
            (polygon.clone(), points.clone())
        });
    }
    halves(head, (plan.axis, plan.at, plan.reference), (below, above))
}

/// The two boxes of a split at `at` across `axis` of the box `head` (its key, depth, cell and
/// vector), holding `below` and `above`; the upper box's vector is `reference`.
fn halves(
    (key, depth, cell, lower): (u128, u32, Cell, Winding),
    (axis, at, reference): (usize, i64, Winding),
    (below, above): (Half, Half),
) -> (Node, Node) {
    let moved = |corner: [i64; 3]| {
        let mut i = 0..;
        corner.map(|v| if i.next() == Some(axis) { at } else { v })
    };
    let under = Cell {
        low: cell.low,
        high: moved(cell.high),
    };
    let over = Cell {
        low: moved(cell.low),
        high: cell.high,
    };
    let bit = 1u128.checked_shl(127u32.saturating_sub(depth)).unwrap_or(0);
    let depth = depth.saturating_add(1);
    (
        Node {
            key,
            depth,
            cell: under,
            reference: lower,
            polygons: Arc::new(below.0),
            sources: below.1,
            points: below.2,
            kept: Trail::default(),
        },
        Node {
            key: key | bit,
            depth,
            cell: over,
            reference,
            polygons: Arc::new(above.0),
            sources: above.1,
            points: above.2,
            kept: Trail::default(),
        },
    )
}

/// The box around `polygons`, in half steps, a step clear of them with its faces on odd half
/// steps, so that no polygon lies in one.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a piece whose adjacent bounds do not meet, [`NumError::OutOfRange`]
/// for a corner outside the frame.
pub fn around(polygons: &[Polygon]) -> Result<Cell, NumError> {
    let mut ranges = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        ranges.push(range_of(polygon)?);
    }
    Ok(cell_around(&ranges))
}

/// [`around`] on `exec`'s pool: each polygon's corners' range in parallel chunks, then the ranges
/// together, so the same box.
///
/// # Errors
///
/// As [`around`], as a [`RunError`] of a [`TraceError`].
pub fn around_on(
    exec: &Exec<'_>,
    polygons: &[Polygon],
    meter: &mut WorkMeter<'_>,
) -> Result<Cell, RunError<TraceError>> {
    let ranges = exec.try_map(meter, AROUND, CORNER_PLAN, polygons, |_, polygon, _| {
        range_of(polygon).map_err(TraceError::from)
    })?;
    Ok(cell_around(&ranges))
}

/// The grid range of `polygon`'s corners, each the point [`nearest`] gives.
fn range_of(polygon: &Polygon) -> Result<([i64; 3], [i64; 3]), NumError> {
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    let Piece { face, bounds } = &polygon.piece;
    let after = bounds.iter().cycle().skip(1);
    for (bound, next) in bounds.iter().zip(after) {
        let coordinates = nearest(face, bound, next)?;
        for ((l, h), v) in low.iter_mut().zip(&mut high).zip(coordinates) {
            *l = (*l).min(v);
            *h = (*h).max(v);
        }
    }
    Ok((low, high))
}

/// The box around the corner ranges, in half steps, a step clear with its faces on odd half steps.
fn cell_around(ranges: &[([i64; 3], [i64; 3])]) -> Cell {
    if ranges.is_empty() {
        return Cell {
            low: [-1; 3],
            high: [1; 3],
        };
    }
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for (l, h) in ranges {
        for ((low, high), (l, h)) in low.iter_mut().zip(&mut high).zip(l.iter().zip(h)) {
            *low = (*low).min(*l);
            *high = (*high).max(*h);
        }
    }
    let half = |v: i64, by: i64| v.saturating_mul(2).saturating_add(by);
    Cell {
        low: low.map(|v| half(v, -3)),
        high: high.map(|v| half(v, 3)),
    }
}

/// The grid point nearest where `face`, `bound` and `next` meet, as `Tpi::to_grid` gives it, in
/// machine words where the point fits them, and refused as it refuses.
fn nearest(face: &Plane, bound: &Plane, next: &Plane) -> Result<[i64; 3], NumError> {
    let words = Meeting::new(face, bound, next)
        .filter(|point| point.weight() != Sign::Zero)
        .and_then(|point| point.rounded());
    if let Some([x, y, z]) = words {
        return Ok([
            Coord::from_i128(x)?.get(),
            Coord::from_i128(y)?.get(),
            Coord::from_i128(z)?.get(),
        ]);
    }
    let at = Tpi::meeting(*face, *bound, *next)?.to_grid()?.value();
    Ok([at.x.get(), at.y.get(), at.z.get()])
}

/// The odd half steps to try splitting `cell` at, as (axis, half step), the longest axis first.
fn splits(cell: &Cell) -> [(usize, Option<i64>); 3] {
    let at = |axis: usize| {
        let (low, high) = (
            cell.low.get(axis).copied().unwrap_or(0),
            cell.high.get(axis).copied().unwrap_or(0),
        );
        let middle = low.saturating_add(high.saturating_sub(low).saturating_div(2));
        let odd = middle | 1;
        (low < odd && odd < high).then_some(odd)
    };
    let width = |axis: usize| {
        let (low, high) = (cell.low.get(axis), cell.high.get(axis));
        low.zip(high).map_or(0, |(l, h)| h.saturating_sub(*l))
    };
    let mut axes = [0, 1, 2];
    axes.sort_by_key(|axis| Reverse(width(*axis)));
    axes.map(|axis| (axis, at(axis)))
}

/// Whether every test is settled in `node`, whatever its polygons do.
fn settled(node: &Node, tests: &[WindingTest]) -> bool {
    if tests.is_empty() || node.polygons.iter().any(|p| p.entry.is_none()) {
        return false;
    }
    let mut open: Vec<Entry> = node.polygons.iter().filter_map(|p| p.entry).collect();
    open.sort_unstable();
    open.dedup();
    tests
        .iter()
        .all(|test| test.decided(&node.reference, &open).is_some())
}

/// The solid entry every polygon of `node` bounds, if there is one.
fn one_solid(node: &Node, solids: &[Entry]) -> Option<Entry> {
    let only = node.polygons.first().and_then(|p| p.entry)?;
    (solids.contains(&only) && node.polygons.iter().all(|p| p.entry == Some(only))).then_some(only)
}

/// What this level does to `node`. A box whose polygons all bound one solid is a leaf at once:
/// solving it costs one copy of each polygon, so splitting it only makes more pieces to merge.
fn divide(
    node: &Node,
    (leaf, solids): (usize, &[Entry]),
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Outcome, TraceError> {
    if settled(node, tests) {
        return Ok(Outcome::Dropped);
    }
    let count = node.polygons.len();
    if count <= leaf || one_solid(node, solids).is_some() {
        return Ok(Outcome::Leaf);
    }
    // Faces that span every split are not separated by more of them.
    let enough = count
        .saturating_mul(2)
        .saturating_sub(count.saturating_div(8));
    for (axis, at) in splits(&node.cell) {
        let Some(at) = at else {
            continue;
        };
        let n = [0, 1, 2].map(|i| if i == axis { 2 } else { 0 });
        let plane = Plane::new(n, i128::from(at).saturating_neg())?;
        // Where each polygon would go; nothing is copied until the split is taken.
        let mut places = Vec::with_capacity(count);
        let mut parts = 0usize;
        let mut in_plane = false;
        for (polygon, points) in node.polygons.iter().zip(&node.points) {
            meter.charge(1)?;
            let part = |piece: Piece| {
                let points = meetings(&piece);
                let polygon = Polygon {
                    piece,
                    entry: polygon.entry,
                };
                (polygon, points)
            };
            let goes = match clip_at(&polygon.piece, points, &plane)? {
                Clipped::Front => Place::Above,
                Clipped::Back => Place::Below,
                Clipped::Split(front, back) => {
                    parts = parts.saturating_add(1);
                    Place::Split(Box::new([part(front), part(back)]))
                }
                Clipped::On => {
                    in_plane = true;
                    break;
                }
            };
            places.push(goes);
        }
        // The two halves hold every polygon once, and each split one twice.
        if in_plane || places.len().saturating_add(parts) >= enough {
            continue;
        }
        let reference =
            reference_above(&node.cell, axis, at, &node.reference, &node.polygons, meter)?;
        return Ok(Outcome::Split(Box::new(Plan {
            axis,
            at,
            reference,
            places,
        })));
    }
    Ok(Outcome::Leaf)
}

/// A leaf's pieces, each naming its polygon in the input.
fn solve(
    node: &Node,
    solids: &[Entry],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    let mut classified = solved_in(node, solids, meter)?;
    for piece in &mut classified {
        piece.polygon = node
            .sources
            .get(piece.polygon)
            .copied()
            .unwrap_or(piece.polygon);
    }
    Ok(classified)
}

/// A leaf's pieces, each naming its polygon by its place in the box: what a kept answer holds
/// (G139), as the same box's polygons lie at other places in another call's input.
fn solved_in(
    node: &Node,
    solids: &[Entry],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    if let Some(entry) = one_solid(node, solids) {
        return solid_leaf(node, entry, meter);
    }
    let pieces: Vec<Piece> = node.polygons.iter().map(|p| p.piece.clone()).collect();
    // A box taken from an earlier call's tree carries no points, as its answer is taken with it:
    // they are made here only if that answer is not there to take.
    let made: Vec<Vec<Option<Meeting>>>;
    let points = if node.points.len() == pieces.len() {
        &node.points
    } else {
        made = pieces.iter().map(meetings).collect();
        &made
    };
    // Each polygon's face in canonical form, and its span from its corners' points, which the
    // node holds, made once for the local cuts and the classifier both: the points are made once
    // and kept down the tree, and neither finds them again.
    let mut held = Vec::with_capacity(node.polygons.len());
    let mut planes = Vec::with_capacity(node.polygons.len());
    let mut spans = Vec::with_capacity(node.polygons.len());
    for (at, polygon) in node.polygons.iter().enumerate() {
        let span = points
            .get(at)
            .and_then(|points| Span::of(points.iter().copied()));
        let face = canonical(&polygon.piece.face)?;
        planes.push(face.plane);
        spans.push(span);
        held.push((polygon, face, span));
    }
    let cuts = local_cuts_with(&pieces, points, (&planes, &spans), meter)?;
    classify_with(&node.cell, &node.reference, &held, &cuts, meter)
}

/// What a box's answer depends on, hashed under [`BOX_KEY_V1`]: the box, the vector at its
/// reference point, and each polygon's entry, plane and bounds, in order. Every count is written as
/// eight bytes before what it counts, so two inputs never write the same bytes, and the key is the
/// same on every target.
fn key_of(node: &Node) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.write(BOX_KEY_V1);
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX).to_le_bytes();
    let plane = |p: &Plane, hash: &mut Sha256| {
        for v in p.n() {
            hash.write(&v.to_le_bytes());
        }
        hash.write(&p.d().to_le_bytes());
    };
    for v in node.cell.low.iter().chain(&node.cell.high) {
        hash.write(&v.to_le_bytes());
    }
    hash.write(&count(node.reference.pairs().len()));
    for (entry, at) in node.reference.pairs() {
        hash.write(&entry.to_le_bytes());
        hash.write(&at.to_le_bytes());
    }
    hash.write(&count(node.polygons.len()));
    for polygon in node.polygons.iter() {
        hash.write(&polygon.entry.map_or(u64::MAX, u64::from).to_le_bytes());
        plane(&polygon.piece.face, &mut hash);
        hash.write(&count(polygon.piece.bounds.len()));
        for bound in &polygon.piece.bounds {
            plane(bound, &mut hash);
        }
    }
    hash.finish()
}

/// The answer of `node`, from `reuse` if it holds one for exactly this box, at the work it cost.
/// [`arrange_reusing`] takes no solids, so a kept answer is always a traced one.
fn answer(
    node: &Node,
    reuse: Option<&Solved>,
    meter: &mut WorkMeter<'_>,
) -> Result<([u8; 32], Arc<Stored>), TraceError> {
    let held = node.kept.old.as_ref().filter(|_| node.kept.held);
    let key = match held.map(|old| &old.shape) {
        Some(Shape::Leaf(key)) => *key,
        _ => key_of(node),
    };
    if let Some(held) = reuse.and_then(|solved| solved.boxes.get(&key)) {
        meter.charge(held.work)?;
        return Ok((key, Arc::clone(held)));
    }
    let before = meter.used().work;
    let pieces = solved_in(node, &[], meter)?;
    // A box's work is what its charges added, each a u64 the meter took: past u64 it would have
    // stopped first.
    let work = u64::try_from(meter.used().work.saturating_sub(before)).unwrap_or(u64::MAX);
    Ok((key, Arc::new(Stored { pieces, work })))
}

/// The pieces of a leaf whose polygons all bound the solid `entry`, each naming its polygon by
/// its place in the box: each polygon whole, with the reference's vector, the solid's count 0 in
/// front and 1 behind.
fn solid_leaf(
    node: &Node,
    entry: Entry,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    let mut front = node.reference.clone();
    front.add(entry, front.count(entry).saturating_neg())?;
    let mut back = front.clone();
    back.add(entry, 1)?;
    let mut out = Vec::with_capacity(node.polygons.len());
    for (at, polygon) in node.polygons.iter().enumerate() {
        meter.charge(1)?;
        out.push(Classified {
            polygon: at,
            piece: polygon.piece.clone(),
            front: front.clone(),
            back: back.clone(),
            first: true,
        });
    }
    Ok(out)
}
/// Every piece of `polygons` with its vectors, in tree order, from the box `cell` whose reference
/// vector is `reference`. A box holds at most `leaf` polygons unless splits no longer separate
/// them, [`LEAF_POLYGONS`] as a rule. With `tests`, a box they all settle is dropped.
///
/// # Errors
///
/// A [`RunError`] of a [`TraceError`]: the lowest-numbered failure of the level or the leaves, or a
/// budget that ran out.
pub fn arrange(
    exec: &Exec<'_>,
    cell: Cell,
    reference: Winding,
    polygons: &[Polygon],
    leaf: usize,
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, RunError<TraceError>> {
    arrange_with(exec, (cell, reference), polygons, (leaf, tests), &[], meter)
}

/// [`arrange`], told that the entries `solids` are solids: no two of their faces cross, and their
/// winding is 1 just behind every face and 0 just in front. A box whose polygons all bound one of
/// them is a leaf, solved without cuts or tracing: fewer pieces, with the same vectors at every
/// point.
///
/// # Errors
///
/// As [`arrange`].
pub fn arrange_with(
    exec: &Exec<'_>,
    (cell, reference): (Cell, Winding),
    polygons: &[Polygon],
    (leaf, tests): (usize, &[WindingTest]),
    solids: &[Entry],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, RunError<TraceError>> {
    let points = exec.try_map(meter, POINTS, CORNER_PLAN, polygons, |_, polygon, _| {
        Ok::<_, TraceError>(meetings(&polygon.piece))
    })?;
    let root = alloc::vec![Node {
        key: 0,
        depth: 0,
        cell,
        reference,
        polygons: Arc::new(polygons.to_vec()),
        sources: (0..polygons.len()).collect(),
        points,
        kept: Trail::default(),
    }];
    // Level by level while a box holds many polygons; a leaf found on the way, or a box small
    // enough, is then one task, its whole subtree.
    let mut units: Vec<(Node, bool)> = Vec::new();
    let small = |node: &Node| node.polygons.len() <= SUBTREE_POLYGONS;
    let (held, mut level): (Vec<Node>, Vec<Node>) = root.into_iter().partition(|n| small(n));
    units.extend(held.into_iter().map(|node| (node, false)));
    while !level.is_empty() {
        let outcomes = exec.try_map(meter, SPLITS, ONE_BOX, &level, |_, node, m| {
            divide(node, (leaf, solids), tests, m)
        })?;
        let mut next = Vec::new();
        for (node, outcome) in level.into_iter().zip(outcomes) {
            match outcome {
                Outcome::Dropped => {}
                Outcome::Leaf => units.push((node, true)),
                Outcome::Split(plan) => {
                    let (under, over) = split(node, *plan);
                    for child in [under, over] {
                        if small(&child) {
                            units.push((child, false));
                        } else {
                            next.push(child);
                        }
                    }
                }
            }
        }
        level = next;
    }
    // A subtree's leaves lie between its box's key and the next unit's, so this is tree order.
    units.sort_unstable_by_key(|(node, _)| node.key);
    let solved = exec.try_map(meter, LEAVES, ONE_BOX, &units, |_, (node, whole), m| {
        if *whole {
            solve(node, solids, m)
        } else {
            subtree(node, (leaf, solids), tests, m)
        }
    })?;
    Ok(solved.into_iter().flatten().collect())
}

/// [`arrange`], taking each box's answer from `reuse` where it holds one for exactly that box, and
/// returning what it solved for the next call. The answer and the work charged are the same as
/// without it, bit for bit: a reused box charges what it cost. The boxes, their pieces and the
/// work are [`arrange`]'s too, each box divided and each leaf solved by the same steps; but as
/// each box's division and each leaf's answer is kept by itself, every level here is a phase and
/// every leaf a task, however few polygons a box holds.
///
/// # Errors
///
/// As [`arrange`].
#[allow(
    clippy::too_many_arguments,
    reason = "arrange's arguments and the one thing it adds to them"
)]
pub fn arrange_reusing(
    exec: &Exec<'_>,
    cell: Cell,
    reference: Winding,
    polygons: &[Polygon],
    leaf: usize,
    tests: &[WindingTest],
    reuse: Option<&Solved>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Classified>, Solved), RunError<TraceError>> {
    let tree = reuse
        .filter(|old| old.leaf == leaf && old.tests == tests)
        .and_then(|old| old.tree.clone());
    let points = exec.try_map(meter, POINTS, CORNER_PLAN, polygons, |_, polygon, _| {
        Ok::<_, TraceError>(meetings(&polygon.piece))
    })?;
    let mut level = alloc::vec![Node {
        key: 0,
        depth: 0,
        cell,
        reference,
        polygons: Arc::new(polygons.to_vec()),
        sources: (0..polygons.len()).collect(),
        points,
        kept: Trail {
            old: tree,
            held: false,
            slot: Some(0),
        },
    }];
    let mut pending: Vec<Option<Pending>> = alloc::vec![None];
    let mut leaves = Vec::new();
    while !level.is_empty() {
        for node in &mut level {
            held_if_same(node);
        }
        let steps = exec.try_map(meter, SPLITS, ONE_BOX, &level, |_, node, m| {
            step(node, leaf, tests, m)
        })?;
        let mut next = Vec::new();
        for (node, step) in level.into_iter().zip(steps) {
            next_level(node, step, &mut pending, &mut leaves, &mut next);
        }
        level = next;
    }
    leaves.sort_unstable_by_key(|node| node.key);
    let solved = exec.try_map(meter, LEAVES, ONE_BOX, &leaves, |_, node, m| {
        answer(node, reuse, m)
    })?;
    let mut out = Vec::new();
    let mut kept = Solved {
        leaf,
        tests: tests.to_vec(),
        ..Solved::default()
    };
    for (node, (key, stored)) in leaves.iter().zip(solved) {
        out.extend(stored.pieces.iter().cloned().map(|mut piece| {
            let source = node.sources.get(piece.polygon);
            piece.polygon = source.copied().unwrap_or(piece.polygon);
            piece
        }));
        kept.boxes.insert(key, stored);
        let slot = node
            .kept
            .slot
            .and_then(|at| pending.get_mut(at))
            .and_then(Option::as_mut);
        if let Some(Pending {
            shape: PendingShape::Leaf(found),
            ..
        }) = slot
        {
            *found = Some(key);
        }
    }
    kept.tree = built(pending);
    Ok((out, kept))
}

/// Marks `node` held when the earlier call's box at its place has the same bounds, vector and
/// polygons, and takes that box's polygons, so both calls share them.
fn held_if_same(node: &mut Node) {
    if node.kept.held {
        return;
    }
    if let Some(old) = &node.kept.old
        && old.cell == node.cell
        && old.reference == node.reference
        && old.polygons == node.polygons
    {
        node.polygons = Arc::clone(&old.polygons);
        node.kept.held = true;
    }
}

/// What this level does to `node`: a held box charges what dividing it charged and divides as it
/// did; any other is divided, at the work that charges. A box that is divided holds its
/// polygons' points: the root's are made for it, and a split gives each half its own.
fn step(
    node: &Node,
    leaf: usize,
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Step, TraceError> {
    if let Some(old) = node.kept.old.as_ref().filter(|_| node.kept.held) {
        meter.charge(old.work)?;
        return Ok(Step::Held);
    }
    let before = meter.used().work;
    let outcome = divide(node, (leaf, &[]), tests, meter)?;
    // A box's work is what its charges added, each a u64 the meter took: past u64 it would have
    // stopped first.
    let work = u64::try_from(meter.used().work.saturating_sub(before)).unwrap_or(u64::MAX);
    Ok(Step::Fresh(outcome, work))
}

/// The boxes `step` makes of `node`, onto `next`, or `node` onto `leaves`; and `node` into
/// `pending` when this call divides it.
fn next_level(
    node: Node,
    step: Step,
    pending: &mut Vec<Option<Pending>>,
    leaves: &mut Vec<Node>,
    next: &mut Vec<Node>,
) {
    let depth = node.depth.saturating_add(1);
    let bit = 1u128
        .checked_shl(127u32.saturating_sub(node.depth))
        .unwrap_or(0);
    let sources = |from: &[usize]| -> Vec<usize> {
        from.iter()
            .filter_map(|at| node.sources.get(*at).copied())
            .collect()
    };
    let held = node.kept.old.clone().filter(|_| node.kept.held);
    let (shape, work) = match (step, held) {
        (Step::Held, Some(old)) => {
            if let Shape::Split(children) = &old.shape {
                for (child, key) in [
                    (&children.lower, node.key),
                    (&children.upper, node.key | bit),
                ] {
                    let divided = &child.divided;
                    next.push(Node {
                        key,
                        depth,
                        cell: divided.cell,
                        reference: divided.reference.clone(),
                        polygons: Arc::clone(&divided.polygons),
                        sources: sources(&child.from),
                        points: Vec::new(),
                        kept: Trail {
                            old: Some(Arc::clone(divided)),
                            held: true,
                            slot: None,
                        },
                    });
                }
            }
            (PendingShape::Held(Arc::clone(&old)), old.work)
        }
        (Step::Held, None) => (PendingShape::Dropped, 0),
        (Step::Fresh(Outcome::Dropped, work), _) => (PendingShape::Dropped, work),
        (Step::Fresh(Outcome::Leaf, work), _) => (PendingShape::Leaf(None), work),
        (Step::Fresh(Outcome::Split(plan), work), _) => {
            // The box's polygons stay with it for the tree, so each half takes copies.
            let origins = plan.origins();
            let (under, over) = split_copied(&node, *plan);
            let olds = match node.kept.old.as_deref().map(|old| &old.shape) {
                Some(Shape::Split(children)) => [
                    Some(Arc::clone(&children.lower.divided)),
                    Some(Arc::clone(&children.upper.divided)),
                ],
                _ => [None, None],
            };
            let mut slots = [(Vec::new(), 0usize), (Vec::new(), 0usize)];
            for ((child, from), (old, slot)) in [under, over]
                .into_iter()
                .zip(origins)
                .zip(olds.into_iter().zip(&mut slots))
            {
                *slot = (from, pending.len());
                next.push(Node {
                    kept: Trail {
                        old,
                        held: false,
                        slot: Some(pending.len()),
                    },
                    ..child
                });
                pending.push(None);
            }
            (PendingShape::Split(slots), work)
        }
    };
    let is_leaf = matches!(
        (&shape, node.kept.old.as_deref().map(|old| &old.shape)),
        (PendingShape::Leaf(_), _) | (PendingShape::Held(_), Some(Shape::Leaf(_)))
    );
    if let Some(slot) = node.kept.slot.and_then(|at| pending.get_mut(at)) {
        *slot = Some(Pending {
            cell: node.cell,
            reference: node.reference.clone(),
            polygons: Arc::clone(&node.polygons),
            work,
            shape,
        });
    }
    if is_leaf {
        leaves.push(node);
    }
}

/// The tree of `pending`, built from its last box back: every box is listed after its parent, so
/// its children are built before it. A leaf whose key was not found leaves no tree.
fn built(pending: Vec<Option<Pending>>) -> Option<Arc<Divided>> {
    let mut done: Vec<Option<Arc<Divided>>> = alloc::vec![None; pending.len()];
    for (at, one) in pending.into_iter().enumerate().rev() {
        let one = one?;
        let shape = match one.shape {
            PendingShape::Held(old) => {
                *done.get_mut(at)? = Some(old);
                continue;
            }
            PendingShape::Dropped => Shape::Dropped,
            PendingShape::Leaf(key) => Shape::Leaf(key?),
            PendingShape::Split([(under_from, under), (over_from, over)]) => {
                let mut take = |at: usize| done.get_mut(at).and_then(Option::take);
                Shape::Split(Box::new(Halves {
                    lower: Child {
                        from: under_from,
                        divided: take(under)?,
                    },
                    upper: Child {
                        from: over_from,
                        divided: take(over)?,
                    },
                }))
            }
        };
        *done.get_mut(at)? = Some(Arc::new(Divided {
            cell: one.cell,
            reference: one.reference,
            polygons: one.polygons,
            shape,
            work: one.work,
        }));
    }
    done.into_iter().next().flatten()
}
/// Every piece of the boxes under `node`, in tree order: divided on an explicit stack, the lower box
/// of a split taken first, and each leaf solved as it is reached.
fn subtree(
    node: &Node,
    (leaf, solids): (usize, &[Entry]),
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    let mut out = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    // The unit's box is borrowed, so its split copies; every box below it is the walk's own.
    match divide(node, (leaf, solids), tests, meter)? {
        Outcome::Dropped => {}
        Outcome::Leaf => out.extend(solve(node, solids, meter)?),
        Outcome::Split(plan) => {
            let (under, over) = split_copied(node, *plan);
            stack.push(over);
            stack.push(under);
        }
    }
    while let Some(next) = stack.pop() {
        let outcome = divide(&next, (leaf, solids), tests, meter)?;
        take(next, outcome, (solids, &mut stack, &mut out), meter)?;
    }
    Ok(out)
}

/// What [`subtree`] does with a box's outcome: a leaf's pieces onto `out`, a split's two boxes
/// onto `stack`, the lower last so it comes off first.
fn take(
    node: Node,
    outcome: Outcome,
    (solids, stack, out): (&[Entry], &mut Vec<Node>, &mut Vec<Classified>),
    meter: &mut WorkMeter<'_>,
) -> Result<(), TraceError> {
    match outcome {
        Outcome::Dropped => {}
        Outcome::Leaf => out.extend(solve(&node, solids, meter)?),
        Outcome::Split(plan) => {
            let (under, over) = split(node, *plan);
            stack.push(over);
            stack.push(under);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::panic,
        clippy::unwrap_used,
        reason = "tests: native arithmetic, and a failure stops the test"
    )]

    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use core::fmt::{self, Write as _};

    use geom_num::plane::Plane;
    use geom_run::{Budget, Exec, WorkMeter};

    use super::{
        Child, Divided, Halves, Node, Part, Printer, Shape, Step, Trail, around, arrange_reusing,
        built, next_level, solved_in,
    };
    use crate::convex::Piece;
    use crate::cut::meetings;
    use crate::order::Meeting;
    use crate::trace::{Cell, Polygon};
    use crate::winding::{Winding, WindingTest};

    /// How deep the trees of the deep tests go, far past the tree's bound of 127.
    const DEEP: usize = 100_000;

    /// How deep the pretty text's deep test goes: its indent grows with the depth, so its length
    /// grows with the square.
    const DEEP_PRETTY: usize = 1_000;

    /// The tree's types as `#[derive]` writes them, the same names, fields and order: the text and
    /// the answers the tree's own walks must give. The derived walks recurse a level a call, so
    /// they run only on trees that fit the stack.
    mod derived {
        #![allow(
            dead_code,
            reason = "the fields are read by the derived implementations alone"
        )]

        use alloc::boxed::Box;

        use crate::trace::{Cell, Polygon};
        use crate::winding::Winding;

        #[derive(Debug, PartialEq)]
        pub struct Divided<'a> {
            pub cell: &'a Cell,
            pub reference: &'a Winding,
            pub polygons: &'a [Polygon],
            pub shape: Shape<'a>,
            pub work: u64,
        }

        #[derive(Debug, PartialEq)]
        pub enum Shape<'a> {
            Dropped,
            Leaf(&'a [u8; 32]),
            Split(Box<Halves<'a>>),
        }

        #[derive(Debug, PartialEq)]
        pub struct Halves<'a> {
            pub lower: Child<'a>,
            pub upper: Child<'a>,
        }

        #[derive(Debug, PartialEq)]
        pub struct Child<'a> {
            pub from: &'a [usize],
            pub divided: Divided<'a>,
        }
    }

    /// The derived form of `d`, made a level a call as the derived walks go: for shallow trees.
    fn mirror(d: &Divided) -> derived::Divided<'_> {
        fn child(c: &Child) -> derived::Child<'_> {
            derived::Child {
                from: c.from.as_slice(),
                divided: mirror(&c.divided),
            }
        }
        derived::Divided {
            cell: &d.cell,
            reference: &d.reference,
            polygons: d.polygons.as_slice(),
            shape: match &d.shape {
                Shape::Dropped => derived::Shape::Dropped,
                Shape::Leaf(key) => derived::Shape::Leaf(key),
                Shape::Split(h) => derived::Shape::Split(Box::new(derived::Halves {
                    lower: child(&h.lower),
                    upper: child(&h.upper),
                })),
            },
            work: d.work,
        }
    }

    fn up(axis: usize, at: i64) -> Plane {
        let mut n = [0i128; 3];
        n[axis] = 1;
        Plane::new(n, -i128::from(at)).unwrap()
    }

    fn down(axis: usize, at: i64) -> Plane {
        let mut n = [0i128; 3];
        n[axis] = -1;
        Plane::new(n, i128::from(at)).unwrap()
    }

    /// The six faces of the block from `low` to `high`, each facing out, as operand `entry`.
    fn block(low: [i64; 3], high: [i64; 3], entry: u32, out: &mut Vec<Polygon>) {
        for axis in 0..3 {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let rim = |flip: bool| {
                if flip {
                    alloc::vec![
                        up(u, low[u]),
                        down(v, high[v]),
                        down(u, high[u]),
                        up(v, low[v])
                    ]
                } else {
                    alloc::vec![
                        up(v, low[v]),
                        down(u, high[u]),
                        down(v, high[v]),
                        up(u, low[u])
                    ]
                }
            };
            for (face, flip) in [(up(axis, high[axis]), false), (down(axis, low[axis]), true)] {
                out.push(Polygon {
                    piece: Piece {
                        face,
                        bounds: rim(flip),
                    },
                    entry: Some(entry),
                });
            }
        }
    }

    /// The tree an arrangement of `list` keeps.
    fn kept(list: &[Polygon], leaf: usize, tests: &[WindingTest]) -> Arc<Divided> {
        let cell = around(list).unwrap();
        let meter = &mut WorkMeter::new(Budget::UNLIMITED);
        let exec = Exec::serial();
        let (_, solved) =
            arrange_reusing(&exec, cell, Winding::new(), list, leaf, tests, None, meter).unwrap();
        solved.tree.unwrap()
    }

    /// A box without polygons, the same at every level but for `shape`.
    fn one(shape: Shape) -> Arc<Divided> {
        Arc::new(Divided {
            cell: Cell {
                low: [0, -1, 2],
                high: [3, 4, 5],
            },
            reference: Winding::new(),
            polygons: Arc::new(Vec::new()),
            shape,
            work: 6,
        })
    }

    /// A tree `depth` splits deep: each split's lower box is the next split, its upper a leaf, and
    /// the lowest box has `bottom`.
    fn chain(depth: usize, bottom: Shape) -> Arc<Divided> {
        let mut tree = one(bottom);
        for _ in 0..depth {
            tree = one(Shape::Split(Box::new(Halves {
                lower: Child {
                    from: alloc::vec![0, 1],
                    divided: tree,
                },
                upper: Child {
                    from: alloc::vec![1],
                    divided: one(Shape::Leaf([7; 32])),
                },
            })));
        }
        tree
    }

    /// The length of a text, not kept.
    fn length(text: fmt::Arguments<'_>) -> usize {
        struct Count(usize);
        impl fmt::Write for Count {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.0 += s.len();
                Ok(())
            }
        }
        let mut count = Count(0);
        count.write_fmt(text).unwrap();
        count.0
    }

    /// The tree's text against the derived form's, plain, pretty, with a flag, and inside another
    /// value's pretty text, where the formatter it is given already indents.
    fn prints_as_derived(tree: &Divided) {
        let want = mirror(tree);
        assert_eq!(format!("{tree:?}"), format!("{want:?}"));
        assert_eq!(format!("{tree:#?}"), format!("{want:#?}"));
        assert_eq!(format!("{tree:x?}"), format!("{want:x?}"));
        assert_eq!(
            format!("{:#?}", (1, Some(tree))),
            format!("{:#?}", (1, Some(&want)))
        );
    }

    #[test]
    fn the_kept_tree_prints_what_the_derived_form_prints() {
        let mut list = Vec::new();
        block([0; 3], [4; 3], 0, &mut list);
        block([2; 3], [6; 3], 1, &mut list);
        let split = kept(&list, 4, &[]);
        let leaf = kept(&list, 64, &[]);
        let dropped = kept(&list, 4, &[WindingTest::intersect(2)]);
        assert!(matches!(split.shape, Shape::Split(_)));
        assert!(matches!(leaf.shape, Shape::Leaf(_)));
        assert!(format!("{dropped:?}").contains("Dropped"));
        for tree in [&split, &leaf, &dropped, &chain(3, Shape::Dropped)] {
            prints_as_derived(tree);
        }
    }

    #[test]
    fn the_kept_tree_compares_as_the_derived_form_does() {
        let mut list = Vec::new();
        block([0; 3], [4; 3], 0, &mut list);
        block([2; 3], [6; 3], 1, &mut list);
        let mut moved = Vec::new();
        block([0; 3], [4; 3], 0, &mut moved);
        block([2; 3], [7, 6, 6], 1, &mut moved);
        let trees = [
            kept(&list, 4, &[]),
            kept(&list, 4, &[]),
            kept(&list, 2, &[]),
            kept(&list, 64, &[]),
            kept(&list, 4, &[WindingTest::intersect(2)]),
            kept(&moved, 4, &[]),
            chain(3, Shape::Dropped),
            chain(3, Shape::Leaf([7; 32])),
            chain(3, Shape::Leaf([8; 32])),
            chain(4, Shape::Dropped),
        ];
        // Each tree is built on its own, so no box is shared and every pair is compared whole.
        assert_eq!(*trees[0], *trees[1]);
        for a in &trees {
            for b in &trees {
                assert_eq!(**a == **b, mirror(a) == mirror(b));
            }
        }
        // A box that differs from its twin in one field only, below the top.
        let twin = |change: &dyn Fn(&mut Divided)| {
            let mut low = Divided {
                cell: Cell {
                    low: [0, -1, 2],
                    high: [3, 4, 5],
                },
                reference: Winding::new(),
                polygons: Arc::new(list.clone()),
                shape: Shape::Leaf([7; 32]),
                work: 6,
            };
            change(&mut low);
            one(Shape::Split(Box::new(Halves {
                lower: Child {
                    from: alloc::vec![0],
                    divided: Arc::new(low),
                },
                upper: Child {
                    from: alloc::vec![1],
                    divided: one(Shape::Dropped),
                },
            })))
        };
        let base = twin(&|_| {});
        let changes: [&dyn Fn(&mut Divided); 6] = [
            &|d: &mut Divided| d.cell.high[2] = 9,
            &|d: &mut Divided| d.reference.add(0, 1).unwrap(),
            &|d: &mut Divided| d.polygons = Arc::new(list[1..].to_vec()),
            &|d: &mut Divided| d.shape = Shape::Leaf([8; 32]),
            &|d: &mut Divided| d.shape = Shape::Dropped,
            &|d: &mut Divided| d.work = 7,
        ];
        assert_eq!(*base, *twin(&|_| {}));
        for change in changes {
            let other = twin(change);
            assert_ne!(*base, *other);
            assert_ne!(mirror(&base), mirror(&other));
        }
        // A split whose halves differ only in which of its polygons they took: no call makes
        // two such, as a box's split is its own, but the comparison is the derived one still.
        let took = |from: Vec<usize>| {
            one(Shape::Split(Box::new(Halves {
                lower: Child {
                    from,
                    divided: one(Shape::Dropped),
                },
                upper: Child {
                    from: alloc::vec![1],
                    divided: one(Shape::Dropped),
                },
            })))
        };
        let (first, second) = (took(alloc::vec![0]), took(alloc::vec![0, 1]));
        assert_eq!(*first, *took(alloc::vec![0]));
        assert_ne!(*first, *second);
        assert_ne!(mirror(&first), mirror(&second));
    }

    #[test]
    fn a_struct_or_variant_with_no_fields_prints_its_name_alone() {
        // Every struct and tuple variant of the tree has a field, so the printer never closes one
        // that has none; were one to come, it would print as the derived form does, the name.
        struct Bare;
        impl fmt::Debug for Bare {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut printer = Printer {
                    pretty: f.alternate(),
                    f,
                    depth: 0,
                    on_newline: false,
                    open: Vec::new(),
                };
                let mut stack = alloc::vec![
                    Part::CloseTuple,
                    Part::Open("Variant"),
                    Part::CloseStruct,
                    Part::Open("Unit"),
                ];
                while let Some(part) = stack.pop() {
                    printer.put(part, &mut stack)?;
                }
                // Nothing open is left to close.
                printer.put(Part::CloseStruct, &mut stack)?;
                printer.put(Part::CloseTuple, &mut stack)
            }
        }
        assert_eq!(format!("{Bare:?}"), "UnitVariant");
        assert_eq!(format!("{Bare:#?}"), "UnitVariant");
    }

    #[test]
    fn a_leaf_without_its_points_makes_them() {
        // A box taken from an earlier tree carries no points, and its answer is always taken with
        // it; solved here anyway, it makes its points and gives the pieces a box with them gives.
        let mut list = Vec::new();
        block([0; 3], [4; 3], 0, &mut list);
        block([2; 3], [6; 3], 1, &mut list);
        let node = |points: Vec<Vec<Option<Meeting>>>| Node {
            key: 0,
            depth: 0,
            cell: around(&list).unwrap(),
            reference: Winding::new(),
            polygons: Arc::new(list.clone()),
            sources: (0..list.len()).collect(),
            points,
            kept: Trail::default(),
        };
        let meter = &mut WorkMeter::new(Budget::UNLIMITED);
        let with = node(list.iter().map(|p| meetings(&p.piece)).collect());
        let without = node(Vec::new());
        let want = solved_in(&with, &[], meter).unwrap();
        assert!(want.len() >= list.len());
        assert_eq!(solved_in(&without, &[], meter).unwrap(), want);
    }

    #[test]
    fn a_held_step_without_a_held_box_is_dropped() {
        // `step` says a box is held only when it holds one, so this never comes; were it to, the
        // box would be kept as dropped and nothing below it made.
        let (mut pending, mut leaves, mut next) = (alloc::vec![None], Vec::new(), Vec::new());
        let node = Node {
            key: 0,
            depth: 0,
            cell: Cell {
                low: [-1; 3],
                high: [1; 3],
            },
            reference: Winding::new(),
            polygons: Arc::new(Vec::new()),
            sources: Vec::new(),
            points: Vec::new(),
            kept: Trail {
                old: None,
                held: false,
                slot: Some(0),
            },
        };
        next_level(node, Step::Held, &mut pending, &mut leaves, &mut next);
        assert!(leaves.is_empty() && next.is_empty());
        let kept = built(pending).unwrap();
        assert_eq!(kept.shape, Shape::Dropped);
        assert_eq!(kept.work, 0);
    }

    #[test]
    fn the_kept_tree_prints_compares_and_drops_a_hundred_levels_deep() {
        let tree = chain(100, Shape::Dropped);
        prints_as_derived(&tree);
        assert_eq!(*tree, *chain(100, Shape::Dropped));
        assert_ne!(*tree, *chain(100, Shape::Leaf([7; 32])));
        assert_ne!(*tree, *chain(99, Shape::Dropped));
        drop(tree);
    }

    #[test]
    fn the_kept_tree_prints_compares_and_drops_far_past_its_bound() {
        // Every level's text is the same, so the plain text grows by the same length a level,
        // and the pretty text, whose indent grows by the same length a level, by a length that
        // grows by the same length a level.
        let plain = |depth| length(format_args!("{:?}", chain(depth, Shape::Dropped)));
        let pretty = |depth| length(format_args!("{:#?}", chain(depth, Shape::Dropped)));
        let deep = chain(DEEP, Shape::Dropped);
        let (one, two) = (plain(1), plain(2));
        assert_eq!(
            length(format_args!("{deep:?}")),
            one + (DEEP - 1) * (two - one)
        );
        let (one, two, three) = (pretty(1), pretty(2), pretty(3));
        let n = DEEP_PRETTY - 1;
        assert_eq!(
            pretty(DEEP_PRETTY),
            one + n * (two - one) + n * (n - 1) / 2 * (three + one - 2 * two)
        );
        assert_eq!(*deep, *chain(DEEP, Shape::Dropped));
        assert_ne!(*deep, *chain(DEEP, Shape::Leaf([7; 32])));
        // A box held elsewhere outlives the tree it was in, and is whole.
        let held = match &deep.shape {
            Shape::Split(halves) => Arc::clone(&halves.lower.divided),
            _ => panic!("the top box is split"),
        };
        drop(deep);
        assert_eq!(Arc::strong_count(&held), 1);
        assert_eq!(*held, *chain(DEEP - 1, Shape::Dropped));
        drop(held);
    }
}
