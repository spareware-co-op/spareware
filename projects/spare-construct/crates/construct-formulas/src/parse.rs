//! Text to a formula's tree, on `winnow`.
//!
//! The grammar, loosest first (CEL's order of operators):
//!
//! ```text
//! formula := expr end
//! expr    := or ('?' expr ':' expr)?
//! or      := and ('||' and)*
//! and     := compare ('&&' compare)*
//! compare := sum (('==' | '!=' | '<=' | '>=' | '<' | '>') sum)?
//! sum     := product (('+' | '-') product)*
//! product := unary (('*' | '/') unary)*
//! unary   := ('-' | '+' | '!') unary | atom
//! atom    := measure | name ('(' (expr (',' expr)*)? ')')? | '(' expr ')'
//! measure := number unit? | number unit (number unit)+      e.g. 820 mm, 2 ft 8 in, 3
//! number  := digits ('.' digits)? | '.' digits
//! unit    := a symbol of construct-params' unit table with a factor:
//!            mm | cm | m | in | ft | mm2 | m2 | mm3 | m3 | deg
//! name    := letter (letter | digit | '_')*
//! ```
//!
//! A unit is read only straight after a number, so a parameter may be called `m` or `deg`. The
//! units and their exact factors are `construct_params::Unit`'s, the one table the program has.
//!
//! [`canonical`] writes a read formula in its one spelling, the form a formula is stored in.

use crate::types::{Dim, Ty, of_kind};
use crate::{FormulaError, MAX_CONDITIONALS, MAX_DEPTH, MAX_LEN};
use construct_params::Unit;
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, cut_err, delimited, opt, preceded, repeat, separated, terminated};
use winnow::error::{ContextError, ErrMode};
use winnow::prelude::*;
use winnow::token::{one_of, take_while};

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

/// A formula, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A number with no unit, as written.
    Number(String),
    /// Numbers with units, added: `820 mm`, `2 ft 8 in`.
    Measure(Vec<(String, &'static str)>),
    Name(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

/// The unit a number may carry under `symbol`, from `construct-params`' table, and its dimension:
/// every unit with an exact factor in stored units (not percent or a ratio, which are slopes'
/// display forms).
pub(crate) fn unit_of(symbol: &str) -> Option<(Unit, Dim)> {
    let unit = Unit::ALL
        .into_iter()
        .find(|u| u.symbol() == symbol && u.factor().is_some())?;
    match of_kind(unit.kind())? {
        Ty::Q(dim) => Some((unit, dim)),
        Ty::Truth => None,
    }
}

type Res<O> = ModalResult<O>;

/// A token after any spaces.
fn ws<'i, O, P>(inner: P) -> impl Parser<&'i str, O, ErrMode<ContextError>>
where
    P: Parser<&'i str, O, ErrMode<ContextError>>,
{
    preceded(multispace0, inner)
}

fn identifier<'i>(input: &mut &'i str) -> Res<&'i str> {
    (
        one_of(|c: char| c.is_ascii_alphabetic() || c == '_'),
        take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
    )
        .take()
        .parse_next(input)
}

fn number(input: &mut &str) -> Res<String> {
    alt(((digit1, opt(('.', digit1))).take(), ('.', digit1).take()))
        .map(str::to_owned)
        .parse_next(input)
}

fn unit(input: &mut &str) -> Res<&'static str> {
    ws(identifier)
        .verify_map(|word| unit_of(word).map(|(unit, _)| unit.symbol()))
        .parse_next(input)
}

fn measure(input: &mut &str) -> Res<Expr> {
    let first = ws(number).parse_next(input)?;
    let Some(first_unit) = opt(unit).parse_next(input)? else {
        return Ok(Expr::Number(first));
    };
    let mut parts = vec![(first, first_unit)];
    let more: Vec<(String, &'static str)> = repeat(0.., (ws(number), unit)).parse_next(input)?;
    parts.extend(more);
    Ok(Expr::Measure(parts))
}

fn name_or_call(input: &mut &str) -> Res<Expr> {
    let name = ws(identifier).parse_next(input)?.to_owned();
    let args: Option<Vec<Expr>> = opt(preceded(
        ws('('),
        cut_err(terminated(separated(0.., expr, ws(',')), ws(')'))),
    ))
    .parse_next(input)?;
    Ok(match args {
        Some(args) => Expr::Call(name, args),
        None => Expr::Name(name),
    })
}

fn atom(input: &mut &str) -> Res<Expr> {
    alt((
        measure,
        name_or_call,
        delimited(ws('('), cut_err(expr), cut_err(ws(')'))),
    ))
    .parse_next(input)
}

fn unary(input: &mut &str) -> Res<Expr> {
    let Some(sign) = opt(ws(one_of(['-', '+', '!']))).parse_next(input)? else {
        return atom(input);
    };
    let inner = cut_err(unary).parse_next(input)?;
    Ok(match sign {
        '-' => Expr::Neg(Box::new(inner)),
        '!' => Expr::Not(Box::new(inner)),
        _ => inner,
    })
}

/// One level of left-associated operators: `next (op next)*`.
fn chain<'i>(
    input: &mut &'i str,
    next: fn(&mut &'i str) -> Res<Expr>,
    mut op: impl Parser<&'i str, Op, ErrMode<ContextError>>,
) -> Res<Expr> {
    let mut left = next(input)?;
    while let Some(op) = opt(ws(op.by_ref())).parse_next(input)? {
        let right = cut_err(next).parse_next(input)?;
        left = Expr::Binary(op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn product(input: &mut &str) -> Res<Expr> {
    chain(input, unary, alt(('*'.value(Op::Mul), '/'.value(Op::Div))))
}

fn sum(input: &mut &str) -> Res<Expr> {
    chain(
        input,
        product,
        alt(('+'.value(Op::Add), '-'.value(Op::Sub))),
    )
}

fn compare(input: &mut &str) -> Res<Expr> {
    let left = sum(input)?;
    let op = opt(ws(alt((
        "==".value(Op::Eq),
        "!=".value(Op::Ne),
        "<=".value(Op::Le),
        ">=".value(Op::Ge),
        "<".value(Op::Lt),
        ">".value(Op::Gt),
    ))))
    .parse_next(input)?;
    let Some(op) = op else {
        return Ok(left);
    };
    let right = cut_err(sum).parse_next(input)?;
    Ok(Expr::Binary(op, Box::new(left), Box::new(right)))
}

fn and(input: &mut &str) -> Res<Expr> {
    chain(input, compare, "&&".value(Op::And))
}

fn or(input: &mut &str) -> Res<Expr> {
    chain(input, and, "||".value(Op::Or))
}

fn expr(input: &mut &str) -> Res<Expr> {
    let test = or(input)?;
    let branches =
        opt(preceded(ws('?'), cut_err((expr, preceded(ws(':'), expr))))).parse_next(input)?;
    Ok(match branches {
        Some((yes, no)) => Expr::If(Box::new(test), Box::new(yes), Box::new(no)),
        None => test,
    })
}

/// Reads a formula. The error says where reading stopped, counted in characters from the start.
///
/// # Errors
///
/// `Syntax` for text that isn't a formula, `TooLarge` past the caps on length and nesting.
pub fn parse(text: &str) -> Result<Expr, FormulaError> {
    if text.len() > MAX_LEN {
        return Err(FormulaError::new(
            "TooLarge",
            format!("A formula is at most {MAX_LEN} characters."),
        ));
    }
    if text.trim().is_empty() {
        return Err(FormulaError::new("Syntax", "The formula is empty."));
    }
    let mut depth = 0usize;
    let mut deepest = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        deepest = deepest.max(depth);
    }
    if deepest > MAX_DEPTH || text.matches('?').count() > MAX_CONDITIONALS {
        return Err(FormulaError::new(
            "TooLarge",
            format!(
                "A formula nests at most {MAX_DEPTH} brackets deep and holds at most \
                 {MAX_CONDITIONALS} choices (?:)."
            ),
        ));
    }
    terminated(expr, multispace0).parse(text).map_err(|e| {
        let at = text.get(..e.offset()).map_or(0, |t| t.chars().count());
        let near: String = text
            .get(e.offset()..)
            .unwrap_or_default()
            .trim_start()
            .chars()
            .take(12)
            .collect();
        let message = if near.is_empty() {
            "The formula stops before it is finished.".to_owned()
        } else {
            format!(
                "The formula can't be read from character {} (\"{near}\").",
                at + 1
            )
        };
        FormulaError::new("Syntax", message)
    })
}

/// How tightly each form binds, loosest first, as the grammar above nests them.
fn level(e: &Expr) -> u8 {
    match e {
        Expr::If(..) => 0,
        Expr::Binary(op, ..) => match op {
            Op::Or => 1,
            Op::And => 2,
            Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne => 3,
            Op::Add | Op::Sub => 4,
            Op::Mul | Op::Div => 5,
        },
        Expr::Neg(_) | Expr::Not(_) => 6,
        Expr::Number(_) | Expr::Measure(_) | Expr::Name(_) | Expr::Call(..) => 7,
    }
}

fn symbol(op: Op) -> &'static str {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::Eq => "==",
        Op::Ne => "!=",
        Op::And => "&&",
        Op::Or => "||",
    }
}

/// A number in its one spelling: no leading zero but one before the point, no trailing zero after
/// it, no point with nothing after it. `007.50` is `7.5`, `.5` is `0.5`.
fn number_text(text: &str) -> String {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole.to_owned()
    } else {
        format!("{whole}.{fraction}")
    }
}

/// `e` written in the one spelling, bracketed if it binds more loosely than `at_least`.
fn print(e: &Expr, at_least: u8) -> String {
    let text = match e {
        Expr::Number(n) => number_text(n),
        Expr::Measure(parts) => parts
            .iter()
            .map(|(n, unit)| format!("{} {unit}", number_text(n)))
            .collect::<Vec<_>>()
            .join(" "),
        Expr::Name(name) => name.clone(),
        Expr::Neg(inner) => format!("-{}", print(inner, 6)),
        Expr::Not(inner) => format!("!{}", print(inner, 6)),
        Expr::Call(name, args) => format!(
            "{name}({})",
            args.iter()
                .map(|a| print(a, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::If(test, yes, no) => {
            format!("{} ? {} : {}", print(test, 1), print(yes, 0), print(no, 0))
        }
        Expr::Binary(op, left, right) => {
            let at = level(e);
            // A chain leans left; a comparison takes a sum either side.
            let (l, r) = if at == 3 { (4, 4) } else { (at, at + 1) };
            format!("{} {} {}", print(left, l), symbol(*op), print(right, r))
        }
    };
    if level(e) < at_least {
        format!("({text})")
    } else {
        text
    }
}

/// A formula in its one spelling: one space either side of each operator and after each comma,
/// a space between a number and its unit, brackets only where they change the meaning, numbers
/// without leading or trailing zeros, a unary `+` dropped. Two formulas that read the same have
/// the same spelling, and the spelling reads back to the same formula.
///
/// # Errors
///
/// As [`parse`]; `TooLarge` if the spelling is longer than a formula may be.
pub fn canonical(text: &str) -> Result<String, FormulaError> {
    let spelled = print(&parse(text)?, 0);
    let again = parse(&spelled)?;
    if print(&again, 0) == spelled {
        Ok(spelled)
    } else {
        Err(FormulaError::new(
            "Spelling",
            "The formula has no one spelling that reads back the same.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(n: &str) -> Box<Expr> {
        Box::new(Expr::Name(n.to_owned()))
    }

    #[test]
    fn operators_bind_as_cel_says() {
        let e = parse("a + b * c").unwrap();
        assert_eq!(
            e,
            Expr::Binary(
                Op::Add,
                name("a"),
                Box::new(Expr::Binary(Op::Mul, name("b"), name("c")))
            )
        );
        let e = parse("a - b - c").unwrap();
        assert_eq!(
            e,
            Expr::Binary(
                Op::Sub,
                Box::new(Expr::Binary(Op::Sub, name("a"), name("b"))),
                name("c")
            )
        );
        assert!(matches!(parse("a < b ? a : b").unwrap(), Expr::If(..)));
        assert!(matches!(parse("a != b").unwrap(), Expr::Binary(Op::Ne, ..)));
        assert!(matches!(parse("a <= b").unwrap(), Expr::Binary(Op::Le, ..)));
        assert!(matches!(parse("!(a < b)").unwrap(), Expr::Not(..)));
        assert!(matches!(
            parse("a > b && b > c || d").unwrap(),
            Expr::Binary(Op::Or, ..)
        ));
    }

    #[test]
    fn units_follow_numbers_only() {
        assert_eq!(
            parse("2 ft 8 in").unwrap(),
            Expr::Measure(vec![("2".into(), "ft"), ("8".into(), "in")])
        );
        assert_eq!(
            parse("820mm").unwrap(),
            Expr::Measure(vec![("820".into(), "mm")])
        );
        assert_eq!(parse("m").unwrap(), Expr::Name("m".into()));
        assert_eq!(parse(" .5 ").unwrap(), Expr::Number(".5".into()));
        assert!(matches!(
            parse("min(a, 3 mm, b)").unwrap(),
            Expr::Call(ref f, ref args) if f == "min" && args.len() == 3
        ));
        assert!(matches!(
            parse("width/2-4mm").unwrap(),
            Expr::Binary(Op::Sub, ..)
        ));
    }

    #[test]
    fn what_isnt_a_formula_says_where() {
        for bad in [
            "2 width", "a +", "(a", "a b", "3 ft 4", "a ? b", "min(a,", "", "  ", "1.", "a == ",
        ] {
            let e = parse(bad).unwrap_err();
            assert_eq!(e.code, "Syntax", "{bad}");
        }
        let e = parse("a + * b").unwrap_err();
        assert!(e.message.contains("\"* b\""), "{}", e.message);
        let deep = format!("{}a{}", "(".repeat(40), ")".repeat(40));
        assert_eq!(parse(&deep).unwrap_err().code, "TooLarge");
        assert_eq!(parse(&"a+".repeat(600)).unwrap_err().code, "TooLarge");
        assert_eq!(parse(&"a?b:".repeat(20)).unwrap_err().code, "TooLarge");
    }
}
