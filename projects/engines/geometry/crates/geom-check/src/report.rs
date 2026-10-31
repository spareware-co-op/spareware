//! What the checker reports: the groups of chapter 8.3, how many of them a check runs, and each
//! fault with the entities it involves.
//!
//! What it does: [`Group`] names the four groups of 8.3; [`Level`] says which run ([`Level::Cheap`]
//! after every operation in every build, [`Level::Full`] in debug builds, in continuous
//! integration, on every import and on request); [`Entity`] names one entity of a body by its
//! index; [`Fault`] is one thing found wrong, of a [`FaultKind`], at one entity and possibly
//! naming a second; [`Report`] holds the faults of one check in the order they were found (the
//! first [`crate::MAX_FAULTS`], and a count of the rest), the groups that ran, and the groups the
//! level asked for that this checker does not have yet.
//!
//! Guarantees:
//!
//! - **Nothing hidden.** A [`Report`] passes only when it found no fault, kept or counted; a group
//!   the level asked for but which is not built yet is listed in [`Report::missing`], never
//!   counted as passed.
//! - **One order.** Faults are listed in the order the checks find them, which is fixed (each
//!   check runs over rows in ascending order), so the same body gives the same report everywhere.
//! - **Plain words.** Every [`FaultKind`] has a sentence saying what is wrong ([`FaultKind::what`]),
//!   and [`Fault`]'s `Display` names its entities by arena and row.
//!
//! Method: Parasolid's published body check, which runs in gated groups and reports each fault
//! with the entities involved (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`,
//! section 5), as chapter 8.3 sets out
//! (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: constant time per accessor; a report holds one fault per thing found wrong.

use alloc::vec::Vec;
use core::fmt;

use geom_topo::entity::{
    CurveIx, EdgeIx, FaceIx, FinIx, LoopIx, PcurveIx, RegionIx, ShellIx, SurfaceIx, VertexIx,
};

/// One of chapter 8.3's four groups of checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    /// Every loop closes, every fin has a partner or is a boundary, shells are closed and
    /// consistently oriented, regions nest correctly, and Euler's formula holds.
    Combinatorial,
    /// Vertices lie on their edges' curves and faces' surfaces, edges on their faces, pcurves on
    /// their 3D curves, certified with bounds (pieces G85 and on).
    Geometric,
    /// No face crosses itself; no two faces cross except along shared edges (pieces G85 and on).
    SelfIntersection,
    /// The body's exactness class and bounds are consistent with its geometry.
    Exactness,
}

impl Group {
    /// The four groups, in 8.3's order.
    pub const ALL: [Group; 4] = [
        Group::Combinatorial,
        Group::Geometric,
        Group::SelfIntersection,
        Group::Exactness,
    ];

    /// 8.3's number for the group, 1 to 4.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Group::Combinatorial => 1,
            Group::Geometric => 2,
            Group::SelfIntersection => 3,
            Group::Exactness => 4,
        }
    }

    /// Whether the group is cheap, and so runs after every operation in every build: the
    /// combinatorial group, which is what keeps the kernel's own loops finite (23.11), and the
    /// exactness record, both linear in the body's entities. The other two certify geometry.
    #[must_use]
    pub const fn cheap(self) -> bool {
        matches!(self, Group::Combinatorial | Group::Exactness)
    }

    /// Whether this checker has the group yet: groups 1 and 4 (piece G84) and 2 (G85); 3 comes
    /// with G85 and the pieces after it.
    #[must_use]
    pub const fn built(self) -> bool {
        matches!(
            self,
            Group::Combinatorial | Group::Geometric | Group::SelfIntersection | Group::Exactness
        )
    }
}

/// Which groups a check runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The cheap groups ([`Group::cheap`]), after every operation in every build.
    Cheap,
    /// Every group: in debug builds, in continuous integration, on every import and on request.
    Full,
}

impl Level {
    /// The level this build runs after every operation: [`Level::Full`] in a debug build or with
    /// the crate's `full` feature (every test build turns it on, chapter 28.4), otherwise
    /// [`Level::Cheap`], as shipped release builds run.
    #[must_use]
    pub const fn of_build() -> Level {
        #[cfg(any(debug_assertions, feature = "full"))]
        {
            Level::Full
        }
        #[cfg(not(any(debug_assertions, feature = "full")))]
        {
            Level::Cheap
        }
    }

    /// Whether the level runs `group`.
    #[must_use]
    pub const fn runs(self, group: Group) -> bool {
        match self {
            Level::Cheap => group.cheap(),
            Level::Full => true,
        }
    }
}

/// One entity of a body, by its index; [`Entity::Body`] for the body as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entity {
    /// The body as a whole.
    Body,
    /// A region.
    Region(RegionIx),
    /// A shell.
    Shell(ShellIx),
    /// A face.
    Face(FaceIx),
    /// A loop.
    Loop(LoopIx),
    /// A fin.
    Fin(FinIx),
    /// An edge.
    Edge(EdgeIx),
    /// A vertex.
    Vertex(VertexIx),
    /// A surface of the surface pool.
    Surface(SurfaceIx),
    /// A 3D curve of the curve pool.
    Curve(CurveIx),
    /// A pcurve of the pcurve pool.
    Pcurve(PcurveIx),
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entity::Body => f.write_str("the body"),
            Entity::Region(i) => write!(f, "region {}", i.row()),
            Entity::Shell(i) => write!(f, "shell {}", i.row()),
            Entity::Face(i) => write!(f, "face {}", i.row()),
            Entity::Loop(i) => write!(f, "loop {}", i.row()),
            Entity::Fin(i) => write!(f, "fin {}", i.row()),
            Entity::Edge(i) => write!(f, "edge {}", i.row()),
            Entity::Vertex(i) => write!(f, "vertex {}", i.row()),
            Entity::Surface(i) => write!(f, "surface {}", i.row()),
            Entity::Curve(i) => write!(f, "curve {}", i.row()),
            Entity::Pcurve(i) => write!(f, "pcurve {}", i.row()),
        }
    }
}

/// What one fault is. Each says, in [`FaultKind::what`], what is wrong at the fault's entity
/// (`at`) and, where one is named, its second entity (`with`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaultKind {
    /// A list of children (a region's shells, a shell's faces, a face's loops) comes back to an
    /// entry it already passed. `at` is the parent.
    ChainCycle,
    /// A child is listed by a second parent. `at` is the child, `with` the second parent.
    ListedTwice,
    /// A child names a parent other than the one that lists it. `at` is the child, `with` the
    /// parent that lists it.
    WrongParent,
    /// A child is in no list: its parent does not list it. `at` is the child.
    Orphan,
    /// A loop has no fins. `at` is the loop.
    EmptyLoop,
    /// A loop's ring of fins does not come back to its first fin. `at` is the loop, `with` the
    /// fin where the ring broke.
    FinRing,
    /// A fin's `next` does not name it as its `prev`. `at` is the fin.
    FinPrev,
    /// An edge's radial ring of fins does not come back to its first fin. `at` is the edge,
    /// `with` the fin where the ring broke.
    RadialRing,
    /// A fin on an edge's radial ring names another edge. `at` is the edge, `with` the fin.
    RadialEdge,
    /// A fin is not on its edge's radial ring. `at` is the fin.
    OffRadialRing,
    /// An edge has one vertex and not the other. `at` is the edge.
    EdgeEnds,
    /// An entity this kind of body cannot hold: a face in a wire or acorn body, an edge in an
    /// acorn, an edge without fins in a solid or sheet, a vertex on no edge outside an acorn or a
    /// general body. `at` is the entity.
    Misplaced,
    /// A loop does not close: a fin ends at a vertex where the next fin does not start. `at` is
    /// the loop, `with` the fin.
    LoopOpen,
    /// A closed edge without vertices shares its loop with other fins. `at` is the loop, `with`
    /// the fin.
    RingEdgeInLoop,
    /// An edge has a number of fins its body kind does not allow: two in a solid, one or two in a
    /// sheet. `at` is the edge; the fault's value is the count.
    FinCount,
    /// An edge's two fins run the same way along it, so their faces are not consistently
    /// oriented. `at` is the edge.
    PartnerSense,
    /// An edge's two fins lie in different shells, so neither shell closes along it. `at` is the
    /// edge.
    PartnerShells,
    /// A shell's faces are not one connected set. `at` is the shell, `with` a face not connected
    /// to its first.
    ShellDisconnected,
    /// A shell has no faces. `at` is the shell.
    EmptyShell,
    /// A solid region has no shell. `at` is the region.
    EmptyRegion,
    /// A solid body has no solid region. `at` is the body.
    NoMaterial,
    /// A sheet, wire or acorn body has a solid region. `at` is the region.
    MaterialInKind,
    /// A shell of a solid body belongs to a void region. `at` is the shell.
    ShellOfVoid,
    /// A shell breaks Euler's formula: its characteristic V − E + 2F − L (a closed edge without
    /// vertices counting one vertex), with its boundary cycles added for a sheet, is odd or above
    /// 2. `at` is the shell; the fault's value is that sum.
    Euler,
    /// The body is recorded Exact, but a face lies on a curved surface. `at` is the face.
    NotFlat,
    /// The body is recorded Exact, but an edge lies on a curve other than a line. `at` is the edge.
    NotStraight,
    /// The body is recorded Exact, but a fin's pcurve is not a line. `at` is the fin.
    CurvedPcurve,
    /// A vertex does not lie on a face's surface within the body's bound. `at` is the vertex,
    /// `with` the face.
    VertexOffSurface,
    /// A vertex does not lie at its edge's curve end within the body's bound. `at` is the vertex,
    /// `with` the edge.
    VertexOffCurve,
    /// An edge's curve does not lie on a face's surface within the body's bound. `at` is the
    /// edge, `with` the face.
    EdgeOffSurface,
    /// A carrier group 2 has no certified check for yet: a surface or curve of another kind, a
    /// plane not given by integers in a body recorded Exact, or a fin's pcurve. Reported, never
    /// passed. `at` is the surface, curve or pcurve, `with` the face, edge or fin that uses it.
    Unchecked,
    /// Two edges of one face meet other than at a vertex they share. `at` is one edge, `with` the
    /// other.
    EdgesCross,
    /// Two faces meet other than along an edge or at a vertex they share. `at` is one face,
    /// `with` the other.
    FacesCross,
    /// A pair group 3 has no certified test for: two edges of one face or two faces whose
    /// carriers no closed form here separates (a face with holes or not convex against a crossing
    /// plane, two curved faces, a circle crossing a plane's line). Reported, never passed. `at`
    /// and `with` are the two edges or the two faces.
    PairUnchecked,
}

impl FaultKind {
    /// The group that finds this kind of fault.
    #[must_use]
    pub const fn group(self) -> Group {
        match self {
            FaultKind::NotFlat | FaultKind::NotStraight | FaultKind::CurvedPcurve => {
                Group::Exactness
            }
            FaultKind::VertexOffSurface
            | FaultKind::VertexOffCurve
            | FaultKind::EdgeOffSurface
            | FaultKind::Unchecked => Group::Geometric,
            FaultKind::EdgesCross | FaultKind::FacesCross | FaultKind::PairUnchecked => {
                Group::SelfIntersection
            }
            _ => Group::Combinatorial,
        }
    }

    /// What is wrong, in plain words.
    #[must_use]
    pub const fn what(self) -> &'static str {
        match self {
            FaultKind::ChainCycle => "its list of children comes back to an entry it passed",
            FaultKind::ListedTwice => "it is listed by a second parent",
            FaultKind::WrongParent => "it names a parent other than the one that lists it",
            FaultKind::Orphan => "its parent does not list it",
            FaultKind::EmptyLoop => "the loop has no fins",
            FaultKind::FinRing => "the loop's ring of fins does not come back to its first fin",
            FaultKind::FinPrev => "the fin's next fin does not name it as its previous one",
            FaultKind::RadialRing => "the edge's ring of fins does not come back to its first fin",
            FaultKind::RadialEdge => "a fin on the edge's ring names another edge",
            FaultKind::OffRadialRing => "the fin is not on its edge's ring",
            FaultKind::EdgeEnds => "the edge has one vertex and not the other",
            FaultKind::Misplaced => "this kind of body cannot hold it",
            FaultKind::LoopOpen => {
                "the loop does not close: a fin ends where the next does not start"
            }
            FaultKind::RingEdgeInLoop => {
                "a closed edge without vertices shares its loop with other fins"
            }
            FaultKind::FinCount => "the edge has a number of fins its body kind does not allow",
            FaultKind::PartnerSense => "the edge's two fins run the same way along it",
            FaultKind::PartnerShells => "the edge's two fins lie in different shells",
            FaultKind::ShellDisconnected => "the shell's faces are not one connected set",
            FaultKind::EmptyShell => "the shell has no faces",
            FaultKind::EmptyRegion => "the solid region has no shell",
            FaultKind::NoMaterial => "the solid body has no solid region",
            FaultKind::MaterialInKind => "a body of this kind has a solid region",
            FaultKind::ShellOfVoid => "a shell of a solid body belongs to a void region",
            FaultKind::Euler => "the shell breaks Euler's formula",
            FaultKind::NotFlat => "the body is recorded Exact, but the face is curved",
            FaultKind::NotStraight => "the body is recorded Exact, but the edge is not straight",
            FaultKind::CurvedPcurve => "the body is recorded Exact, but the fin's pcurve is curved",
            FaultKind::VertexOffSurface => {
                "the vertex is not on the face's surface within the bound"
            }
            FaultKind::VertexOffCurve => {
                "the vertex is not at the edge's curve end within the bound"
            }
            FaultKind::EdgeOffSurface => "the edge is not on the face's surface within the bound",
            FaultKind::Unchecked => "no certified check of this carrier exists yet",
            FaultKind::EdgesCross => "two edges of the face meet other than at a shared vertex",
            FaultKind::FacesCross => "the two faces meet other than along a shared edge or vertex",
            FaultKind::PairUnchecked => "no certified test of this pair exists yet",
        }
    }
}

/// One fault: what is wrong, at which entity, possibly naming a second entity and a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fault {
    /// What is wrong.
    pub kind: FaultKind,
    /// The entity it is wrong at.
    pub at: Entity,
    /// A second entity the fault involves, where there is one.
    pub with: Option<Entity>,
    /// A count the fault reports, where there is one: an edge's fins ([`FaultKind::FinCount`]),
    /// a shell's characteristic ([`FaultKind::Euler`]).
    pub value: Option<i64>,
}

impl Fault {
    /// A fault at `at`.
    #[must_use]
    pub const fn at(kind: FaultKind, at: Entity) -> Fault {
        Fault {
            kind,
            at,
            with: None,
            value: None,
        }
    }

    /// The same fault, naming a second entity.
    #[must_use]
    pub const fn with(self, with: Entity) -> Fault {
        Fault {
            with: Some(with),
            ..self
        }
    }

    /// The same fault, with a value.
    #[must_use]
    pub const fn value(self, value: i64) -> Fault {
        Fault {
            value: Some(value),
            ..self
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "group {}, {}: {}",
            self.kind.group().number(),
            self.at,
            self.kind.what()
        )?;
        if let Some(with) = self.with {
            write!(f, " ({with})")?;
        }
        if let Some(value) = self.value {
            write!(f, " ({value})")?;
        }
        Ok(())
    }
}

/// What one check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub(crate) faults: Vec<Fault>,
    pub(crate) more: u64,
    pub(crate) ran: [bool; 4],
    pub(crate) missing: [bool; 4],
}

impl Report {
    /// Whether the body passed every group that ran: no fault at all.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.faults.is_empty() && self.more == 0
    }

    /// The first [`crate::MAX_FAULTS`] faults, in the order they were found.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }

    /// How many more faults were found past [`crate::MAX_FAULTS`], counted and not kept.
    #[must_use]
    pub const fn more(&self) -> u64 {
        self.more
    }

    /// The groups that ran, in 8.3's order.
    pub fn ran(&self) -> impl Iterator<Item = Group> + '_ {
        Group::ALL
            .into_iter()
            .zip(self.ran)
            .filter_map(|(g, on)| on.then_some(g))
    }

    /// The groups the level asked for that this checker does not have yet ([`Group::built`]),
    /// in 8.3's order. They did not run, and nothing about them is claimed.
    pub fn missing(&self) -> impl Iterator<Item = Group> + '_ {
        Group::ALL
            .into_iter()
            .zip(self.missing)
            .filter_map(|(g, on)| on.then_some(g))
    }
}
