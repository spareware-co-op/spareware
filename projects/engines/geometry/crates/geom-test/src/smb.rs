//! `SMLib`'s B-rep files (`.smb`), read as `SMLib` reads them: the reader of NVIDIA's fixtures
//! (piece G262a).
//!
//! What it does: [`read`] takes an ASCII `.smb` file's bytes and returns its [`Smb`]: the database
//! version, the B-rep's properties, its topology records ([`records`]) in the order they are
//! written, and its UV curves, 3D curves and surfaces ([`nurbs`]). It follows
//! `SmBrepData::ReadFromFile` and `SmBrepData::ReadFromDB` step for step: the version line (a file
//! without one is version 30) and, from version 42, the healer version; then each section's count
//! and records, the comment lines between them skipped as `SMLib` skips them, one
//! `GoToNextLine` each. [`Smb::edges_of`] and [`Smb::faces_of_edge`] read the records' links.
//!
//! Guarantees:
//!
//! - **`SMLib`'s reading, not a parser of our own.** Every value is taken by the same extraction,
//!   in the same order, with the same lines skipped, as `SMLib`'s ASCII reader ([`stream`] holds the
//!   `std::istream` rules), so a file reads to the values `SMLib` reads from it.
//! - **A file that does not read is an error, never a guess.** A value that does not read, a count
//!   the rest of the file cannot hold, a geometry kind this reader does not hold or a binary file
//!   is an [`SmbError`].
//! - **No attributes.** `ReadFromFile` reads none, so none are read here.
//!
//! What it does not do: binary files, attributes, and the geometry kinds besides B-spline curves
//! and surfaces (lines, arcs, analytic surfaces, offsets, composites), which `SMLib` writes from
//! database version 33 on and no fixture of G262a holds; each is refused with its type code.
//!
//! Method: `SmBrepData::ReadFromFile` and `SmBrepData::ReadFromDB` of `SMLib`, as solidcore's port
//! reproduces them (its `topology::brep_data`).
//!
//! Source: PORT from NVIDIA USD `BRep` `SMLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBrepData.cpp>
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmDatabaseIO.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated from `ReadFromFile`, `ReadFromDB`, `ReadCurveFromDB`,
//!   `ReadSurfaceFromDB`, `GoToNextLine` and `ReadType` through solidcore's port; ASCII only, no
//!   attributes, and B-spline geometry only.
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/topology/brep_data.rs>
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/topology/database_io.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the ASCII path alone, every count checked against the data left before
//!   it allocates, as solidcore checks it, and the stream checked after each record; the records
//!   kept as plain structs and the geometry as the values read.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: linear in the file's bytes; each count is bounded by the bytes left before anything is
//! allocated for it.

pub mod nurbs;
pub mod records;
pub mod stream;

use core::fmt;

use nurbs::{Curve, Surface};
use records::{Edge, Edgeuse, Face, Faceuse, Loop, Region, Shell, Vertex};
use stream::Stream;

/// `SM_MAXSIZE`: the bytes `GoToNextLine` and `ReadType` skip at most.
const MAX_LINE: usize = 256;

/// `sizeof(char sHeader[8])` in `ReadFromFile`.
const HEADER_WORD: usize = 8;

/// `SmBSplineCurve_TYPE` (`CURV_BASE_TYPE + 33`).
pub const BSPLINE_CURVE: i64 = 14_033;

/// `SmBSplineSurface_TYPE` (`SURF_BASE_TYPE + 2`).
pub const BSPLINE_SURFACE: i64 = 15_002;

/// Why a file did not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmbError {
    /// The bytes are not an ASCII B-rep database as `SMLib` writes one: the cause.
    Shape(&'static str),
    /// A curve or surface of a kind this reader does not hold, by its `SMLib` type code.
    Kind(i64),
    /// A binary database, which this reader does not read.
    Binary,
}

impl fmt::Display for SmbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmbError::Shape(why) => write!(f, "not an SMLib B-rep file: {why}"),
            SmbError::Kind(code) => write!(f, "a geometry kind this reader does not hold: {code}"),
            SmbError::Binary => f.write_str("a binary SMLib file, which this reader does not read"),
        }
    }
}

impl std::error::Error for SmbError {}

/// The reading state: the stream and the database version (`SmDatabaseIO`'s read side).
pub(crate) struct Reader {
    stream: Stream,
    /// The database version.
    pub(crate) version: u64,
}

impl Reader {
    /// `GoToNextLine`: skips past the next newline, at most [`MAX_LINE`] bytes; nothing at the end
    /// of the data, where there is no next line (as solidcore's port does).
    pub(crate) fn next_line(&mut self) {
        if !self.stream.eof() {
            self.stream.ignore(MAX_LINE, b'\n');
        }
    }

    /// `>> ULONG` into `slot`, which keeps its value if nothing is stored.
    pub(crate) fn ulong(&mut self, slot: &mut u64) {
        if let Some(v) = self.stream.ulong() {
            *slot = v;
        }
    }

    /// A record's index, read and dropped.
    pub(crate) fn skip_ulong(&mut self) {
        let _ = self.stream.ulong();
    }

    /// `>> long`.
    pub(crate) fn long(&mut self, slot: &mut i64) {
        if let Some(v) = self.stream.long() {
            *slot = v;
        }
    }

    /// `>> SmBoolean` (an `int`).
    pub(crate) fn int(&mut self, slot: &mut i32) {
        if let Some(v) = self.stream.int() {
            *slot = v;
        }
    }

    /// `>> short`.
    pub(crate) fn short(&mut self, slot: &mut i16) {
        if let Some(v) = self.stream.short() {
            *slot = v;
        }
    }

    /// `>> double`.
    pub(crate) fn double(&mut self, slot: &mut f64) {
        if let Some(v) = self.stream.double() {
            *slot = v;
        }
    }

    /// `ReadFailed`: an error once the stream has failed.
    pub(crate) fn check(&self) -> Result<(), SmbError> {
        if self.stream.fail() {
            Err(SmbError::Shape(
                "a value that does not read, or data that ends early",
            ))
        } else {
            Ok(())
        }
    }

    /// An error unless at least `n` bytes are left: every value takes one byte or more.
    pub(crate) fn room(&self, n: usize) -> Result<(), SmbError> {
        if n <= self.stream.remaining() {
            Ok(())
        } else {
            Err(SmbError::Shape("a count the rest of the file cannot hold"))
        }
    }

    /// A section's count, bounded by the bytes left.
    fn count(&mut self) -> Result<usize, SmbError> {
        let mut n = 0;
        self.ulong(&mut n);
        self.check()?;
        let n = usize::try_from(n).map_err(|_| SmbError::Shape("a count past usize"))?;
        self.room(n)?;
        Ok(n)
    }

    /// `ReadType` without a dimension: the line `<word> Type <code>`.
    fn type_code(&mut self) -> Result<i64, SmbError> {
        let _ = self.stream.word(MAX_LINE);
        let _ = self.stream.word(MAX_LINE);
        let code = self.stream.long();
        self.stream.ignore(MAX_LINE, b'\n');
        code.ok_or(SmbError::Shape("a type line that does not read"))
    }

    /// `ReadType` with a dimension: the type line, then `Curve Dim <dim>`.
    fn type_and_dim(&mut self) -> Result<(i64, u64), SmbError> {
        let code = self.type_code()?;
        let _ = self.stream.word(MAX_LINE);
        let _ = self.stream.word(MAX_LINE);
        let dim = self.stream.ulong();
        self.stream.ignore(MAX_LINE, b'\n');
        Ok((code, dim.ok_or(SmbError::Shape("a dimension line that does not read"))?))
    }

    /// Skips `n` lines.
    fn lines(&mut self, n: usize) {
        for _ in 0..n {
            self.next_line();
        }
    }

    /// `n` records of one kind, the stream checked before each.
    fn records<T>(&mut self, n: usize, read: fn(&mut Reader) -> T) -> Result<Vec<T>, SmbError> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if !self.stream.good() {
                return Err(SmbError::Shape(
                    "the data ends before every record of a section is read",
                ));
            }
            out.push(read(self));
        }
        self.check()?;
        Ok(out)
    }

    /// A composite face or edge section: its count and, when not zero, its entries, flattened as
    /// solidcore keeps them: the count, then each entry's item count and items.
    fn composites(&mut self) -> Result<Vec<u64>, SmbError> {
        self.lines(2);
        let n = self.count()?;
        self.next_line();
        let mut list = Vec::new();
        if n == 0 {
            return Ok(list);
        }
        self.next_line();
        list.push(u64::try_from(n).map_err(|_| SmbError::Shape("a count past u64"))?);
        for _ in 0..n {
            self.skip_ulong();
            let mut items = 0;
            self.ulong(&mut items);
            self.check()?;
            list.push(items);
            let m = usize::try_from(items).map_err(|_| SmbError::Shape("a count past usize"))?;
            self.room(m)?;
            for _ in 0..m {
                let mut item = 0;
                self.ulong(&mut item);
                list.push(item);
            }
            self.next_line();
            self.check()?;
        }
        Ok(list)
    }

    /// `ReadCurveFromDB` for a B-rep wanting dimension `dim`: up to version 32 a type line, from 33
    /// on a type and a dimension line; then the curve.
    fn curve(&mut self, dim: usize) -> Result<Curve, SmbError> {
        let code = if self.version > 32 {
            self.type_and_dim()?.0
        } else {
            self.type_code()?
        };
        if code != BSPLINE_CURVE {
            return Err(SmbError::Kind(code));
        }
        Curve::read(self, dim)
    }

    /// `ReadSurfaceFromDB`.
    fn surface(&mut self) -> Result<Surface, SmbError> {
        let code = self.type_code()?;
        if code != BSPLINE_SURFACE {
            return Err(SmbError::Kind(code));
        }
        Surface::read(self)
    }

    /// A geometry section: its count, then each item's index line and the item.
    fn geometry<T>(
        &mut self,
        read: impl Fn(&mut Reader) -> Result<T, SmbError>,
    ) -> Result<Vec<T>, SmbError> {
        let n = self.count()?;
        self.next_line();
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if !self.stream.good() {
                return Err(SmbError::Shape(
                    "the data ends before every item of a section is read",
                ));
            }
            self.skip_ulong();
            self.next_line();
            out.push(read(self)?);
        }
        Ok(out)
    }
}

/// A B-rep as an `SMLib` file holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Smb {
    /// The database version (30 for a file without a version line).
    pub version: u64,
    /// From version 42, the healer version.
    pub healer_version: u64,
    /// The colour.
    pub color: [f64; 3],
    /// The B-rep's tolerance, read up to version 43.
    pub tolerance: f64,
    /// The manifold flag.
    pub manifold: i32,
    /// The regions' count, as the header gives it.
    pub num_regions: u64,
    /// The first region.
    pub start_region: u64,
    /// The regions.
    pub regions: Vec<Region>,
    /// The shells.
    pub shells: Vec<Shell>,
    /// The composite faces, flattened.
    pub composite_faces: Vec<u64>,
    /// The faceuses.
    pub faceuses: Vec<Faceuse>,
    /// The faces.
    pub faces: Vec<Face>,
    /// The loops.
    pub loops: Vec<Loop>,
    /// The composite edges, flattened.
    pub composite_edges: Vec<u64>,
    /// The edge uses.
    pub edgeuses: Vec<Edgeuse>,
    /// The edges.
    pub edges: Vec<Edge>,
    /// The vertices.
    pub vertices: Vec<Vertex>,
    /// The edge uses' curves in their faces' domains.
    pub uv_curves: Vec<Curve>,
    /// The edges' curves.
    pub curves: Vec<Curve>,
    /// The faces' surfaces.
    pub surfaces: Vec<Surface>,
}

/// Reads an ASCII `SMLib` B-rep file (`SmBrepData::ReadFromFile` with `SM_ASCII`).
///
/// # Errors
///
/// An [`SmbError`] for bytes that are not such a file, or that hold what this reader does not read.
pub fn read(bytes: &[u8]) -> Result<Smb, SmbError> {
    if bytes.contains(&0) {
        return Err(SmbError::Binary);
    }
    let mut r = Reader {
        stream: Stream::new(bytes.to_vec()),
        version: 30,
    };
    // The first byte, past one newline, says whether a version line opens the file.
    let mut first = r.stream.get();
    if first == Some(b'\n') {
        first = r.stream.get();
    }
    r.stream.unget();
    if first == Some(b'V') {
        let _ = r.stream.word(HEADER_WORD);
        let v = r.stream.ulong();
        r.next_line();
        r.version = v.ok_or(SmbError::Shape("a version line that does not read"))?;
    }
    let mut healer_version = 0;
    if r.version > 41 {
        let _ = r.stream.word(HEADER_WORD);
        let _ = r.stream.word(HEADER_WORD);
        r.ulong(&mut healer_version);
        r.next_line();
    }
    let mut smb = Smb {
        version: r.version,
        healer_version,
        color: [0.0; 3],
        tolerance: 1.0e-5,
        manifold: 1,
        num_regions: 0,
        start_region: 0,
        regions: Vec::new(),
        shells: Vec::new(),
        composite_faces: Vec::new(),
        faceuses: Vec::new(),
        faces: Vec::new(),
        loops: Vec::new(),
        composite_edges: Vec::new(),
        edgeuses: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
        uv_curves: Vec::new(),
        curves: Vec::new(),
        surfaces: Vec::new(),
    };
    // The properties: "Brep Starts", the colour, the tolerance, the manifold and composites flags,
    // a blank line, "Region Data", the regions' count and the first region, a column comment.
    r.next_line();
    let [cx, cy, cz] = &mut smb.color;
    r.double(cx);
    r.double(cy);
    r.double(cz);
    r.next_line();
    if r.version <= 43 {
        r.double(&mut smb.tolerance);
    }
    r.next_line();
    r.int(&mut smb.manifold);
    r.next_line();
    let mut composites = 0;
    r.int(&mut composites);
    r.lines(3);
    r.ulong(&mut smb.num_regions);
    r.next_line();
    r.ulong(&mut smb.start_region);
    r.lines(2);
    r.check()?;
    let n = usize::try_from(smb.num_regions).map_err(|_| SmbError::Shape("a count past usize"))?;
    r.room(n)?;
    smb.regions = r.records(n, Region::read)?;
    // Shells: a blank line and the title, the count, three column comments.
    r.lines(2);
    let n = r.count()?;
    r.lines(4);
    smb.shells = r.records(n, Shell::read)?;
    smb.composite_faces = r.composites()?;
    r.lines(2);
    let n = r.count()?;
    r.lines(2);
    smb.faceuses = r.records(n, Faceuse::read)?;
    r.lines(2);
    let n = r.count()?;
    r.lines(3);
    smb.faces = r.records(n, Face::read)?;
    r.lines(2);
    let n = r.count()?;
    r.lines(3);
    smb.loops = r.records(n, Loop::read)?;
    smb.composite_edges = r.composites()?;
    r.lines(2);
    let n = r.count()?;
    r.lines(5);
    smb.edgeuses = r.records(n, Edgeuse::read)?;
    r.lines(2);
    let n = r.count()?;
    r.lines(2);
    smb.edges = r.records(n, Edge::read)?;
    r.lines(2);
    let n = r.count()?;
    r.lines(2);
    smb.vertices = r.records(n, Vertex::read)?;
    r.lines(2);
    smb.uv_curves = r.geometry(|r| r.curve(2))?;
    r.lines(2);
    smb.curves = r.geometry(|r| r.curve(3))?;
    r.lines(2);
    smb.surfaces = r.geometry(Reader::surface)?;
    Ok(smb)
}

impl Smb {
    /// The edge uses of edge `edge` that lie in loops, by index.
    #[must_use]
    pub fn edges_of(&self, edge: u64) -> Vec<usize> {
        self.edgeuses
            .iter()
            .enumerate()
            .filter(|(_, u)| u.eu_type == 0 && u.edge == edge)
            .map(|(i, _)| i)
            .collect()
    }

    /// The face that holds loop `index`: the face whose loops run from its first loop for its
    /// count of loops.
    #[must_use]
    pub fn face_of_loop(&self, index: u64) -> Option<usize> {
        self.faces.iter().position(|f| {
            index >= f.start_loop && index.checked_sub(f.start_loop).is_some_and(|k| k < f.num_loops)
        })
    }

    /// The faces edge `edge`'s edge uses lie on, one per edge use, in the edge uses' order.
    #[must_use]
    pub fn faces_of_edge(&self, edge: u64) -> Vec<usize> {
        self.edges_of(edge)
            .into_iter()
            .filter_map(|u| self.edgeuses.get(u))
            .filter_map(|u| self.face_of_loop(u.loop_index))
            .collect()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp,
    reason = "tests: a failure stops the test"
)]
mod tests {
    use super::*;

    /// A unit square's face and its four edges, written as `SMLib` writes a version-30 file.
    const SQUARE: &str = "//Brep Starts
1.0000000000000000 0.0000000000000000 0.0000000000000000 - Color
0.0001000000000000 - Brep Tolerance
1 - Manifold
0 - Allow Composites

//Region Data
1 Regions In Brep
0 - Start Region
//Index, Number Of Shells, First Shell, Is void Region
0 1 0 1

//Shell Data
1 Shells In Brep
//Index, 0(Faceuse), Orientation, Number Of Faceuses, Start Faceuse
//Index, 1(Wire  Edge), Edge
//Index, 2(Vertex), Vertex
0 0 1 1 0

//Composite Face Data
0 Composite Faces In Brep

//Faceuse Data
1 Faceuses In Brep
//Index, Face, Orientation
0 0 1

//Face Data
1 Faces In Brep
//Index, Tol, Surface, Outer Loop, # of Inner Loops, Naturally Trimmed
//UMin, UMax, VMin, VMax
0 0.0001000000000000 0 0 1 0
0.0000000000000000 1.0000000000000000 0.0000000000000000 1.0000000000000000

//Loop Data
1 Loops In Brep
//Index, 0(Edgeuse), # Of Edgeuses, Start Edgeuse
//Index, 1(Vertex), Vertex
0 0 1 0

//Composite Edge Data
0 Composite Edges In Brep

//Edgeuse Data
1 Edgeuses In Brep
//Index, 0(Edge), Edge, Loop, Orientation, UV Curve, Next EU, Mate Next EU
//Index, 1(Vertex), Vertex, Loop, Orientation, UV Curve
//Index, 2(Wire Edge), Edge, Shell, Orientation, UV Curve
//Index, 3(Vertex At Pole), Vertex, Loop, Orientation, UV Curve
0 0 0 0 1 0 0 0

//Edge Data
1 Edges In Brep
//Index, Tol, Curve, Start Vertex, End Vertex, Primary Edgeuse, Param Min, Param Max
0 0.0001000000000000 0 0 1 0 0.0000000000000000 1.0000000000000000

//Vertex Data
2 Vertices In Brep
//Index, Tol, X, Y, Z
0 0.0001000000000000 0.0000000000000000 0.0000000000000000 0.0000000000000000
1 0.0001000000000000 1.0000000000000000 0.0000000000000000 0.0000000000000000

// UV-Curve Data
1 UV-Curves In Brep
0 - UV-Curve:
Curve Type 14033
1
1
0
2
0.0000000000000000 0.0000000000000000
1.0000000000000000 0.0000000000000000
0.0000000000000000
0.0000000000000000
1.0000000000000000
1.0000000000000000

// 3D-Curve Data
1 3D-Curves In Brep
0 - 3D-Curve:
Curve Type 14033
1
1
0
3
0.0000000000000000 0.0000000000000000 0.0000000000000000
1.0000000000000000 0.0000000000000000 0.0000000000000000
0.0000000000000000
0.0000000000000000
1.0000000000000000
1.0000000000000000

// Surface Data
1 Surfaces In Brep
0 - Surface:
Surface Type 15002
1 1
1 1
0
0.0000000000000000 0.0000000000000000 0.0000000000000000
0.0000000000000000 1.0000000000000000 0.0000000000000000
1.0000000000000000 0.0000000000000000 0.0000000000000000
1.0000000000000000 1.0000000000000000 0.0000000000000000
0.0000000000000000
0.0000000000000000
1.0000000000000000
1.0000000000000000
0.0000000000000000
0.0000000000000000
1.0000000000000000
1.0000000000000000
";

    #[test]
    fn a_version_30_file_reads_record_by_record() {
        let smb = read(SQUARE.as_bytes()).unwrap();
        assert_eq!(smb.version, 30);
        assert_eq!(smb.tolerance, 0.0001);
        assert_eq!((smb.regions.len(), smb.shells.len()), (1, 1));
        assert_eq!(smb.faces[0].domain, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(smb.edges[0].interval, [0.0, 1.0]);
        assert_eq!(smb.vertices[1].point, [1.0, 0.0, 0.0]);
        assert_eq!(smb.uv_curves[0].dim, 2);
        assert_eq!(smb.curves[0].points[1], vec![1.0, 0.0, 0.0]);
        let s = &smb.surfaces[0];
        assert_eq!((s.degree, s.count), ([1, 1], [2, 2]));
        assert_eq!(s.points[1], [0.0, 1.0, 0.0]);
        assert_eq!(s.knots[1], vec![0.0, 0.0, 1.0, 1.0]);
        assert_eq!(smb.faces_of_edge(0), vec![0]);
    }

    #[test]
    fn crlf_lines_read_as_lf_lines() {
        let crlf = SQUARE.replace('\n', "\r\n");
        assert_eq!(read(crlf.as_bytes()).unwrap(), read(SQUARE.as_bytes()).unwrap());
    }

    #[test]
    fn what_does_not_read_is_refused() {
        // A file cut short, a degree past NL_MAXDEG, another kind of surface, and a binary file.
        let cut = &SQUARE[..SQUARE.len() - 40];
        assert!(matches!(read(cut.as_bytes()), Err(SmbError::Shape(_))));
        let degree = SQUARE.replace("1 1\n1 1\n0\n", "1 1\n18 1\n0\n");
        assert!(matches!(read(degree.as_bytes()), Err(SmbError::Shape(_))));
        let kind = SQUARE.replace("Surface Type 15002", "Surface Type 15003");
        assert_eq!(read(kind.as_bytes()), Err(SmbError::Kind(15_003)));
        assert_eq!(read(b"V\0\0"), Err(SmbError::Binary));
        let count = SQUARE.replace("2 Vertices In Brep", "2000000 Vertices In Brep");
        assert!(matches!(read(count.as_bytes()), Err(SmbError::Shape(_))));
    }
}
