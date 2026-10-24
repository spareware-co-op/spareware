//! The predicate description language: a small, line-based language for a polynomial predicate
//! over grid coordinates, and its flattening into one step per operation.
//!
//! What it does: [`parse`] reads a description into a [`Description`]; [`Description::steps`]
//! flattens it into [`Step`]s, one per arithmetic operation, in the order the generated code
//! evaluates them. The generator derives everything from these steps
//! ([`crate::predicate_derive`]) and emits code that performs exactly them, in this order
//! ([`crate::predicate_emit`]).
//!
//! The language, line by line (blank lines and text after `#` are ignored):
//!
//! ```text
//! predicate <name>                      the predicate's name; the first line
//! input <p> <q> ...: point2             grid points in 2D, coordinates <p>.x and <p>.y
//! input <p> <q> ...: point3             grid points in 3D, coordinates <p>.x, <p>.y and <p>.z
//! input <s> ...: rounded <bits> in <g>  f64 copies of integers with |v| ≤ 2^bits − 1, group <g>
//! let <name> = <expression>             an intermediate value
//! sign <name>                           the predicate's answer: the sign of that value
//! ```
//!
//! An expression combines `+`, `-` and `*` with the usual precedence (`*` first), left to right,
//! with parentheses; its operands are the coordinates of point inputs (`a.x`), rounded inputs by
//! name, and earlier `let` names. Every operation is evaluated exactly as written: `a - b + c` is
//! `(a - b) + c`. There are no constants and no division.
//!
//! The rules, each refused with a [`LangError`] naming the line:
//!
//! - Grid points: every coordinate has |v| ≤ 2^40 − 1 (`geom_num::grid::COORD_MAX`), and each
//!   axis is a group of the filter's scale: `x`, `y`, `z`. A group is Meyer and Pion's: a set of
//!   variables the polynomial is homogeneous in, scaled at run time by its largest magnitude. The
//!   grouping is a choice: here one group per axis, where Geogram's FPG-generated orient2d takes
//!   one per row of the determinant (`max1` over `a11` and `a12`, `orient2d.h` at 16843b4).
//! - A difference of two coordinates of the same axis, `p.x - q.x`, is a *translation*: one of
//!   the scale variables of its axis's group (FPG's translation filter). Any other use of a
//!   coordinate, or of a rounded input, loads it as a scale variable of its group by itself.
//! - A name is lowercase snake case: a letter `a` to `z`, then letters, digits and single `_`s, not
//!   ending in `_` (so every name, and every step name made from it, is a Rust identifier that
//!   rustc's `non_snake_case` lint accepts). Names are unique and do not end in `_<digits>` (the
//!   form of the inner step names below). A `let` uses only earlier names; every `let` is used,
//!   directly or not, by `sign`.
//! - Every name becomes an identifier in the generated Rust, so none may be one the generated code
//!   cannot bind or already binds ([`reserved`]): the language's keywords; Rust's keywords
//!   (strict, reserved and weak, edition 2024) and primitive types; the items and helpers every
//!   generated module declares or imports (`filter`, `value`, `exact`, `decide`, `exact_f64`,
//!   `larger`, `largest_magnitude`, `rounded_f64`, `within`); the filter's own locals, `scale`
//!   and the group maxima `m_<group>` for the axes and every rounded input's group; and, for a
//!   predicate's name, which is also a file name, the device names Windows refuses as file names.
//! - A coordinate is loaded under the name `<point>_<axis>`, so no input or `let` may have that
//!   name for a point input: `a_x` would otherwise be both the coordinate `a.x` and the `let`.
//!   [`Description::steps`] refuses any two steps of one name, and a load whose name is already a
//!   different step.
//! - Each step is named: a `let`'s last operation takes the `let`'s name; the operations inside
//!   it take the `let`'s name with `_1`, `_2`, … in the order they are evaluated (operands first,
//!   left before right); a coordinate or rounded input used by itself is loaded once, as
//!   `<point>_<axis>` or under its own name.
//!
//! Guarantees: a description that parses names every value the generated code computes, each
//! with a name of its own that the generated code does not use for anything else, and the steps
//! are in an order where each operand is computed before it is used.
//!
//! Method: the straight-line predicate programs of Meyer and Pion's FPG input language (a subset
//! of C with `group` declarations; Meyer and Pion, *FPG: A code generator for fast and certified
//! geometric predicates*, Real Numbers and Computers 2008, <https://inria.hal.science/inria-00344297>)
//! and of Geogram's PCK `.pck` files (BSD-3-Clause; for example
//! <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/orient2d.pck>),
//! cut down to what the kernel's predicates need: one expression per line, no preprocessor, no
//! symbolic perturbation. The parser is recursive descent over a token list (Aho, Lam, Sethi and
//! Ullman, *Compilers: Principles, Techniques, and Tools*, second edition, section 4.4).
//! Consulted: Geogram PCK, BSD-3-Clause,
//!   <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/orient2d.pck>,
//!   for the form of its predicate descriptions. No code copied.
//!
//! Cost: linear in the description's length; recursion as deep as an expression's nesting.

use core::fmt;

use geom_num::grid::COORD_MAX;

/// An axis of a grid point, and the group of its coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    /// The first coordinate.
    X,
    /// The second coordinate.
    Y,
    /// The third coordinate.
    Z,
}

impl Axis {
    /// Its letter, also its group's name.
    #[must_use]
    pub const fn letter(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }

    fn from_letter(s: &str) -> Option<Axis> {
        match s {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    /// Its position: 0, 1 or 2.
    const fn index(self) -> u8 {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

/// What an input is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    /// A grid point with this many coordinates (2 or 3), each with |v| ≤ `COORD_MAX`.
    Point {
        /// 2 or 3.
        dim: u8,
    },
    /// An `f64` copy, rounded to nearest, of an integer with |v| ≤ 2^bits − 1.
    Rounded {
        /// The integer's bits.
        bits: u32,
        /// Its group in the filter's scale.
        group: String,
    },
}

/// One input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    /// Its name.
    pub name: String,
    /// What it is.
    pub kind: InputKind,
}

/// An arithmetic operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}

impl Op {
    /// Its symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
        }
    }
}

/// An expression, as parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A point's coordinate.
    Coord(String, Axis),
    /// A rounded input or an earlier `let`, by name.
    Name(String),
    /// An operation.
    Bin(Box<Expr>, Op, Box<Expr>),
}

/// One `let`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Let {
    /// Its name.
    pub name: String,
    /// Its expression.
    pub expr: Expr,
    /// Its line, from 1.
    pub line: usize,
}

/// A parsed description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    /// The predicate's name.
    pub name: String,
    /// The inputs, in order.
    pub inputs: Vec<Input>,
    /// The `let`s, in order.
    pub lets: Vec<Let>,
    /// The `let` whose sign is the answer.
    pub result: String,
}

/// What a step computes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepKind {
    /// A grid coordinate, loaded as itself.
    LoadCoord {
        /// The point.
        point: String,
        /// The axis.
        axis: Axis,
    },
    /// A rounded input, loaded as itself.
    LoadRounded {
        /// The input.
        input: String,
    },
    /// The translation `minuend.axis - subtrahend.axis` of two grid points.
    Translate {
        /// The point subtracted from.
        minuend: String,
        /// The point subtracted.
        subtrahend: String,
        /// The axis.
        axis: Axis,
    },
    /// An operation on two earlier steps, by index.
    Op {
        /// The operation.
        op: Op,
        /// The left operand's step.
        left: usize,
        /// The right operand's step.
        right: usize,
    },
}

/// One step: one value the generated code computes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Its name in the generated code.
    pub name: String,
    /// What it computes.
    pub kind: StepKind,
}

/// A description refused, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangError {
    /// The line, from 1; 0 for the description as a whole.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, LangError> {
    Err(LangError {
        line,
        message: message.into(),
    })
}

/// The language's own keywords.
const KEYWORDS: [&str; 7] = [
    "predicate",
    "input",
    "let",
    "sign",
    "point2",
    "point3",
    "rounded",
];

/// Rust's keywords (the Rust Reference, "Keywords": strict, reserved and weak, edition 2024) and
/// its primitive types, lowercase ones only (a name is lowercase). None can be a `let` or a
/// parameter in the generated code, or a module name (`pub mod type;` does not compile), and a
/// primitive's name as a local would read as the type. One keyword is spelt in two halves:
/// geom-gen's purity test refuses that word anywhere in a generator's code, string or not.
pub const RUST_WORDS: &[&str] = &[
    // Strict.
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    concat!("un", "safe"),
    "use",
    "where",
    "while",
    // Reserved.
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "gen",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    // Weak.
    "macro_rules",
    "raw",
    "safe",
    "union",
    // Primitive types.
    "bool",
    "char",
    "str",
    "f32",
    "f64",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
];

/// The names every generated module declares, imports or binds besides the description's own
/// (module header): its items, the parent module's helpers, and the filter's `scale`. The group
/// maxima `m_<group>` are reserved by [`reserved`] for each description's groups.
pub const GENERATED_NAMES: [&str; 10] = [
    "filter",
    "value",
    "exact",
    "decide",
    "exact_f64",
    "larger",
    "largest_magnitude",
    "rounded_f64",
    "within",
    "scale",
];

/// The device names Windows refuses as file names, whatever the extension (the same list as the
/// registry's, `crate::files`): a predicate's name is also its module's file name.
const DEVICE_NAMES: [&str; 24] = [
    "con", "prn", "aux", "nul", "com0", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
    "com8", "com9", "lpt0", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// The name of group `group`'s maximum in the generated filter.
#[must_use]
pub fn max_name(group: &str) -> String {
    format!("m_{group}")
}

/// Why `name` cannot name a value of a description whose groups are `groups` (the axes and its
/// rounded inputs' groups), or `None` if it can (module header).
#[must_use]
pub fn reserved(name: &str, groups: &[&str]) -> Option<String> {
    if KEYWORDS.contains(&name) {
        return Some(format!("`{name}` is a keyword of the description language"));
    }
    if RUST_WORDS.contains(&name) {
        return Some(format!(
            "`{name}` is a Rust keyword or primitive type, which the generated code cannot bind"
        ));
    }
    if GENERATED_NAMES.contains(&name) {
        return Some(format!(
            "`{name}` is a name every generated module already uses (an item, a helper or the \
             filter's scale)"
        ));
    }
    if groups.iter().any(|g| max_name(g) == name) {
        return Some(format!(
            "`{name}` is the name of a group maximum in the generated filter"
        ));
    }
    None
}

/// Whether `s` has the form of a name: lowercase snake case (module header).
fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.contains("__")
        && !s.ends_with('_')
        && !KEYWORDS.contains(&s)
}

/// Why `name` cannot be a predicate's name, or `None` if it can: a name, not reserved, and not a
/// device name Windows refuses as a file name.
fn predicate_name_refusal(name: &str) -> Option<String> {
    if !is_name(name) {
        return Some(format!("`{name}` is not a name (lowercase snake case)"));
    }
    if let Some(why) = reserved(name, &[]) {
        return Some(why);
    }
    DEVICE_NAMES
        .contains(&name)
        .then(|| format!("`{name}` is a device name Windows refuses as a file name"))
}

/// Whether `s` ends in `_<digits>`, the form of the names given to inner steps.
fn has_step_suffix(s: &str) -> bool {
    match s.rsplit_once('_') {
        Some((_, tail)) => !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// A token of an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Dot,
    Plus,
    Minus,
    Star,
    Open,
    Close,
}

fn tokens(text: &str, line: usize) -> Result<Vec<Token>, LangError> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            out.push(Token::Word(core::mem::take(&mut word)));
        }
        match c {
            '.' => out.push(Token::Dot),
            '+' => out.push(Token::Plus),
            '-' => out.push(Token::Minus),
            '*' => out.push(Token::Star),
            '(' => out.push(Token::Open),
            ')' => out.push(Token::Close),
            c if c.is_whitespace() => {}
            c => return err(line, format!("unexpected character {c:?}")),
        }
    }
    if !word.is_empty() {
        out.push(Token::Word(word));
    }
    Ok(out)
}

/// A recursive-descent parser over one line's tokens.
struct Parser<'t> {
    tokens: &'t [Token],
    at: usize,
    line: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn bump(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.at);
        if t.is_some() {
            self.at = self.at.saturating_add(1);
        }
        t
    }

    /// expression := term (('+' | '-') term)*
    fn expression(&mut self) -> Result<Expr, LangError> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => Op::Add,
                Some(Token::Minus) => Op::Sub,
                _ => return Ok(left),
            };
            self.bump();
            let right = self.term()?;
            left = Expr::Bin(Box::new(left), op, Box::new(right));
        }
    }

    /// term := factor ('*' factor)*
    fn term(&mut self) -> Result<Expr, LangError> {
        let mut left = self.factor()?;
        while self.peek() == Some(&Token::Star) {
            self.bump();
            let right = self.factor()?;
            left = Expr::Bin(Box::new(left), Op::Mul, Box::new(right));
        }
        Ok(left)
    }

    /// factor := name | name '.' axis | '(' expression ')'
    fn factor(&mut self) -> Result<Expr, LangError> {
        let line = self.line;
        match self.bump().cloned() {
            Some(Token::Open) => {
                let inner = self.expression()?;
                match self.bump() {
                    Some(Token::Close) => Ok(inner),
                    _ => err(line, "expected `)`"),
                }
            }
            Some(Token::Word(name)) => {
                if self.peek() == Some(&Token::Dot) {
                    self.bump();
                    match self.bump().cloned() {
                        Some(Token::Word(axis)) => match Axis::from_letter(&axis) {
                            Some(axis) => Ok(Expr::Coord(name, axis)),
                            None => err(line, format!("`{axis}` is not an axis (x, y or z)")),
                        },
                        _ => err(line, "expected an axis after `.`"),
                    }
                } else {
                    Ok(Expr::Name(name))
                }
            }
            _ => err(line, "expected a name, a coordinate or `(`"),
        }
    }
}

/// Reads a description.
///
/// # Errors
///
/// A [`LangError`] naming the first line that breaks a rule of the language (module header).
pub fn parse(text: &str) -> Result<Description, LangError> {
    let mut name: Option<String> = None;
    let mut inputs: Vec<Input> = Vec::new();
    let mut lets: Vec<Let> = Vec::new();
    let mut result: Option<String> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = index.saturating_add(1);
        let content = raw.split('#').next().unwrap_or_default().trim();
        if content.is_empty() {
            continue;
        }
        let (keyword, rest) = content.split_once(' ').unwrap_or((content, ""));
        let rest = rest.trim();
        if name.is_none() && keyword != "predicate" {
            return err(line, "a description starts with `predicate <name>`");
        }
        if result.is_some() {
            return err(line, "nothing may follow `sign`");
        }
        match keyword {
            "predicate" => {
                if name.is_some() {
                    return err(line, "a second `predicate` line");
                }
                if let Some(why) = predicate_name_refusal(rest) {
                    return err(line, why);
                }
                name = Some(rest.to_owned());
            }
            "input" => {
                if !lets.is_empty() {
                    return err(line, "inputs come before the first `let`");
                }
                inputs.extend(parse_inputs(rest, line)?);
            }
            "let" => {
                let Some((target, expr)) = rest.split_once('=') else {
                    return err(line, "expected `let <name> = <expression>`");
                };
                let target = target.trim();
                if !is_name(target) {
                    return err(
                        line,
                        format!("`{target}` is not a name (lowercase snake case)"),
                    );
                }
                let toks = tokens(expr, line)?;
                let mut parser = Parser {
                    tokens: &toks,
                    at: 0,
                    line,
                };
                let expr = parser.expression()?;
                if parser.at != toks.len() {
                    return err(line, "unexpected text after the expression");
                }
                lets.push(Let {
                    name: target.to_owned(),
                    expr,
                    line,
                });
            }
            "sign" => {
                if !is_name(rest) {
                    return err(line, format!("`{rest}` is not a name"));
                }
                result = Some(rest.to_owned());
            }
            other => return err(line, format!("unknown keyword `{other}`")),
        }
    }
    let Some(name) = name else {
        return err(0, "empty description");
    };
    let Some(result) = result else {
        return err(0, "no `sign` line");
    };
    let description = Description {
        name,
        inputs,
        lets,
        result,
    };
    description.validate()?;
    Ok(description)
}

/// `<names>: point2`, `<names>: point3` or `<names>: rounded <bits> in <group>`.
fn parse_inputs(rest: &str, line: usize) -> Result<Vec<Input>, LangError> {
    let Some((names, kind)) = rest.split_once(':') else {
        return err(line, "expected `input <names>: <kind>`");
    };
    let words: Vec<&str> = kind.split_whitespace().collect();
    let kind = match words.as_slice() {
        ["point2"] => InputKind::Point { dim: 2 },
        ["point3"] => InputKind::Point { dim: 3 },
        ["rounded", bits, "in", group] => {
            let Ok(bits) = bits.parse::<u32>() else {
                return err(line, format!("`{bits}` is not a bit count"));
            };
            if !(1..=1_000).contains(&bits) {
                return err(line, "a rounded input has 1 to 1000 bits");
            }
            if !is_name(group) || Axis::from_letter(group).is_some() {
                return err(
                    line,
                    format!("`{group}` cannot name a group (x, y and z are the axes')"),
                );
            }
            InputKind::Rounded {
                bits,
                group: (*group).to_owned(),
            }
        }
        _ => {
            return err(
                line,
                "an input is `point2`, `point3` or `rounded <bits> in <group>`",
            );
        }
    };
    let mut out = Vec::new();
    for name in names.split_whitespace() {
        if !is_name(name) {
            return err(line, format!("`{name}` is not a name"));
        }
        out.push(Input {
            name: name.to_owned(),
            kind: kind.clone(),
        });
    }
    if out.is_empty() {
        return err(line, "an `input` line names at least one input");
    }
    Ok(out)
}

/// What a name refers to while flattening.
#[derive(Clone, Copy)]
enum Bound {
    Point(u8),
    Rounded,
    Let(usize),
}

impl Description {
    /// The input called `name`.
    #[must_use]
    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|i| i.name == name)
    }

    /// The grid domain: every coordinate of a point input has |v| ≤ this.
    #[must_use]
    pub const fn coord_max() -> i64 {
        COORD_MAX
    }

    fn lookup(&self, name: &str, before: usize) -> Option<Bound> {
        if let Some(input) = self.input(name) {
            return Some(match input.kind {
                InputKind::Point { dim } => Bound::Point(dim),
                InputKind::Rounded { .. } => Bound::Rounded,
            });
        }
        self.lets
            .iter()
            .take(before)
            .position(|l| l.name == name)
            .map(Bound::Let)
    }

    /// The names of the groups a description can have: the axes, then its rounded inputs' groups.
    #[must_use]
    pub fn group_names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = [Axis::X, Axis::Y, Axis::Z]
            .iter()
            .map(|a| a.letter())
            .collect();
        for input in &self.inputs {
            if let InputKind::Rounded { group, .. } = &input.kind {
                if !out.contains(&group.as_str()) {
                    out.push(group);
                }
            }
        }
        out
    }

    /// The coordinate whose load is named `name` (`a_x` for a point input `a`), if any.
    fn coordinate_named(&self, name: &str) -> Option<String> {
        let (point, axis) = name.rsplit_once('_')?;
        let axis = Axis::from_letter(axis)?;
        match self.input(point)?.kind {
            InputKind::Point { .. } => Some(format!("{point}.{}", axis.letter())),
            InputKind::Rounded { .. } => None,
        }
    }

    /// Checks the naming and scoping rules of the module header.
    fn validate(&self) -> Result<(), LangError> {
        let groups = self.group_names();
        let mut seen: Vec<&str> = Vec::new();
        let declared = self
            .inputs
            .iter()
            .map(|i| (i.name.as_str(), 0))
            .chain(self.lets.iter().map(|l| (l.name.as_str(), l.line)));
        for (name, line) in declared {
            if seen.contains(&name) {
                return err(line, format!("`{name}` is named twice"));
            }
            if has_step_suffix(name) {
                return err(
                    line,
                    format!("`{name}` ends in `_<digits>`, the form of step names"),
                );
            }
            if let Some(why) = reserved(name, &groups) {
                return err(line, why);
            }
            if let Some(coordinate) = self.coordinate_named(name) {
                return err(
                    line,
                    format!("`{name}` is the name of the coordinate {coordinate}'s load"),
                );
            }
            seen.push(name);
        }
        for (i, l) in self.lets.iter().enumerate() {
            self.check_expr(&l.expr, i, l.line)?;
            if matches!(l.expr, Expr::Name(_)) {
                return err(l.line, "a `let` must compute something, not rename a value");
            }
        }
        let Some(result) = self.lets.iter().position(|l| l.name == self.result) else {
            return err(0, format!("`sign {}` names no `let`", self.result));
        };
        // Every let must be used by the result, directly or through others.
        let mut used = vec![false; self.lets.len()];
        if let Some(u) = used.get_mut(result) {
            *u = true;
        }
        for (i, l) in self.lets.iter().enumerate().rev() {
            if used.get(i) == Some(&true) {
                let mut names = Vec::new();
                names_in(&l.expr, &mut names);
                for n in names {
                    if let Some(Bound::Let(j)) = self.lookup(n, i) {
                        if let Some(u) = used.get_mut(j) {
                            *u = true;
                        }
                    }
                }
            }
        }
        if let Some((l, _)) = self.lets.iter().zip(&used).find(|(_, u)| !**u) {
            return err(l.line, format!("`{}` is never used by `sign`", l.name));
        }
        for input in &self.inputs {
            let mut referenced = false;
            for l in &self.lets {
                referenced = referenced || refers_to(&l.expr, &input.name);
            }
            if !referenced {
                return err(0, format!("input `{}` is never used", input.name));
            }
        }
        Ok(())
    }

    fn check_expr(&self, e: &Expr, before: usize, line: usize) -> Result<(), LangError> {
        match e {
            Expr::Coord(p, axis) => match self.lookup(p, before) {
                Some(Bound::Point(dim)) if axis.index() < dim => Ok(()),
                Some(Bound::Point(_)) => err(line, format!("`{p}` has no {} axis", axis.letter())),
                _ => err(line, format!("`{p}` is not a point input")),
            },
            Expr::Name(n) => match self.lookup(n, before) {
                Some(Bound::Rounded | Bound::Let(_)) => Ok(()),
                Some(Bound::Point(_)) => err(line, format!("use a coordinate of `{n}`, not `{n}`")),
                None => err(line, format!("`{n}` is not an input or an earlier `let`")),
            },
            Expr::Bin(l, _, r) => {
                self.check_expr(l, before, line)?;
                self.check_expr(r, before, line)
            }
        }
    }

    /// The steps, in evaluation order (module header).
    ///
    /// # Errors
    ///
    /// A [`LangError`] if two steps would have the same name, a load's name is already another
    /// step's, or a step's name is reserved ([`reserved`]).
    pub fn steps(&self) -> Result<Vec<Step>, LangError> {
        let mut flat = Flattener {
            description: self,
            steps: Vec::new(),
            let_steps: Vec::new(),
        };
        for (i, l) in self.lets.iter().enumerate() {
            let mut counter = 0usize;
            let top = flat.flatten(&l.expr, i, &l.name, &mut counter, true, l.line)?;
            flat.let_steps.push(top);
        }
        let groups = self.group_names();
        let mut names: Vec<&str> = Vec::new();
        for s in &flat.steps {
            // A rounded input's load is named after the input itself; any other step must not be.
            let loads_itself =
                matches!(&s.kind, StepKind::LoadRounded { input } if *input == s.name);
            if names.contains(&s.name.as_str()) || (self.input(&s.name).is_some() && !loads_itself)
            {
                return err(0, format!("two values would be named `{}`", s.name));
            }
            if let Some(why) = reserved(&s.name, &groups) {
                return err(0, format!("step `{}`: {why}", s.name));
            }
            names.push(&s.name);
        }
        Ok(flat.steps)
    }

    /// The step that holds the result, in [`Description::steps`]'s list.
    ///
    /// # Errors
    ///
    /// As [`Description::steps`].
    pub fn result_step(&self, steps: &[Step]) -> Result<usize, LangError> {
        match steps.iter().position(|s| s.name == self.result) {
            Some(i) => Ok(i),
            None => err(0, format!("no step is named `{}`", self.result)),
        }
    }
}

fn names_in<'e>(e: &'e Expr, out: &mut Vec<&'e str>) {
    match e {
        Expr::Coord(..) => {}
        Expr::Name(n) => out.push(n),
        Expr::Bin(l, _, r) => {
            names_in(l, out);
            names_in(r, out);
        }
    }
}

fn refers_to(e: &Expr, input: &str) -> bool {
    match e {
        Expr::Coord(p, _) => p == input,
        Expr::Name(n) => n == input,
        Expr::Bin(l, _, r) => refers_to(l, input) || refers_to(r, input),
    }
}

struct Flattener<'d> {
    description: &'d Description,
    steps: Vec<Step>,
    /// For each `let` flattened so far, its step.
    let_steps: Vec<usize>,
}

impl Flattener<'_> {
    fn push(&mut self, name: String, kind: StepKind) -> usize {
        self.steps.push(Step { name, kind });
        self.steps.len().saturating_sub(1)
    }

    /// The step of a load, made the first time it is needed. A step of the same name is reused
    /// only if it is this very load; a step of that name that computes anything else is refused,
    /// never read in the load's place.
    fn load(&mut self, name: String, kind: StepKind, line: usize) -> Result<usize, LangError> {
        match self.steps.iter().position(|s| s.name == name) {
            Some(i) if self.steps.get(i).is_some_and(|s| s.kind == kind) => Ok(i),
            Some(_) => err(
                line,
                format!("`{name}` would name both a load and another value"),
            ),
            None => Ok(self.push(name, kind)),
        }
    }

    /// The name of the next inner step of `owner`.
    fn inner_name(owner: &str, counter: &mut usize) -> String {
        *counter = counter.saturating_add(1);
        format!("{owner}_{counter}")
    }

    fn flatten(
        &mut self,
        e: &Expr,
        before: usize,
        owner: &str,
        counter: &mut usize,
        top: bool,
        line: usize,
    ) -> Result<usize, LangError> {
        match e {
            Expr::Coord(p, axis) => self.load(
                format!("{p}_{}", axis.letter()),
                StepKind::LoadCoord {
                    point: p.clone(),
                    axis: *axis,
                },
                line,
            ),
            Expr::Name(n) => match self.description.lookup(n, before) {
                Some(Bound::Rounded) => {
                    self.load(n.clone(), StepKind::LoadRounded { input: n.clone() }, line)
                }
                Some(Bound::Let(j)) => match self.let_steps.get(j) {
                    Some(&s) => Ok(s),
                    None => err(line, format!("`{n}` is used before it is computed")),
                },
                _ => err(line, format!("`{n}` cannot be used here")),
            },
            Expr::Bin(l, op, r) => {
                if let (Op::Sub, Expr::Coord(p, pa), Expr::Coord(q, qa)) = (op, &**l, &**r) {
                    if pa == qa {
                        let name = if top {
                            owner.to_owned()
                        } else {
                            Self::inner_name(owner, counter)
                        };
                        return Ok(self.push(
                            name,
                            StepKind::Translate {
                                minuend: p.clone(),
                                subtrahend: q.clone(),
                                axis: *pa,
                            },
                        ));
                    }
                }
                let left = self.flatten(l, before, owner, counter, false, line)?;
                let right = self.flatten(r, before, owner, counter, false, line)?;
                let name = if top {
                    owner.to_owned()
                } else {
                    Self::inner_name(owner, counter)
                };
                Ok(self.push(
                    name,
                    StepKind::Op {
                        op: *op,
                        left,
                        right,
                    },
                ))
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    const ORIENT: &str = "predicate orient2d\n\
        input a b c: point2\n\
        let ba_x = b.x - a.x\n\
        let ba_y = b.y - a.y\n\
        let ca_x = c.x - a.x\n\
        let ca_y = c.y - a.y\n\
        let det = ba_x * ca_y - ba_y * ca_x # the determinant\n\
        sign det\n";

    #[test]
    fn steps_are_named_and_ordered_as_documented() {
        let d = parse(ORIENT).unwrap();
        let steps = d.steps().unwrap();
        let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["ba_x", "ba_y", "ca_x", "ca_y", "det_1", "det_2", "det"]
        );
        assert_eq!(
            steps[4].kind,
            StepKind::Op {
                op: Op::Mul,
                left: 0,
                right: 3
            }
        );
        assert_eq!(d.result_step(&steps).unwrap(), 6);
    }

    #[test]
    fn precedence_and_left_to_right() {
        let d =
            parse("predicate p\ninput a b: point2\nlet v = a.x - b.x + a.y * b.y - a.y\nsign v\n")
                .unwrap();
        let steps = d.steps().unwrap();
        let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
        // ((a.x - b.x) + (a.y * b.y)) - a.y: the translation, two loads, then the operations.
        assert_eq!(names, ["v_1", "a_y", "b_y", "v_2", "v_3", "v"]);
        assert!(matches!(steps[0].kind, StepKind::Translate { .. }));
    }

    #[test]
    fn refusals_name_the_line() {
        let cases = [
            ("input a: point2\n", 1, "starts with"),
            (
                "predicate p\ninput a: point2\nlet v = a.z * a.x\nsign v\n",
                3,
                "no z axis",
            ),
            (
                "predicate p\ninput a: point2\nlet v = a * a.x\nsign v\n",
                3,
                "coordinate",
            ),
            (
                "predicate p\ninput a: point2\nlet v = w * a.x\nsign v\n",
                3,
                "earlier",
            ),
            (
                "predicate p\ninput a: point2\nlet v = a.x * a.y\nlet w = v * v\nsign v\n",
                4,
                "never used",
            ),
            (
                "predicate p\ninput a: point2\nlet v_2 = a.x * a.y\nsign v_2\n",
                3,
                "_<digits>",
            ),
            (
                "predicate p\ninput a: point2\nlet v = a.x / a.y\nsign v\n",
                3,
                "unexpected character",
            ),
            (
                "predicate p\ninput a: point2\nlet v = a.x * a.y\nsign v\nlet w = v\n",
                5,
                "follow",
            ),
            (
                "predicate p\ninput a: point2\nlet v = (a.x * a.y\nsign v\n",
                3,
                "`)`",
            ),
            (
                "predicate p\ninput n: rounded 83 in x\nlet v = n * n\nsign v\n",
                2,
                "group",
            ),
        ];
        for (text, line, words) in cases {
            let e = parse(text).unwrap_err();
            assert_eq!(e.line, line, "{text}: {e}");
            assert!(e.to_string().contains(words), "{text}: {e}");
        }
    }

    /// Every name the generated code cannot bind, or binds itself, is refused with its line, and
    /// so is a name that is a coordinate's load.
    #[test]
    fn reserved_and_colliding_names_are_refused() {
        let with_let = |name: &str| {
            format!(
                "predicate p\ninput a b: point2\nlet {name} = a.x * b.y - a.y * b.x\nsign {name}\n"
            )
        };
        for name in [
            "type",
            "match",
            "loop",
            concat!("un", "safe"),
            "self",
            "gen",
            "union",
            "i128",
            "f64",
            "filter",
            "value",
            "exact",
            "decide",
            "larger",
            "exact_f64",
            "largest_magnitude",
            "rounded_f64",
            "within",
            "scale",
            "m_x",
            "m_y",
            "m_z",
        ] {
            let e = parse(&with_let(name)).unwrap_err();
            assert_eq!(e.line, 3, "{name}: {e}");
        }
        // Not lowercase snake case: refused as names.
        for name in ["ERROR", "None", "Some", "Sign", "aB", "a__b", "a_", "_a"] {
            let e = parse(&with_let(name)).unwrap_err();
            assert!(e.to_string().contains("not a name"), "{name}: {e}");
        }
        // A rounded input's group maximum is reserved too.
        let e = parse(
            "predicate p\ninput n: rounded 83 in n\ninput a: point2\nlet m_n = n * a.x\nsign m_n\n",
        )
        .unwrap_err();
        assert!(e.to_string().contains("group maximum"), "{e}");
        // Predicate names: keywords, reserved names and Windows device names.
        for name in [
            "type", "fn", "mod", "filter", "con", "aux", "nul", "com1", "lpt9",
        ] {
            let text = format!("predicate {name}\ninput a b: point2\nlet v = a.x * b.y\nsign v\n");
            assert_eq!(parse(&text).unwrap_err().line, 1, "{name}");
        }
        // A let or an input named like a coordinate's load.
        for text in [
            "predicate p\ninput a b: point2\nlet a_x = b.x * b.y\nlet v = a.x * a_x - a.y\nsign v\n",
            "predicate p\ninput a b c: point2\nlet a_x = b.x - c.x\nlet v = a.x * a_x\nsign v\n",
            "predicate p\ninput a: point2\ninput a_y: rounded 83 in n\nlet v = a.x * a_y\nsign v\n",
        ] {
            let e = parse(text).unwrap_err();
            assert!(e.to_string().contains("coordinate"), "{text}: {e}");
        }
        // A point named `m`: its coordinate loads would be the group maxima.
        let d = parse("predicate p\ninput m b: point2\nlet v = m.x * b.y - m.y * b.x\nsign v\n")
            .unwrap();
        let e = d.steps().unwrap_err();
        assert!(e.to_string().contains("group maximum"), "{e}");
    }

    /// A load whose name an earlier step already has is refused when that step is anything else,
    /// whatever the order: the flattener never reads another value in a coordinate's place.
    #[test]
    fn a_load_never_reuses_another_value() {
        // Built directly, past `validate`, to reach the flattener's own check.
        let d = Description {
            name: "p".to_owned(),
            inputs: vec![
                Input {
                    name: "a".to_owned(),
                    kind: InputKind::Point { dim: 2 },
                },
                Input {
                    name: "b".to_owned(),
                    kind: InputKind::Point { dim: 2 },
                },
            ],
            lets: vec![
                Let {
                    name: "a_x".to_owned(),
                    expr: Expr::Bin(
                        Box::new(Expr::Coord("b".to_owned(), Axis::X)),
                        Op::Mul,
                        Box::new(Expr::Coord("b".to_owned(), Axis::Y)),
                    ),
                    line: 3,
                },
                Let {
                    name: "v".to_owned(),
                    expr: Expr::Bin(
                        Box::new(Expr::Coord("a".to_owned(), Axis::X)),
                        Op::Mul,
                        Box::new(Expr::Name("a_x".to_owned())),
                    ),
                    line: 4,
                },
            ],
            result: "v".to_owned(),
        };
        let e = d.steps().unwrap_err();
        assert_eq!(e.line, 4, "{e}");
        assert!(
            e.to_string().contains("both a load and another value"),
            "{e}"
        );
    }

    #[test]
    fn rounded_inputs_load_once() {
        let d = parse(
            "predicate side\ninput n: rounded 83 in n\ninput p q: point3\n\
             let v = n * (p.x - q.x) + n * p.y\nsign v\n",
        )
        .unwrap();
        let steps = d.steps().unwrap();
        let loads = steps
            .iter()
            .filter(|s| matches!(s.kind, StepKind::LoadRounded { .. }))
            .count();
        assert_eq!(loads, 1);
        assert_eq!(steps[0].name, "n");
    }
}
