//! Structural analysis of a sketch: which equations and variables belong together, and what can be
//! told about them from the pattern alone.
//!
//! What it does: [`Graph`] is the bipartite graph of equations and variables. [`substitute`] builds
//! it from a sketch, merging the two variables of every equality between single variables, such as
//! coincident points or a horizontal line. [`analyse`] splits the graph into connected
//! [`Component`]s, finds a largest matching of equations to variables by Hopcroft and Karp's method,
//! and from it the Dulmage–Mendelsohn decomposition of each component: an over-determined part, an
//! under-determined part, and square [`Part`]s in the order to solve them.
//!
//! Guarantees:
//!
//! - **From structure alone, in exact integers.** No value is read, so the plan is the same on every
//!   machine. Every tie goes to the lower index, and indices are authoring order.
//! - **Parts are exact.** The over-determined and under-determined parts do not depend on which
//!   largest matching was found, and neither do the square blocks as sets. The order of the blocks
//!   is the one that takes, of those ready, the block with the lowest equation.
//! - **Iterative.** Every graph walk has an explicit stack charged to the meter, so a chain of any
//!   length cannot overflow the machine's stack (principle 13).
//! - **A solve order.** Each square block's equations touch only variables of its own block, of
//!   earlier blocks, and of the under-determined part.
//! - **Exact counts.** A component's `freedoms` is its under-determined variables less equations, and
//!   its `excess` its over-determined equations less variables.
//!
//! Method: section 15.5 (`projects/engines/geometry/architecture/15-sketches.md`): J. E. Hopcroft and
//! R. M. Karp, "An n^(5/2) algorithm for maximum matchings in bipartite graphs", SIAM J. Comput. 2(4)
//! (1973), doi:10.1137/0202019; A. L. Dulmage and N. S. Mendelsohn, "Coverings of bipartite graphs",
//! Canad. J. Math. 10 (1958), doi:10.4153/CJM-1958-052-0; A. Pothen and C.-J. Fan, "Computing the
//! block triangular form of a sparse matrix", ACM TOMS 16(4) (1990), doi:10.1145/98267.98287; and
//! strongly connected components by R. E. Tarjan, "Depth-first search and linear graph algorithms",
//! SIAM J. Comput. 1(2) (1972), doi:10.1137/0201010, with its recursion on an explicit stack.
//!
//! Cost: O(z√n) for the matching and O(z + n log n) for the rest, z the graph's edges and n its
//! nodes; O(z + n) memory. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the papers': e an equation, v a variable, w another equation, k a block"
)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::equation::{Slot, inputs_of, rules_of};
use crate::model::{ConstraintKind, ItemId, Sketch, SketchError};

/// No node.
const NONE: usize = usize::MAX;

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn slot<T>(list: &mut [T], i: usize) -> Result<&mut T, SketchError> {
    list.get_mut(i).ok_or(SketchError::Internal)
}

fn next(i: usize) -> Result<usize, SketchError> {
    i.checked_add(1).ok_or(SketchError::Internal)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, SketchError> {
    let mut list = Vec::new();
    list.try_reserve_exact(n)
        .map_err(|_| SketchError::OutOfMemory)?;
    list.resize(n, value);
    Ok(list)
}

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The bipartite graph of equations and variables, by equation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    variables: usize,
    /// Equation e's variables are at `starts[e]..starts[e + 1]`.
    starts: Vec<usize>,
    /// Each equation's variables, ascending and without repeats.
    incident: Vec<usize>,
}

impl Graph {
    /// The graph of `variables` variables and one equation for each of `rows`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for a variable past the last; [`SketchError::OutOfMemory`].
    pub fn new<'r>(
        variables: usize,
        rows: impl Iterator<Item = &'r [usize]>,
    ) -> Result<Graph, SketchError> {
        let mut starts = Vec::new();
        let mut incident = Vec::new();
        push(&mut starts, 0)?;
        for row in rows {
            let from = incident.len();
            for &v in row {
                if v >= variables {
                    return Err(SketchError::Internal);
                }
                push(&mut incident, v)?;
            }
            let mine = incident.get_mut(from..).ok_or(SketchError::Internal)?;
            mine.sort_unstable();
            let mut kept = 0_usize;
            let mut last = NONE;
            for at in 0..mine.len() {
                let v = get(mine, at)?;
                if v != last {
                    *slot(mine, kept)? = v;
                    kept = next(kept)?;
                    last = v;
                }
            }
            incident.truncate(from.checked_add(kept).ok_or(SketchError::Internal)?);
            push(&mut starts, incident.len())?;
        }
        Ok(Graph {
            variables,
            starts,
            incident,
        })
    }

    /// How many equations.
    #[must_use]
    pub fn equations(&self) -> usize {
        self.starts.len().saturating_sub(1)
    }

    /// How many variables.
    #[must_use]
    pub const fn variables(&self) -> usize {
        self.variables
    }

    /// Equation `e`'s variables, ascending.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] past the last equation.
    pub fn row(&self, e: usize) -> Result<&[usize], SketchError> {
        let (lo, hi) = (get(&self.starts, e)?, get(&self.starts, next(e)?)?);
        self.incident.get(lo..hi).ok_or(SketchError::Internal)
    }

    /// Each variable's equations, ascending: `(starts, equations)`.
    fn transpose(&self) -> Result<(Vec<usize>, Vec<usize>), SketchError> {
        let mut starts = filled(next(self.variables)?, 0_usize)?;
        for &v in &self.incident {
            let count = slot(&mut starts, next(v)?)?;
            *count = next(*count)?;
        }
        for v in 0..self.variables {
            let before = get(&starts, v)?;
            let here = slot(&mut starts, next(v)?)?;
            *here = here.checked_add(before).ok_or(SketchError::Internal)?;
        }
        let mut place = starts.clone();
        let mut equations = filled(self.incident.len(), 0_usize)?;
        for e in 0..self.equations() {
            for &v in self.row(e)? {
                let at = slot(&mut place, v)?;
                *slot(&mut equations, *at)? = e;
                *at = next(*at)?;
            }
        }
        Ok((starts, equations))
    }
}

/// A sketch's graph after substitution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Substituted {
    /// The graph, on the merged variables.
    pub graph: Graph,
    /// Which merged variable each of the sketch's variables became.
    pub merged: Vec<usize>,
    /// Which constraint each equation came from.
    pub owners: Vec<ItemId>,
}

/// The root of `v`, halving the path as it goes. A root is its set's lowest variable.
fn root(parent: &mut [usize], mut v: usize) -> Result<usize, SketchError> {
    loop {
        let up = get(parent, v)?;
        if up == v {
            return Ok(v);
        }
        let above = get(parent, up)?;
        *slot(parent, v)? = above;
        v = above;
    }
}

/// Joins the sets of `a` and `b` under the lower root.
fn join(parent: &mut [usize], a: usize, b: usize) -> Result<(), SketchError> {
    let (a, b) = (root(parent, a)?, root(parent, b)?);
    let (low, high) = if a < b { (a, b) } else { (b, a) };
    *slot(parent, high)? = low;
    Ok(())
}

/// Which of a kind's inputs it says are equal, if that is all it says: the stage 1 equalities of
/// section 15.5.
const fn pairs(kind: &ConstraintKind) -> Option<&'static [(usize, usize)]> {
    match kind {
        ConstraintKind::Coincident { .. } | ConstraintKind::Concentric { .. } => {
            Some(&[(0, 2), (1, 3)])
        }
        ConstraintKind::Horizontal { .. }
        | ConstraintKind::Vertical { .. }
        | ConstraintKind::EqualRadius { .. } => Some(&[(0, 1)]),
        _ => None,
    }
}

/// The pairs a rule merges: none if an end is a constant, a reference point's, so that it stays an
/// equation.
fn merges(kind: &ConstraintKind, slots: &[Slot]) -> Option<&'static [(usize, usize)]> {
    let variable = |i: usize| matches!(slots.get(i), Some(Slot::Variable(_)));
    pairs(kind).filter(|list| list.iter().all(|&(a, b)| variable(a) && variable(b)))
}

/// The variable in `slots` at `i`.
fn column(slots: &[Slot], i: usize) -> Result<usize, SketchError> {
    match slots.get(i) {
        Some(Slot::Variable(v)) => Ok(*v),
        _ => Err(SketchError::Internal),
    }
}

/// The graph of `sketch`'s rules, the driving constraints and each arc's two ends, after merging the
/// variables of every equality between two of them.
///
/// # Errors
///
/// [`SketchError::OutOfMemory`]; what the sketch's lookups refuse.
pub fn substitute(sketch: &Sketch) -> Result<Substituted, SketchError> {
    let variables = sketch.variables()?;
    let count = variables.len();
    let mut parent = filled(count, 0_usize)?;
    for (v, held) in parent.iter_mut().enumerate() {
        *held = v;
    }
    let rules = rules_of(sketch)?;
    let mut slots = Vec::new();
    for (_, kind) in &rules {
        slots.clear();
        inputs_of(sketch, kind, &variables, &mut slots)?;
        for &(a, b) in merges(kind, &slots).unwrap_or(&[]) {
            join(&mut parent, column(&slots, a)?, column(&slots, b)?)?;
        }
    }
    let mut merged = filled(count, NONE)?;
    let mut next_id = 0_usize;
    for v in 0..count {
        let top = root(&mut parent, v)?;
        if top == v {
            *slot(&mut merged, v)? = next_id;
            next_id = next(next_id)?;
        } else {
            *slot(&mut merged, v)? = get(&merged, top)?;
        }
    }
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut owners = Vec::new();
    for (id, kind) in &rules {
        slots.clear();
        let equations = inputs_of(sketch, kind, &variables, &mut slots)?;
        if merges(kind, &slots).is_some() {
            continue;
        }
        let mut row = Vec::new();
        for slot in &slots {
            if let Slot::Variable(v) = slot {
                push(&mut row, get(&merged, *v)?)?;
            }
        }
        for _ in 0..equations {
            push(&mut rows, row.clone())?;
            push(&mut owners, *id)?;
        }
    }
    let graph = Graph::new(next_id, rows.iter().map(Vec::as_slice))?;
    Ok(Substituted {
        graph,
        merged,
        owners,
    })
}

/// A largest matching: each equation's variable and each variable's equation, or [`NONE`].
struct Matching {
    of_equation: Vec<usize>,
    of_variable: Vec<usize>,
}

/// The first variable of each equation that is free takes it: the start every phase improves on.
fn greedy(graph: &Graph, matching: &mut Matching) -> Result<(), SketchError> {
    for e in 0..graph.equations() {
        for &v in graph.row(e)? {
            if get(&matching.of_variable, v)? == NONE {
                *slot(&mut matching.of_variable, v)? = e;
                *slot(&mut matching.of_equation, e)? = v;
                break;
            }
        }
    }
    Ok(())
}

/// Hopcroft and Karp's layers: each free equation at 0, each equation reached at one more than the
/// one before it. Whether a free variable was reached.
fn layers(
    graph: &Graph,
    matching: &Matching,
    depth: &mut [usize],
    queue: &mut Vec<usize>,
) -> Result<bool, SketchError> {
    queue.clear();
    for e in 0..graph.equations() {
        if get(&matching.of_equation, e)? == NONE {
            *slot(depth, e)? = 0;
            push(queue, e)?;
        } else {
            *slot(depth, e)? = NONE;
        }
    }
    let mut found = false;
    let mut at = 0_usize;
    while let Some(&e) = queue.get(at) {
        at = next(at)?;
        let there = next(get(depth, e)?)?;
        for &v in graph.row(e)? {
            let w = get(&matching.of_variable, v)?;
            if w == NONE {
                found = true;
            } else if get(depth, w)? == NONE {
                *slot(depth, w)? = there;
                push(queue, w)?;
            }
        }
    }
    Ok(found)
}

/// One augmenting path from free equation `start` along the layers, walked on an explicit stack of
/// (equation, the variable it tried last). `tried` holds each equation's place in its row.
fn augment(
    graph: &Graph,
    matching: &mut Matching,
    depth: &mut [usize],
    tried: &mut [usize],
    stack: &mut Vec<(usize, usize)>,
    start: usize,
) -> Result<(), SketchError> {
    stack.clear();
    push(stack, (start, NONE))?;
    while let Some(&(e, _)) = stack.last() {
        let row = graph.row(e)?;
        let place = get(tried, e)?;
        let Some(&v) = row.get(place) else {
            // Nothing past this equation: it is out of this phase.
            *slot(depth, e)? = NONE;
            stack.pop();
            continue;
        };
        *slot(tried, e)? = next(place)?;
        if let Some(top) = stack.last_mut() {
            top.1 = v;
        }
        let w = get(&matching.of_variable, v)?;
        if w == NONE {
            for &(e, v) in stack.iter() {
                *slot(&mut matching.of_equation, e)? = v;
                *slot(&mut matching.of_variable, v)? = e;
            }
            return Ok(());
        }
        if get(depth, w)? == next(get(depth, e)?)? {
            push(stack, (w, NONE))?;
        }
    }
    Ok(())
}

fn match_all(graph: &Graph, meter: &mut WorkMeter<'_>) -> Result<Matching, SketchError> {
    let mut matching = Matching {
        of_equation: filled(graph.equations(), NONE)?,
        of_variable: filled(graph.variables, NONE)?,
    };
    greedy(graph, &mut matching)?;
    let mut depth = filled(graph.equations(), NONE)?;
    let mut tried = filled(graph.equations(), 0_usize)?;
    let (mut queue, mut stack) = (Vec::new(), Vec::new());
    loop {
        meter.charge(cost(graph.incident.len()).saturating_add(cost(graph.equations())))?;
        if !layers(graph, &matching, &mut depth, &mut queue)? {
            return Ok(matching);
        }
        tried.fill(0);
        for e in 0..graph.equations() {
            if get(&matching.of_equation, e)? == NONE {
                augment(graph, &mut matching, &mut depth, &mut tried, &mut stack, e)?;
            }
        }
    }
}

/// Which side of the decomposition a node is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Square,
    Over,
    Under,
}

/// Marks the over- and under-determined parts: what alternating paths reach from each free
/// equation, and from each free variable.
fn sides(
    graph: &Graph,
    matching: &Matching,
    by_variable: (&[usize], &[usize]),
) -> Result<(Vec<Side>, Vec<Side>), SketchError> {
    let mut equation = filled(graph.equations(), Side::Square)?;
    let mut variable = filled(graph.variables, Side::Square)?;
    let mut stack = Vec::new();
    for e in 0..graph.equations() {
        if get(&matching.of_equation, e)? == NONE {
            *slot(&mut equation, e)? = Side::Over;
            push(&mut stack, e)?;
        }
    }
    while let Some(e) = stack.pop() {
        for &v in graph.row(e)? {
            if get(&variable, v)? == Side::Square {
                *slot(&mut variable, v)? = Side::Over;
                let w = get(&matching.of_variable, v)?;
                if w != NONE {
                    *slot(&mut equation, w)? = Side::Over;
                    push(&mut stack, w)?;
                }
            }
        }
    }
    let (starts, equations) = by_variable;
    for v in 0..graph.variables {
        if get(&matching.of_variable, v)? == NONE {
            *slot(&mut variable, v)? = Side::Under;
            push(&mut stack, v)?;
        }
    }
    while let Some(v) = stack.pop() {
        let (lo, hi) = (get(starts, v)?, get(starts, next(v)?)?);
        for &e in equations.get(lo..hi).ok_or(SketchError::Internal)? {
            if get(&equation, e)? == Side::Square {
                *slot(&mut equation, e)? = Side::Under;
                let w = get(&matching.of_equation, e)?;
                if w != NONE {
                    *slot(&mut variable, w)? = Side::Under;
                    push(&mut stack, w)?;
                }
            }
        }
    }
    Ok((equation, variable))
}

/// The equation that determines `v` in a square block, if `v` is in the square part.
fn solver_of(
    v: usize,
    matching: &Matching,
    variable: &[Side],
) -> Result<Option<usize>, SketchError> {
    (get(variable, v)? == Side::Square)
        .then(|| get(&matching.of_variable, v))
        .transpose()
}

/// Tarjan's strongly connected components of the square part, on an explicit stack: each equation's
/// block number, and how many blocks there are. An edge runs from an equation to the one that
/// determines a variable it reads.
fn blocks_of(
    graph: &Graph,
    matching: &Matching,
    equation: &[Side],
    variable: &[Side],
) -> Result<(Vec<usize>, usize), SketchError> {
    let n = graph.equations();
    let mut order = filled(n, NONE)?;
    let mut low = filled(n, 0_usize)?;
    let mut waiting = filled(n, false)?;
    let mut block = filled(n, NONE)?;
    let (mut seen, mut blocks) = (0_usize, 0_usize);
    let (mut path, mut walk): (Vec<usize>, Vec<(usize, usize)>) = (Vec::new(), Vec::new());
    for start in 0..n {
        if get(equation, start)? != Side::Square || get(&order, start)? != NONE {
            continue;
        }
        push(&mut walk, (start, 0))?;
        while let Some(&(e, place)) = walk.last() {
            if place == 0 {
                *slot(&mut order, e)? = seen;
                *slot(&mut low, e)? = seen;
                seen = next(seen)?;
                push(&mut path, e)?;
                *slot(&mut waiting, e)? = true;
            }
            let row = graph.row(e)?;
            if let Some(top) = walk.last_mut() {
                top.1 = next(place)?;
            }
            if let Some(&v) = row.get(place) {
                if let Some(w) = solver_of(v, matching, variable)? {
                    if get(&order, w)? == NONE {
                        push(&mut walk, (w, 0))?;
                    } else if get(&waiting, w)? {
                        let there = get(&order, w)?;
                        if there < get(&low, e)? {
                            *slot(&mut low, e)? = there;
                        }
                    }
                }
                continue;
            }
            walk.pop();
            if let Some(&(up, _)) = walk.last() {
                let mine = get(&low, e)?;
                if mine < get(&low, up)? {
                    *slot(&mut low, up)? = mine;
                }
            }
            if get(&low, e)? == get(&order, e)? {
                while let Some(w) = path.pop() {
                    *slot(&mut waiting, w)? = false;
                    *slot(&mut block, w)? = blocks;
                    if w == e {
                        break;
                    }
                }
                blocks = next(blocks)?;
            }
        }
    }
    Ok((block, blocks))
}

/// The blocks in solve order: of those whose dependencies are done, the one with the lowest
/// equation first, which is the same whatever order Tarjan's walk found them in.
fn solve_order(
    graph: &Graph,
    matching: &Matching,
    variable: &[Side],
    block: &[usize],
    blocks: usize,
) -> Result<Vec<usize>, SketchError> {
    let mut lowest = filled(blocks, NONE)?;
    let mut waits = filled(blocks, 0_usize)?;
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for e in 0..graph.equations() {
        let mine = get(block, e)?;
        if mine == NONE {
            continue;
        }
        if get(&lowest, mine)? == NONE {
            *slot(&mut lowest, mine)? = e;
        }
        for &v in graph.row(e)? {
            if let Some(w) = solver_of(v, matching, variable)? {
                let theirs = get(block, w)?;
                if theirs != mine {
                    push(&mut edges, (theirs, mine))?;
                }
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let mut from = filled(next(blocks)?, 0_usize)?;
    for &(a, b) in &edges {
        let held = slot(&mut from, next(a)?)?;
        *held = next(*held)?;
        let count = slot(&mut waits, b)?;
        *count = next(*count)?;
    }
    for k in 0..blocks {
        let before = get(&from, k)?;
        let here = slot(&mut from, next(k)?)?;
        *here = here.checked_add(before).ok_or(SketchError::Internal)?;
    }
    let mut ready = BTreeSet::new();
    for k in 0..blocks {
        if get(&waits, k)? == 0 {
            ready.insert((get(&lowest, k)?, k));
        }
    }
    let mut order = Vec::new();
    while let Some((_, k)) = ready.pop_first() {
        push(&mut order, k)?;
        let (lo, hi) = (get(&from, k)?, get(&from, next(k)?)?);
        for &(_, b) in edges.get(lo..hi).ok_or(SketchError::Internal)? {
            let count = slot(&mut waits, b)?;
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.insert((get(&lowest, b)?, b));
            }
        }
    }
    Ok(order)
}

/// Some of a component: equations and variables, each ascending.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Part {
    /// Its equations.
    pub equations: Vec<usize>,
    /// Its variables.
    pub variables: Vec<usize>,
}

/// One connected component's decomposition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Component {
    /// More equations than variables: candidates for redundancy or conflict.
    pub over: Part,
    /// More variables than equations.
    pub under: Part,
    /// Square blocks, in the order to solve them.
    pub blocks: Vec<Part>,
    /// The under-determined part's variables less its equations.
    pub freedoms: usize,
    /// The over-determined part's equations less its variables.
    pub excess: usize,
}

/// Every component of a graph, by its lowest variable; a component with no variable, an equation
/// of none, after them in equation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analysis {
    /// The components.
    pub components: Vec<Component>,
    /// The total of the components' freedoms: the structural degrees of freedom.
    pub freedoms: usize,
}

/// Each variable's component number by its lowest variable, and each equation's; a variable-free
/// equation gets a number of its own after those.
fn components_of(graph: &Graph) -> Result<(Vec<usize>, Vec<usize>, usize), SketchError> {
    let mut parent = filled(graph.variables, 0_usize)?;
    for (v, held) in parent.iter_mut().enumerate() {
        *held = v;
    }
    for e in 0..graph.equations() {
        if let Some((&first, rest)) = graph.row(e)?.split_first() {
            for &v in rest {
                join(&mut parent, first, v)?;
            }
        }
    }
    let mut of_variable = filled(graph.variables, NONE)?;
    let mut count = 0_usize;
    for v in 0..graph.variables {
        let top = root(&mut parent, v)?;
        if top == v {
            *slot(&mut of_variable, v)? = count;
            count = next(count)?;
        } else {
            *slot(&mut of_variable, v)? = get(&of_variable, top)?;
        }
    }
    let mut of_equation = filled(graph.equations(), NONE)?;
    for e in 0..graph.equations() {
        *slot(&mut of_equation, e)? = if let Some(&v) = graph.row(e)?.first() {
            get(&of_variable, v)?
        } else {
            let mine = count;
            count = next(count)?;
            mine
        };
    }
    Ok((of_variable, of_equation, count))
}

/// The components of `graph` and each one's decomposition.
///
/// # Errors
///
/// [`SketchError::Stopped`] when the meter stops; [`SketchError::OutOfMemory`].
pub fn analyse(graph: &Graph, meter: &mut WorkMeter<'_>) -> Result<Analysis, SketchError> {
    let (of_variable, of_equation, count) = components_of(graph)?;
    let matching = match_all(graph, meter)?;
    meter.charge(cost(graph.incident.len()).saturating_mul(4))?;
    let (starts, equations) = graph.transpose()?;
    let (equation_side, variable_side) = sides(graph, &matching, (&starts, &equations))?;
    let (block, blocks) = blocks_of(graph, &matching, &equation_side, &variable_side)?;
    let order = solve_order(graph, &matching, &variable_side, &block, blocks)?;
    let mut components: Vec<Component> = filled(count, Component::default())?;
    let mut parts: Vec<Part> = filled(blocks, Part::default())?;
    for e in 0..graph.equations() {
        let component = components
            .get_mut(get(&of_equation, e)?)
            .ok_or(SketchError::Internal)?;
        match get(&equation_side, e)? {
            Side::Over => push(&mut component.over.equations, e)?,
            Side::Under => push(&mut component.under.equations, e)?,
            Side::Square => {
                let part = parts
                    .get_mut(get(&block, e)?)
                    .ok_or(SketchError::Internal)?;
                push(&mut part.equations, e)?;
            }
        }
    }
    for v in 0..graph.variables {
        let component = components
            .get_mut(get(&of_variable, v)?)
            .ok_or(SketchError::Internal)?;
        match get(&variable_side, v)? {
            Side::Over => push(&mut component.over.variables, v)?,
            Side::Under => push(&mut component.under.variables, v)?,
            Side::Square => {
                let solver = get(&matching.of_variable, v)?;
                let part = parts
                    .get_mut(get(&block, solver)?)
                    .ok_or(SketchError::Internal)?;
                push(&mut part.variables, v)?;
            }
        }
    }
    for k in order {
        let part = core::mem::take(parts.get_mut(k).ok_or(SketchError::Internal)?);
        let first = *part.equations.first().ok_or(SketchError::Internal)?;
        let component = components
            .get_mut(get(&of_equation, first)?)
            .ok_or(SketchError::Internal)?;
        push(&mut component.blocks, part)?;
    }
    let mut freedoms = 0_usize;
    for component in &mut components {
        component.freedoms = component
            .under
            .variables
            .len()
            .saturating_sub(component.under.equations.len());
        component.excess = component
            .over
            .equations
            .len()
            .saturating_sub(component.over.variables.len());
        freedoms = freedoms.saturating_add(component.freedoms);
    }
    Ok(Analysis {
        components,
        freedoms,
    })
}
