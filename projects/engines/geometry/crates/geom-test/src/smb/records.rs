//! The topology records of an `SMLib` B-rep database: regions, shells, faceuses, faces, loops,
//! edge uses, edges and vertices, each read from its ASCII line as `SmBrepData`'s record readers
//! read it.
//!
//! What it does: each record type holds the fields its line carries, and its `read` takes them
//! from a [`Reader`] in the order `SMLib` writes them, with the fields a later database version
//! added read only from that version on. A record starts with every field zero, as `SmTArray`
//! leaves the records it sizes, so a field its line does not carry stays zero.
//!
//! Guarantees:
//!
//! - **Field for field.** A record holds the values of its line in `SMLib`'s order, every index
//!   and flag as the integer written and every double as the stream reads it.
//! - **What a line lacks stays zero.** An extraction that fails stores nothing (as in C++), and the
//!   caller sees the stream fail ([`Reader::failed`]) rather than a value made up.
//!
//! Method: `SmBrepData::ReadFromDB` and its record classes' `ReadFromDB`
//! (<https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBrepData.cpp>),
//! as solidcore ports them.
//!
//! Source: PORT from NVIDIA USD `BRep` `SMLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBrepData.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated from the record classes' ASCII `ReadFromDB` branches through
//!   solidcore's port; no attributes, which `SmBrepData::ReadFromFile` does not read either.
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/topology/brep_data.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the ASCII readers alone, one plain struct per record with the fields
//!   the reader of a B-rep's shape needs, and no binary branch.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: constant per record.

use super::Reader;

/// The fields every record carries: from database version 32 on, its flags and two user indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Common {
    /// `m_lFlags`.
    pub flags: u64,
    /// `m_lUserIndex1`.
    pub user1: u64,
    /// `m_lUserIndex2`.
    pub user2: u64,
}

impl Common {
    /// `SmTopologyData::ReadFromDB`.
    fn read(r: &mut Reader) -> Common {
        let mut c = Common::default();
        if r.version > 31 {
            r.ulong(&mut c.flags);
            r.ulong(&mut c.user1);
            r.ulong(&mut c.user2);
        }
        c
    }
}

/// `SmRegionData`: a region and its shells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Region {
    /// The common fields.
    pub common: Common,
    /// `m_lNumShells`.
    pub num_shells: u64,
    /// `m_lStartShell`.
    pub start_shell: u64,
    /// `m_bIsVoidFlag`: 1 for the infinite region around a solid.
    pub is_void: i32,
}

impl Region {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Region {
        let mut x = Region::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.ulong(&mut x.num_shells);
        r.ulong(&mut x.start_shell);
        r.int(&mut x.is_void);
        r.next_line();
        x
    }
}

/// `SmShellData`: a shell of faceuses (type 0), a wire edge (1) or a vertex (2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shell {
    /// The common fields.
    pub common: Common,
    /// `m_lShellType`.
    pub shell_type: u64,
    /// `m_lNumFaceuses`.
    pub num_faceuses: u64,
    /// `m_lFaceuseStart`.
    pub faceuse_start: u64,
    /// `m_lEdge`, for a wire-edge shell.
    pub edge: u64,
    /// `m_lVertex`, for a vertex shell.
    pub vertex: u64,
}

impl Shell {
    /// Reads one line; a faceuse shell's orientation, which `SMLib` keeps only so the file's layout
    /// stays the same, is read and dropped.
    pub(super) fn read(r: &mut Reader) -> Shell {
        let mut x = Shell::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.ulong(&mut x.shell_type);
        match x.shell_type {
            0 => {
                let mut orientation = 1;
                r.int(&mut orientation);
                r.ulong(&mut x.num_faceuses);
                r.ulong(&mut x.faceuse_start);
            }
            1 => r.ulong(&mut x.edge),
            2 => r.ulong(&mut x.vertex),
            _ => {}
        }
        r.next_line();
        x
    }
}

/// `SmFaceuseData`: one side of a face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Faceuse {
    /// The common fields.
    pub common: Common,
    /// `m_lFace`.
    pub face: u64,
    /// `m_eOrientation`: 1 the face's own sense, 0 the opposite.
    pub orientation: i32,
}

impl Faceuse {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Faceuse {
        let mut x = Faceuse::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.ulong(&mut x.face);
        r.int(&mut x.orientation);
        r.next_line();
        x
    }
}

/// `SmFaceData`: a face, its surface, its loops and its domain.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Face {
    /// The common fields.
    pub common: Common,
    /// `m_dZoneTol3d`.
    pub tolerance: f64,
    /// `m_lSurface`: the index of its surface.
    pub surface: u64,
    /// `m_lStartLoop`: its first loop, the outer one.
    pub start_loop: u64,
    /// `m_lNumLoops`: how many loops it has, from `start_loop` on, the outer one included.
    pub num_loops: u64,
    /// `m_bRectangularTrim`.
    pub rectangular_trim: i32,
    /// The domain, `[u_min, u_max, v_min, v_max]`.
    pub domain: [f64; 4],
}

impl Face {
    /// Reads its two lines.
    pub(super) fn read(r: &mut Reader) -> Face {
        let mut x = Face::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.double(&mut x.tolerance);
        r.ulong(&mut x.surface);
        r.ulong(&mut x.start_loop);
        r.ulong(&mut x.num_loops);
        r.int(&mut x.rectangular_trim);
        r.next_line();
        let [u0, u1, v0, v1] = &mut x.domain;
        r.double(u0);
        r.double(u1);
        r.double(v0);
        r.double(v1);
        r.next_line();
        x
    }
}

/// `SmLoopData`: a loop of edge uses (type 0) or a single vertex (1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Loop {
    /// The common fields.
    pub common: Common,
    /// `m_lLoopType`.
    pub loop_type: u64,
    /// `m_lNumEU`.
    pub num_edgeuses: u64,
    /// `m_lStartEU`.
    pub start_edgeuse: u64,
    /// `m_lVertex`, for a vertex loop.
    pub vertex: u64,
}

impl Loop {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Loop {
        let mut x = Loop::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.ulong(&mut x.loop_type);
        match x.loop_type {
            0 => {
                r.ulong(&mut x.num_edgeuses);
                r.ulong(&mut x.start_edgeuse);
            }
            1 => r.ulong(&mut x.vertex),
            _ => {}
        }
        r.next_line();
        x
    }
}

/// `SmEUData`: an edge use in a loop (type 0), a vertex use (1, or 3 at a pole) or a wire edge's
/// use (2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edgeuse {
    /// The common fields.
    pub common: Common,
    /// From database version 32 on, the mate's flags and user indices.
    pub mate: Common,
    /// `m_lEUType`.
    pub eu_type: u64,
    /// `m_lEdge`.
    pub edge: u64,
    /// `m_lVertex`.
    pub vertex: u64,
    /// `m_lLoop`.
    pub loop_index: u64,
    /// `m_lShell`.
    pub shell: u64,
    /// `m_eOrientation`: 1 along the edge, 0 against it.
    pub orientation: i32,
    /// `m_lUVCurve`: its curve in the face's domain.
    pub uv_curve: u64,
    /// `m_lNextEU`: the next edge use of the loop; `SMLib` writes it signed.
    pub next: i64,
    /// `m_lMateNextEU`.
    pub mate_next: i64,
}

impl Edgeuse {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Edgeuse {
        let mut x = Edgeuse::default();
        r.skip_ulong();
        x.common = Common::read(r);
        x.mate = Common::read(r);
        r.ulong(&mut x.eu_type);
        match x.eu_type {
            0 => {
                r.ulong(&mut x.edge);
                r.ulong(&mut x.loop_index);
                r.int(&mut x.orientation);
                r.ulong(&mut x.uv_curve);
                r.long(&mut x.next);
                r.long(&mut x.mate_next);
            }
            1 | 3 => {
                r.ulong(&mut x.vertex);
                r.ulong(&mut x.loop_index);
                r.int(&mut x.orientation);
                r.ulong(&mut x.uv_curve);
            }
            2 => {
                r.ulong(&mut x.edge);
                r.ulong(&mut x.shell);
                r.int(&mut x.orientation);
                r.ulong(&mut x.uv_curve);
            }
            _ => {}
        }
        r.next_line();
        x
    }
}

/// `SmEdgeData`: an edge, its curve, its ends and its interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edge {
    /// The common fields.
    pub common: Common,
    /// `m_dZoneTol3d`.
    pub tolerance: f64,
    /// `m_lCurve`: the index of its 3D curve.
    pub curve: u64,
    /// `m_lStartVertex`.
    pub start: u64,
    /// `m_lEndVertex`.
    pub end: u64,
    /// `m_lPrimEU`: its primary edge use, written signed.
    pub primary: i64,
    /// The interval of its curve, `[min, max]`.
    pub interval: [f64; 2],
}

impl Edge {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Edge {
        let mut x = Edge::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.double(&mut x.tolerance);
        r.ulong(&mut x.curve);
        r.ulong(&mut x.start);
        r.ulong(&mut x.end);
        r.long(&mut x.primary);
        let [a, b] = &mut x.interval;
        r.double(a);
        r.double(b);
        r.next_line();
        x
    }
}

/// `SmVertexData`: a vertex and its point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    /// The common fields.
    pub common: Common,
    /// `m_dZoneTol3d`.
    pub tolerance: f64,
    /// `m_vPoint`.
    pub point: [f64; 3],
}

impl Vertex {
    /// Reads one line.
    pub(super) fn read(r: &mut Reader) -> Vertex {
        let mut x = Vertex::default();
        r.skip_ulong();
        x.common = Common::read(r);
        r.double(&mut x.tolerance);
        let [px, py, pz] = &mut x.point;
        r.double(px);
        r.double(py);
        r.double(pz);
        r.next_line();
        x
    }
}
