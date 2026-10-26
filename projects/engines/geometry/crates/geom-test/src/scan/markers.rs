//! The banned-source scanner's markers: the words, phrases, names and values that show code may
//! come from a source chapter 29 bans.
//!
//! What it does: holds the tables [`crate::scan::scan_text`] matches, each with where it comes
//! from. Nothing else in the repository's scanned crates may hold these markers, so this file is
//! one of the scanner's named exemptions ([`crate::scan::repo::EXEMPT`]).
//!
//! Guarantees:
//!
//! - **Every marker has a written source.** Each table below says which rule of chapter 29.20, or
//!   which evidence report, names it; a marker found only by memory is not here.
//! - **Distinctive markers only.** Common words that a routine, pattern or licence shares with
//!   ordinary text (Brent, Jacobi, golden, spline; CENTER, HIDDEN, BRICK; a bare "Triangle", the
//!   GPL constraint solver Frontier, the GPL repository offsets-and-remeshing and the proof tool
//!   Gappa, which runs only as a separate program) are left out, so a hit is a reason to look, not
//!   noise. The phrases of taken-from statements need
//!   both a taking phrase and a copyleft source's name within [`WINDOW`] words, with no negation
//!   just before.
//!
//! Method: chapter 29.20, rules 2 and 7, and chapter 29.4 and 29.6 on Numerical Recipes
//! (`projects/engines/geometry/architecture/29-sources-licences.md`); the Numerical Recipes markers
//! evidence report 13 found in `SMLib`, `NLib` and solidcore (routine names, macros, the step-control
//! constants and the variable names of the Cash-Karp step), and the Autodesk pattern and linetype
//! names and descriptions evidence report 12 found reproduced in `CADCraft`
//! (`projects/kit/notes/G01-evidence/13-vetting-smlib.md`,
//! `projects/kit/notes/G01-evidence/12-vetting-cadcraft.md`); the hatch pattern names Autodesk's
//! own customization guide shows, `ANSI31` among them
//! (<https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Customization/files/GUID-A6F2E6FF-1717-44B6-A476-0CA817ADD77E.htm>)
//! and AR-B816 (<https://help.autodesk.com/cloudhelp/2017/ENU/AutoCAD-LT/files/GUID-79B99823-7600-4CAB-BD12-04D5103D90CB.htm>).
//!
//! Cost: constant tables.

/// Routine names of Numerical Recipes in C, matched against each part of an identifier (split at
/// `_` and at a lower-case letter or digit followed by a capital), ignoring case: rule 7 bans
/// "`svdcmp`, `tqli`, `rkqs` or `pythag` naming", and report 13 found `pythag` as `ST_pythag` and
/// `smos_Pythag`, `polint`, `trapzd`, `qromb`, `qsimp`, `rkck`, `ludcmp`, `tred2` and `dbrent`
/// behind `SMLib`'s and solidcore's routines. The others are the same book's names for the routines
/// chapter 29.4 rewrites from primary sources (LU, QR, SVD, eigenproblems, Brent, tridiagonal
/// solves, quadrature, ODE steps). Decision 32.8 lets the kernel port NVIDIA's `SMLib` release, but
/// keeps its Numerical Recipes-derived routines and fixtures out
/// (`projects/engines/geometry/architecture/32-decisions.md`); these names, the macros and the
/// constant below are what flag one ported with the rest.
pub const NR_ROUTINES: [&str; 47] = [
    "svdcmp", "svbksb", "tqli", "tred2", "rkqs", "rkck", "rkdumb", "odeint", "bsstep", "stifbs",
    "mmid", "pzextr", "rzextr", "ludcmp", "lubksb", "pythag", "polint", "ratint", "trapzd",
    "qromb", "qromo", "qsimp", "qtrap", "qgaus", "zbrent", "zbrak", "rtbis", "rtflsp", "rtsafe",
    "rtnewt", "dbrent", "mnbrak", "frprmn", "dfpmin", "linmin", "dlinmin", "df1dim", "amotry",
    "tridag", "gammln", "betacf", "gasdev", "nrutil", "nrerror", "splie2", "splin2", "qrdcmp",
];

/// Macros and constants of Numerical Recipes, matched as whole words or as parts of an identifier,
/// ignoring case: rule 7 bans the `ZEPS` and `MOV3` macros (and `SIGN`, below); report 13 found
/// `SHFT`-style shifts with `MOV3`, the step-control constants `PGROW`, `PSHRNK` and `ERRCON`
/// (with `SAFETY` and `TINY`, too common to match), and the Bulirsch-Stoer constants `SAFE1`,
/// `SAFE2`, `REDMIN` and `SCALMX`; `NRANSI` is the book's utility header's.
pub const NR_MACROS: [&str; 15] = [
    "ZEPS", "MOV3", "SHFT2", "SHFT3", "GLIMIT", "ERRCON", "PGROW", "PSHRNK", "SAFE1", "SAFE2",
    "REDMIN", "SCALMX", "KMAXX", "IMAXX", "NRANSI",
];

/// Macros of Numerical Recipes too close to ordinary words to match in any case, matched as whole
/// words exactly: rule 7's `SIGN` (ordinary code says `sign` and `Sign`), and the utility
/// header's `NR_END` and `FREE_ARG`.
pub const NR_MACROS_EXACT: [&str; 3] = ["SIGN", "NR_END", "FREE_ARG"];

/// Variable names of Numerical Recipes' Cash-Karp step and its driver, matched as whole words,
/// ignoring case: report 13 found `ak2` to `ak6` of its Cash-Karp step in `SMLib`'s integrator;
/// `htry`, `hdid`, `hnext` and `yscal` are the step driver's arguments in the same code.
pub const NR_VARIABLES: [&str; 9] = [
    "ak2", "ak3", "ak4", "ak5", "ak6", "htry", "hdid", "hnext", "yscal",
];

/// Phrases naming Numerical Recipes or its authors, as runs of words, ignoring case: report 13
/// found `SMLib`'s integrator "taken from Numerical Recipies in C", misspelt; Teukolsky and
/// Vetterling are two of the book's authors, whose names no other source of the kernel shares.
pub const NR_PHRASES: [&[&str]; 4] = [
    &["numerical", "recipes"],
    &["numerical", "recipies"],
    &["teukolsky"],
    &["vetterling"],
];

/// Numerical Recipes' own step-control constant `ERRCON`, 1.89 × 10^−4, as a value however the
/// literal writes it: rule 7, "NR's own step-control set, with ERRCON 1.89e-4, is flagged for
/// review". The safety factor 0.9 and the exponents −1/(q+1) are standard in the literature
/// (rule 7), so they are not markers.
pub const NR_VALUES: [f64; 1] = [1.89e-4];

/// Phrases of a copyleft licence's notice, as runs of words, ignoring case: the words with which
/// GPL, LGPL and AGPL code announces its licence in each file's header. Code that carries them was
/// copied with its notice (29.2: no GPL, LGPL or AGPL code is copied into Spare Kit).
pub const COPYLEFT_NOTICES: [&[&str]; 5] = [
    &["free", "software", "foundation"],
    &["gnu", "general", "public", "license"],
    &["gnu", "lesser", "general", "public", "license"],
    &["gnu", "library", "general", "public", "license"],
    &["gnu", "affero", "general", "public", "license"],
];

/// Phrases saying code was taken from somewhere, as runs of words, ignoring case: rule 2's signs
/// of trouble, comments saying code was "taken from", "based on", "adapted from" or "mirrors"
/// another work, and the other ways of saying it.
pub const TAKING: [&[&str]; 18] = [
    &["taken", "from"],
    &["adapted", "from"],
    &["ported", "from"],
    &["translated", "from"],
    &["transcribed", "from"],
    &["derived", "from"],
    &["copied", "from"],
    &["lifted", "from"],
    &["borrowed", "from"],
    &["modified", "from"],
    &["extracted", "from"],
    &["based", "on"],
    &["based", "upon"],
    &["port", "of"],
    &["translation", "of"],
    &["copy", "of"],
    &["mirrors"],
    &["mirroring"],
];

/// How many words after the first word of a two-word taking phrase its second may come, so words
/// put between them ("taken, with changes, from") do not hide it.
pub const TAKING_GAP: usize = 4;

/// How many words after a taking phrase a copyleft source's name may come.
pub const WINDOW: usize = 12;

/// How many words before a taking phrase, in its clause, a negation cancels it ("no code is taken
/// from").
pub const NEGATION_REACH: usize = 4;

/// The words that cancel a taking phrase just after them.
pub const NEGATIONS: [&str; 7] = ["no", "not", "never", "nothing", "none", "nor", "without"];

/// The words that, right after a negation, keep it from cancelling ("not only taken from", "not
/// entirely taken from" still say something was taken).
pub const NEGATION_LIMITS: [&str; 7] = [
    "only", "just", "merely", "simply", "entirely", "all", "wholly",
];

/// Names of copyleft licences and of the copyleft or otherwise banned sources chapter 29 lists, as
/// runs of words, ignoring case: the licences 29.2 never lets us take (GPL, LGPL, AGPL, EUPL,
/// CDDL, `PolyForm`, Numerical Recipes), and the projects chapter 29 classes as LGPL or GPL
/// (method only or papers only) or rejects for their licence or provenance: `OpenCascade`, CGAL,
/// `FreeCAD` and its planegcs, `LibreCAD`, `LibreDWG`, acadrust, `ACadSharp`, the ODA (Open Design
/// Alliance) specification and `OpenSKP`, `SolveSpace` with `libslvs` and `rust_slvs`, SISL and
/// `GoTools`, `TetGen`, `TetWild` and `fTetWild`, GEOS, `IfcOpenShell`, Topologic, Xbim, BRL-CAD,
/// Blender, `KiCad`, `OpenSCAD`, val3dity, `LNLib`, Attene's `Indirect_Predicates` and nfg, GMP,
/// MPFR and malachite, mcut, Cork, `MeshFix`, vcglib and `MeshLab`, Gmsh, Netgen and Mmg,
/// `VolumeRemesher`, `ImplicitCAD`, Sverchok, polyskel, surfer2, `CloudCompare`, chili3d, brepkit,
/// Aetheris, `CuraEngine`, `PrusaSlicer`, `OpenCAMLib`, Creusot, `HLBFGS`, IRIT, trueform and
/// `MeshLib`, the Adobe Research repositories (mtetcol, mtet), `contour_tracing`, openbimrs,
/// `JSketcher`, `CADmium`, cadcore, PCO and Dune 3D, and the unlicensed `NoteCAD`, `GeoSolver`,
/// `OffsetCrust`, clothoid-rs and raybevel. Chapter 29 is the list to keep this in step with: a
/// new row that bans a source's code adds its name here, unless the name is an ordinary word
/// (module header); piece G16's attack tests read the chapter's copyleft, non-commercial and
/// unlicensed rows and fail on a name the scan does not know.
pub const COPYLEFT_SOURCES: [&[&str]; 94] = [
    &["gpl"],
    &["lgpl"],
    &["agpl"],
    &["gplv2"],
    &["gplv3"],
    &["lgplv2"],
    &["lgplv3"],
    &["agplv3"],
    &["gnu"],
    &["copyleft"],
    &["eupl"],
    &["cddl"],
    &["numerical", "recipes"],
    &["occt"],
    &["opencascade"],
    &["open", "cascade"],
    &["cgal"],
    &["freecad"],
    &["planegcs"],
    &["librecad"],
    &["libredwg"],
    &["acadrust"],
    &["solvespace"],
    &["libslvs"],
    &["sisl"],
    &["gotools"],
    &["tetgen"],
    &["geos"],
    &["ifcopenshell"],
    &["topologic"],
    &["topologicpy"],
    &["brl", "cad"],
    &["blender"],
    &["kicad"],
    &["openscad"],
    &["val3dity"],
    &["lnlib"],
    &["attene"],
    &["indirect_predicates"],
    &["gmp"],
    &["mpfr"],
    &["malachite"],
    &["chili3d"],
    &["brepkit"],
    &["aetheris"],
    &["curaengine"],
    &["prusaslicer"],
    &["opencamlib"],
    &["creusot"],
    &["hlbfgs"],
    &["irit"],
    &["dune", "3d"],
    &["polyform"],
    &["acadsharp"],
    &["oda"],
    &["open", "design", "alliance"],
    &["openskp"],
    &["rust_slvs"],
    &["tetwild"],
    &["ftetwild"],
    &["xbim"],
    &["xbimessentials"],
    &["xbimgeometry"],
    &["nfg"],
    &["mcut"],
    &["cork"],
    &["meshfix"],
    &["vcglib"],
    &["meshlab"],
    &["gmsh"],
    &["netgen"],
    &["mmg"],
    &["volumeremesher"],
    &["implicitcad"],
    &["sverchok"],
    &["polyskel"],
    &["surfer2"],
    &["cloudcompare"],
    &["trueform"],
    &["meshlib"],
    &["mtetcol"],
    &["adobe", "research"],
    &["contour_tracing"],
    &["openbimrs"],
    &["jsketcher"],
    &["cadmium"],
    &["cadcore"],
    &["mtet"],
    &["pco"],
    &["notecad"],
    &["geosolver"],
    &["offsetcrust"],
    &["clothoid", "rs"],
    &["raybevel"],
];

/// Names of Autodesk's standard hatch patterns, matched as terms (letters, digits, `_` and `-`),
/// ignoring case and reading `_` as `-`: the ANSI and AR families of the standard pattern file, which evidence report 12
/// found reproduced, with Autodesk's descriptions, in `CADCraft`'s "our own" library; `ANSI31` and
/// AR-B816 are shown on Autodesk's own help pages (module header), and the other names of the two
/// families come from widely published lists of the same file (not checked against an Autodesk
/// page). Rule 7: no hatch patterns or linetypes from memory.
pub const ACAD_PATTERNS: [&str; 19] = [
    "ANSI31", "ANSI32", "ANSI33", "ANSI34", "ANSI35", "ANSI36", "ANSI37", "ANSI38", "AR-B816",
    "AR-B816C", "AR-B88", "AR-BRELM", "AR-BRSTD", "AR-CONC", "AR-HBONE", "AR-PARQ1", "AR-RROOF",
    "AR-RSHKE", "AR-SAND",
];

/// The ISO linetype and pattern names of Autodesk's metric files: `ACAD_ISO` followed by two
/// digits and `W100`, ignoring case, as a whole word.
pub const ACAD_ISO: (&str, &str) = ("acad_iso", "w100");

/// Descriptions from Autodesk's standard pattern and linetype files that evidence report 12 quotes
/// as found in `CADCraft`, matched as text, ignoring case, with every run of white space (a line
/// break and a comment marker's indentation included) read as one space.
pub const ACAD_DESCRIPTIONS: [&str; 6] = [
    "Iron, brick, stone masonry",
    "Staircase effect",
    "Random dot and stone pattern",
    "Fenceline circle",
    "Zig zag /\\/\\/",
    "Border __ __ . __",
];
