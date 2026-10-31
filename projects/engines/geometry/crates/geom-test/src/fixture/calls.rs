//! The operations a log can hold, with their parameters as the log writes them (chapter 28.16;
//! pieces G97, G101 and G116).
//!
//! What it does: a [`Call`] is one replayable operation of the kernel: its name, its version and
//! its inputs in their own types. [`Call::params`] writes the operation record's `params`, one item
//! of SKG's canonical CBOR: an array of the fields of the operation's schema, in the schema's
//! order, without the bodies it takes, which the record's `inputs` carry. [`Call::read`] reads them
//! back, and [`Call::run`] runs the operation.
//!
//! | Value | Encoding |
//! | --- | --- |
//! | a point | `[x, y, z]`, in grid steps |
//! | a length | an unsigned integer, in grid steps |
//! | a grid step | `Q0` 0, `Q1` 1, `Q2` 2 |
//! | an axis | `X` 0, `Y` 1, `Z` 2 |
//! | an exactness | `[0]`, `[1, bound, step]` or `[2, deviation, step]` |
//! | what a caller accepts | `[]`, or `[exactness]` |
//! | a rotation | `[0, quarter turns]`, or `[1, cos, sin]` |
//! | a scale factor | `[num, den]` |
//! | a pattern's copy | `[key, 0, offset]`, `[key, 1, axis, rotation]`, `[key, 2, axis]` or `[key, 3, factor]` |
//!
//! Guarantees:
//!
//! - **One writing.** [`Call::read`] writes what it read and refuses anything that does not come
//!   back byte for byte, so a log has one spelling of every call.
//! - **An unknown operation is refused, never guessed**: a name or version that [`Call::read`] does
//!   not know is an error, which the kernel turns into a refusal no fixture status accepts.
//!
//! Method: the operation record of chapter 24.4 and format/SPEC.md, with the fields of each
//! operation's schema in `engine_geometry::operations`
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: linear in the parameters, to write and to read. Running is the operation's cost.

use geom_format::payload::{PayloadError, Reader};
use geom_make::{
    BoxInput, Copies, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::grid::P3;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::cbor::{EncodeError, Encoder};
use geom_op::{OpContext, OpError, OpOutput};
use geom_run::{Budget, WorkMeter};
use geom_topo::Body;

/// A rotation as a log writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// Quarter turns.
    Quarter(i32),
    /// The direction `(cos, sin)`, in whole numbers.
    Direction(i32, i32),
}

impl Turn {
    /// The rotation; `None` for a direction of length zero.
    #[must_use]
    pub fn rotation(self) -> Option<Rotation> {
        match self {
            Turn::Quarter(turns) => Some(Rotation::quarter_turns(turns)),
            Turn::Direction(cos, sin) => Rotation::direction(cos, sin).ok(),
        }
    }
}

/// What a transform says about the body it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    /// The body's exactness record.
    pub exactness: Exactness,
    /// The class and bound the caller accepts for a result less exact than the body.
    pub accept: Option<Exactness>,
}

/// Where a pattern's copy goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A move by whole steps.
    Move([i64; 3]),
    /// A rotation about an axis through the origin.
    Rotate(Axis, Turn),
    /// A mirror across the plane through the origin.
    Mirror(Axis),
    /// A uniform scale about the origin, `(num, den)`.
    Scale(u64, u64),
}

impl Place {
    /// The transform; `None` for a rotation or a scale that does not exist.
    #[must_use]
    pub fn transform(self) -> Option<TypedTransform> {
        match self {
            Place::Move(offset) => GridTransform::translation(offset)
                .ok()
                .map(TypedTransform::Exact),
            Place::Rotate(axis, turn) => TypedTransform::rotation(axis, turn.rotation()?).ok(),
            Place::Mirror(axis) => Some(TypedTransform::Exact(GridTransform::mirror(axis))),
            Place::Scale(num, den) => RationalTransform::uniform_scale(Scale::new(num, den).ok()?)
                .ok()
                .map(TypedTransform::from),
        }
    }
}

/// One replayable operation of the kernel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// `make.box`.
    Box(BoxInput),
    /// `make.prism`.
    Prism(PrismInput),
    /// `make.cylinder`.
    Cylinder(RoundInput),
    /// `make.cone`.
    Cone(RoundInput),
    /// `make.sphere`.
    Sphere(SphereInput),
    /// `make.torus`.
    Torus(TorusInput),
    /// `transform.move`.
    Move(Input, [i64; 3]),
    /// `transform.rotate`.
    Rotate(Input, Axis, Turn, P3),
    /// `transform.mirror`.
    Mirror(Input, Axis, P3),
    /// `transform.scale`, each factor `(num, den)`.
    Scale(Input, [(u64, u64); 3], P3),
    /// `transform.pattern`.
    Pattern(Input, Vec<([u8; 16], Place)>),
}

/// What an operation gave: one body per result.
pub type Answer = Result<OpOutput<Vec<Made>>, OpError>;

/// Why a call could not be written or read.
#[derive(Clone, Debug, PartialEq)]
pub enum CallError {
    /// A name or a version the table does not hold.
    Unknown,
    /// The parameters' shape, or a value out of its type's range.
    Shape(PayloadError),
    /// A value no input can hold.
    Value(&'static str),
    /// Parameters that are not this call's one writing.
    NotCanonical,
    /// The encoder refused; only a defect in this module can cause it.
    Encode(EncodeError),
}

impl From<PayloadError> for CallError {
    fn from(e: PayloadError) -> Self {
        CallError::Shape(e)
    }
}

impl From<EncodeError> for CallError {
    fn from(e: EncodeError) -> Self {
        CallError::Encode(e)
    }
}

/// The operations' names, each at the version this table replays.
pub const OPERATIONS: [(&str, u64); 11] = [
    ("make.box", 1),
    ("make.cone", 1),
    ("make.cylinder", 1),
    ("make.prism", 1),
    ("make.sphere", 1),
    ("make.torus", 1),
    ("transform.mirror", 1),
    ("transform.move", 1),
    ("transform.pattern", 1),
    ("transform.rotate", 1),
    ("transform.scale", 1),
];

fn point(e: &mut Encoder, p: P3) -> Result<(), EncodeError> {
    e.array(3)?;
    e.int(i128::from(p.x.get()))?;
    e.int(i128::from(p.y.get()))?;
    e.int(i128::from(p.z.get()))
}

const fn step_code(step: GridStep) -> u64 {
    match step {
        GridStep::Q0 => 0,
        GridStep::Q1 => 1,
        GridStep::Q2 => 2,
    }
}

const fn axis_code(axis: Axis) -> u64 {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn exactness(e: &mut Encoder, x: Exactness) -> Result<(), EncodeError> {
    match x {
        Exactness::Exact => {
            e.array(1)?;
            e.uint(0)
        }
        Exactness::Certified { bound, step } => {
            e.array(3)?;
            e.uint(1)?;
            e.uint(bound.get())?;
            e.uint(step_code(step))
        }
        Exactness::Approximate { deviation, step } => {
            e.array(3)?;
            e.uint(2)?;
            e.uint(deviation.get())?;
            e.uint(step_code(step))
        }
    }
}

fn input(e: &mut Encoder, x: Input) -> Result<(), EncodeError> {
    exactness(e, x.exactness)?;
    match x.accept {
        Some(accept) => {
            e.array(1)?;
            exactness(e, accept)
        }
        None => e.array(0),
    }
}

fn turn(e: &mut Encoder, t: Turn) -> Result<(), EncodeError> {
    match t {
        Turn::Quarter(turns) => {
            e.array(2)?;
            e.uint(0)?;
            e.int(i128::from(turns))
        }
        Turn::Direction(cos, sin) => {
            e.array(3)?;
            e.uint(1)?;
            e.int(i128::from(cos))?;
            e.int(i128::from(sin))
        }
    }
}

fn factor(e: &mut Encoder, (num, den): (u64, u64)) -> Result<(), EncodeError> {
    e.array(2)?;
    e.uint(num)?;
    e.uint(den)
}

fn place(e: &mut Encoder, key: &[u8; 16], p: Place) -> Result<(), EncodeError> {
    match p {
        Place::Move(offset) => {
            e.array(3)?;
            e.bytes(key)?;
            e.uint(0)?;
            e.array(3)?;
            for v in offset {
                e.int(i128::from(v))?;
            }
            Ok(())
        }
        Place::Rotate(axis, t) => {
            e.array(4)?;
            e.bytes(key)?;
            e.uint(1)?;
            e.uint(axis_code(axis))?;
            turn(e, t)
        }
        Place::Mirror(axis) => {
            e.array(3)?;
            e.bytes(key)?;
            e.uint(2)?;
            e.uint(axis_code(axis))
        }
        Place::Scale(num, den) => {
            e.array(3)?;
            e.bytes(key)?;
            e.uint(3)?;
            factor(e, (num, den))
        }
    }
}

const fn expected(what: &'static str) -> PayloadError {
    PayloadError::Shape {
        offset: 0,
        expected: what,
    }
}

/// Reads from a log's parameters.
struct Params<'a, 'm, 'b> {
    r: Reader<'a>,
    meter: &'b mut WorkMeter<'m>,
}

impl Params<'_, '_, '_> {
    fn array(&mut self, what: &'static str) -> Result<u64, CallError> {
        Ok(self.r.array(self.meter, what)?)
    }

    fn fields(&mut self, count: u64, what: &'static str) -> Result<(), CallError> {
        if self.array(what)? == count {
            Ok(())
        } else {
            Err(CallError::Shape(expected(what)))
        }
    }

    fn uint(&mut self, what: &'static str) -> Result<u64, CallError> {
        Ok(self.r.uint(self.meter, what)?)
    }

    fn int(&mut self, what: &'static str) -> Result<i64, CallError> {
        use geom_format::cbor::Item;
        let value = match self.r.next(self.meter)? {
            Item::Uint(n) => i128::from(n),
            Item::Nint(n) => (-1i128).saturating_sub(i128::from(n)),
            _ => return Err(CallError::Shape(expected(what))),
        };
        i64::try_from(value).map_err(|_| CallError::Value(what))
    }

    fn i32(&mut self, what: &'static str) -> Result<i32, CallError> {
        i32::try_from(self.int(what)?).map_err(|_| CallError::Value(what))
    }

    fn point(&mut self) -> Result<P3, CallError> {
        self.fields(3, "a point")?;
        let (x, y, z) = (self.int("x")?, self.int("y")?, self.int("z")?);
        P3::new(x, y, z).map_err(|_| CallError::Value("a point past the grid's range"))
    }

    fn length(&mut self) -> Result<Steps, CallError> {
        Ok(Steps(self.uint("a length")?))
    }

    fn step(&mut self) -> Result<GridStep, CallError> {
        match self.uint("a grid step")? {
            0 => Ok(GridStep::Q0),
            1 => Ok(GridStep::Q1),
            2 => Ok(GridStep::Q2),
            _ => Err(CallError::Value("a grid step")),
        }
    }

    fn axis(&mut self) -> Result<Axis, CallError> {
        match self.uint("an axis")? {
            0 => Ok(Axis::X),
            1 => Ok(Axis::Y),
            2 => Ok(Axis::Z),
            _ => Err(CallError::Value("an axis")),
        }
    }

    fn exactness(&mut self) -> Result<Exactness, CallError> {
        let count = self.array("an exactness")?;
        match (self.uint("an exactness class")?, count) {
            (0, 1) => Ok(Exactness::Exact),
            (1, 3) => Ok(Exactness::Certified {
                bound: self.length()?,
                step: self.step()?,
            }),
            (2, 3) => Ok(Exactness::Approximate {
                deviation: self.length()?,
                step: self.step()?,
            }),
            _ => Err(CallError::Value("an exactness")),
        }
    }

    fn input(&mut self) -> Result<Input, CallError> {
        let exactness = self.exactness()?;
        let accept = match self.array("what the caller accepts")? {
            0 => None,
            1 => Some(self.exactness()?),
            _ => return Err(CallError::Value("what the caller accepts")),
        };
        Ok(Input { exactness, accept })
    }

    fn turn(&mut self) -> Result<Turn, CallError> {
        let count = self.array("a rotation")?;
        match (self.uint("a rotation's form")?, count) {
            (0, 2) => Ok(Turn::Quarter(self.i32("quarter turns")?)),
            (1, 3) => Ok(Turn::Direction(self.i32("a cosine")?, self.i32("a sine")?)),
            _ => Err(CallError::Value("a rotation")),
        }
    }

    fn factor(&mut self) -> Result<(u64, u64), CallError> {
        self.fields(2, "a scale factor")?;
        Ok((self.uint("a numerator")?, self.uint("a denominator")?))
    }

    fn offset(&mut self) -> Result<[i64; 3], CallError> {
        self.fields(3, "an offset")?;
        Ok([self.int("x")?, self.int("y")?, self.int("z")?])
    }

    fn copy(&mut self) -> Result<([u8; 16], Place), CallError> {
        let count = self.array("a copy")?;
        let key = self.r.bytes(self.meter, "a copy's key")?;
        let key: [u8; 16] = key
            .try_into()
            .map_err(|_| CallError::Value("a copy's key"))?;
        let place = match (self.uint("a copy's form")?, count) {
            (0, 3) => Place::Move(self.offset()?),
            (1, 4) => Place::Rotate(self.axis()?, self.turn()?),
            (2, 3) => Place::Mirror(self.axis()?),
            (3, 3) => {
                let (num, den) = self.factor()?;
                Place::Scale(num, den)
            }
            _ => return Err(CallError::Value("a copy")),
        };
        Ok((key, place))
    }
}

impl Call {
    /// The operation's name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Call::Box(_) => "make.box",
            Call::Prism(_) => "make.prism",
            Call::Cylinder(_) => "make.cylinder",
            Call::Cone(_) => "make.cone",
            Call::Sphere(_) => "make.sphere",
            Call::Torus(_) => "make.torus",
            Call::Move(..) => "transform.move",
            Call::Rotate(..) => "transform.rotate",
            Call::Mirror(..) => "transform.mirror",
            Call::Scale(..) => "transform.scale",
            Call::Pattern(..) => "transform.pattern",
        }
    }

    /// Whether it takes a body.
    #[must_use]
    pub const fn takes_a_body(&self) -> bool {
        matches!(
            self,
            Call::Move(..)
                | Call::Rotate(..)
                | Call::Mirror(..)
                | Call::Scale(..)
                | Call::Pattern(..)
        )
    }

    /// The record's `params`.
    ///
    /// # Errors
    ///
    /// The encoder's: only a defect, or a prism's corners past 2^64, can cause it.
    pub fn params(&self) -> Result<Vec<u8>, EncodeError> {
        let mut e = Encoder::new();
        match self {
            Call::Box(b) => {
                e.array(2)?;
                point(&mut e, b.min)?;
                e.array(3)?;
                for s in b.size {
                    e.uint(s.get())?;
                }
            }
            Call::Prism(p) => {
                e.array(2)?;
                e.array(u64::try_from(p.base.len()).map_err(|_| EncodeError::TooLong)?)?;
                for corner in &p.base {
                    point(&mut e, *corner)?;
                }
                e.uint(p.height.get())?;
            }
            Call::Cylinder(r) | Call::Cone(r) => {
                e.array(4)?;
                point(&mut e, r.base)?;
                e.uint(r.radius.get())?;
                e.uint(r.height.get())?;
                e.uint(step_code(r.step))?;
            }
            Call::Sphere(s) => {
                e.array(3)?;
                point(&mut e, s.center)?;
                e.uint(s.radius.get())?;
                e.uint(step_code(s.step))?;
            }
            Call::Torus(t) => {
                e.array(4)?;
                point(&mut e, t.center)?;
                e.uint(t.major.get())?;
                e.uint(t.minor.get())?;
                e.uint(step_code(t.step))?;
            }
            Call::Move(i, offset) => {
                e.array(3)?;
                input(&mut e, *i)?;
                e.array(3)?;
                for v in offset {
                    e.int(i128::from(*v))?;
                }
            }
            Call::Rotate(i, axis, t, center) => {
                e.array(5)?;
                input(&mut e, *i)?;
                e.uint(axis_code(*axis))?;
                turn(&mut e, *t)?;
                point(&mut e, *center)?;
            }
            Call::Mirror(i, axis, at) => {
                e.array(4)?;
                input(&mut e, *i)?;
                e.uint(axis_code(*axis))?;
                point(&mut e, *at)?;
            }
            Call::Scale(i, factors, center) => {
                e.array(4)?;
                input(&mut e, *i)?;
                e.array(3)?;
                for f in factors {
                    factor(&mut e, *f)?;
                }
                point(&mut e, *center)?;
            }
            Call::Pattern(i, copies) => {
                e.array(3)?;
                input(&mut e, *i)?;
                e.array(u64::try_from(copies.len()).map_err(|_| EncodeError::TooLong)?)?;
                for (key, p) in copies {
                    place(&mut e, key, *p)?;
                }
            }
        }
        Ok(e.finish()?.into_bytes())
    }

    /// The call a record's `name`, `version` and `params` write.
    ///
    /// # Errors
    ///
    /// [`CallError::Unknown`] for a name or version this table does not hold, and the other
    /// [`CallError`]s for parameters that are not its one writing.
    pub fn read(name: &str, version: u64, params: &[u8]) -> Result<Call, CallError> {
        if !OPERATIONS.contains(&(name, version)) {
            return Err(CallError::Unknown);
        }
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mut p = Params {
            r: Reader::new(params),
            meter: &mut meter,
        };
        let call = match name {
            "make.box" => {
                p.fields(2, "make.box")?;
                let min = p.point()?;
                p.fields(3, "sizes")?;
                let size = [p.length()?, p.length()?, p.length()?];
                Call::Box(BoxInput { min, size })
            }
            "make.prism" => {
                p.fields(2, "make.prism")?;
                let corners = p.array("a base")?;
                let mut base = Vec::new();
                for _ in 0..corners {
                    base.push(p.point()?);
                }
                let height = p.length()?;
                Call::Prism(PrismInput { base, height })
            }
            "make.cylinder" | "make.cone" => {
                p.fields(4, "a round primitive")?;
                let input = RoundInput {
                    base: p.point()?,
                    radius: p.length()?,
                    height: p.length()?,
                    step: p.step()?,
                };
                if name == "make.cone" {
                    Call::Cone(input)
                } else {
                    Call::Cylinder(input)
                }
            }
            "make.sphere" => {
                p.fields(3, "make.sphere")?;
                Call::Sphere(SphereInput {
                    center: p.point()?,
                    radius: p.length()?,
                    step: p.step()?,
                })
            }
            "make.torus" => {
                p.fields(4, "make.torus")?;
                Call::Torus(TorusInput {
                    center: p.point()?,
                    major: p.length()?,
                    minor: p.length()?,
                    step: p.step()?,
                })
            }
            "transform.move" => {
                p.fields(3, "transform.move")?;
                let input = p.input()?;
                Call::Move(input, p.offset()?)
            }
            "transform.rotate" => {
                p.fields(5, "transform.rotate")?;
                let input = p.input()?;
                let axis = p.axis()?;
                let turn = p.turn()?;
                Call::Rotate(input, axis, turn, p.point()?)
            }
            "transform.mirror" => {
                p.fields(4, "transform.mirror")?;
                let input = p.input()?;
                let axis = p.axis()?;
                Call::Mirror(input, axis, p.point()?)
            }
            "transform.scale" => {
                p.fields(4, "transform.scale")?;
                let input = p.input()?;
                p.fields(3, "scale factors")?;
                let factors = [p.factor()?, p.factor()?, p.factor()?];
                Call::Scale(input, factors, p.point()?)
            }
            "transform.pattern" => {
                p.fields(3, "transform.pattern")?;
                let input = p.input()?;
                let count = p.array("the copies")?;
                let mut copies = Vec::new();
                for _ in 0..count {
                    copies.push(p.copy()?);
                }
                Call::Pattern(input, copies)
            }
            _ => return Err(CallError::Unknown),
        };
        p.r.end()?;
        if call.params()? == params {
            Ok(call)
        } else {
            Err(CallError::NotCanonical)
        }
    }

    /// Runs the operation under `ctx`, on `body` for one that takes a body.
    ///
    /// # Errors
    ///
    /// The operation's own error. A transform given no body, or a rotation or a copy that does not
    /// exist, is `None`: no call the kernel could make.
    #[must_use]
    pub fn run(&self, ctx: &OpContext<'_>, body: Option<&Body>) -> Option<Answer> {
        let one = |out: Result<OpOutput<Made>, OpError>| -> Answer {
            out.map(|o| {
                let (exactness, diagnostics) = (o.exactness, o.diagnostics);
                let mut all = OpOutput::new(vec![o.result], exactness);
                for d in diagnostics {
                    all = all.with_diagnostic(d);
                }
                all
            })
        };
        let moved = |i: &Input| -> Option<Moved<'_>> {
            Some(Moved {
                body: body?,
                exactness: i.exactness,
                accept: i.accept,
            })
        };
        Some(match self {
            Call::Box(i) => one(geom_make::make_box(ctx, i)),
            Call::Prism(i) => one(geom_make::prism(ctx, i)),
            Call::Cylinder(i) => one(geom_make::cylinder(ctx, i)),
            Call::Cone(i) => one(geom_make::cone(ctx, i)),
            Call::Sphere(i) => one(geom_make::sphere(ctx, i)),
            Call::Torus(i) => one(geom_make::torus(ctx, i)),
            Call::Move(i, offset) => one(geom_make::move_by(ctx, moved(i)?, *offset)),
            Call::Rotate(i, axis, t, center) => one(geom_make::rotate(
                ctx,
                moved(i)?,
                *axis,
                t.rotation()?,
                *center,
            )),
            Call::Mirror(i, axis, at) => one(geom_make::mirror(ctx, moved(i)?, *axis, *at)),
            Call::Scale(i, factors, center) => {
                let f = |(num, den): (u64, u64)| Scale::new(num, den).ok();
                let factors = [f(factors[0])?, f(factors[1])?, f(factors[2])?];
                one(geom_make::scale(ctx, moved(i)?, factors, *center))
            }
            Call::Pattern(i, copies) => {
                let instances: Option<Vec<Instance>> = copies
                    .iter()
                    .map(|(key, p)| {
                        Some(Instance {
                            key: *key,
                            transform: p.transform()?,
                        })
                    })
                    .collect();
                pattern(geom_make::pattern(ctx, moved(i)?, &instances?))
            }
        })
    }
}

/// A pattern's copies, in key order, as the results of one operation.
fn pattern(out: Result<OpOutput<Copies>, OpError>) -> Answer {
    out.map(|o| {
        let made: Vec<Made> = o.result.into_iter().map(|(_, made)| made).collect();
        OpOutput::new(made, o.exactness)
    })
}
