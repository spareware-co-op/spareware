//! A small JavaScript regular expression engine, for the compliance rules that match patterns in text.
//!
//! The rules (the statutory wording linter, zone and precinct matching, the section tests of the checklist) use the
//! prototype's own regular expressions, so this reads the same syntax: literals, `.`, classes with ranges and the
//! escapes `\d \D \w \W \s \S`, `\b \B`, anchors `^ $`, groups (capturing, `(?:...)`), alternation, the quantifiers
//! `* + ? {n} {n,} {n,m}` (each with a lazy `?` form), look-ahead `(?=...) (?!...)` and look-behind `(?<=...) (?<!...)`,
//! and the flags `i`, `g` and `m`. Matching works on characters and backtracks as the JavaScript engine does, so the
//! first match found is the one the prototype finds.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// One compiled expression.
#[derive(Debug)]
pub struct Regex {
    node: Node,
    groups: usize,
    icase: bool,
    multiline: bool,
    global: bool,
    source: String,
}

#[derive(Debug, Clone)]
enum Item {
    Ch(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(Vec<Item>, bool),
    Start,
    End,
    WordB(bool),
    Group(Box<Node>, Option<usize>),
    Cat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: usize,
        greedy: bool,
    },
    Look {
        node: Box<Node>,
        behind: bool,
        neg: bool,
    },
    BackRef(usize),
}

/// The text of a match: the whole match and each capture group (None for a group that did not take part).
#[derive(Debug, Clone, PartialEq)]
pub struct Captures {
    /// Character index where the match starts.
    pub index: usize,
    /// Character index just after the match.
    pub end: usize,
    /// Group 0 is the whole match, then each capturing group in order.
    pub groups: Vec<Option<String>>,
}

impl Captures {
    /// The whole matched text (`m[0]`).
    pub fn text(&self) -> &str {
        self.groups[0].as_deref().unwrap_or("")
    }
    /// Capture group n as text (`m[n]`), empty for a group that did not take part.
    pub fn get(&self, n: usize) -> &str {
        self.groups.get(n).and_then(|g| g.as_deref()).unwrap_or("")
    }
}

type Caps = Vec<Option<(usize, usize)>>;

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    groups: usize,
}

fn is_line_term(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_space(c: char) -> bool {
    crate::jsnum::is_js_space(c)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.cat()?];
        while self.eat('|') {
            alts.push(self.cat()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Node::Alt(alts)
        })
    }
    fn cat(&mut self) -> Result<Node, String> {
        let mut v = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            v.push(self.quant(atom)?);
        }
        Ok(match v.len() {
            0 => Node::Empty,
            1 => v.pop().unwrap(),
            _ => Node::Cat(v),
        })
    }
    fn number(&mut self) -> Option<usize> {
        let st = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.i == st {
            None
        } else {
            self.s[st..self.i].iter().collect::<String>().parse().ok()
        }
    }
    fn quant(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let lo = self.number();
                let r = match lo {
                    Some(lo) => {
                        if self.eat('}') {
                            Some((lo, lo))
                        } else if self.eat(',') {
                            if self.eat('}') {
                                Some((lo, usize::MAX))
                            } else {
                                match self.number() {
                                    Some(hi) if self.eat('}') => Some((lo, hi)),
                                    _ => None,
                                }
                            }
                        } else {
                            None
                        }
                    }
                    None => None,
                };
                match r {
                    Some(p) => {
                        self.i -= 1; // the closing brace is consumed below with the other quantifier characters
                        p
                    }
                    None => {
                        self.i = save;
                        return Ok(atom);
                    }
                }
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        let greedy = !self.eat('?');
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        })
    }
    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.i += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                if self.eat('?') {
                    let k = self.peek().ok_or("bad group")?;
                    self.i += 1;
                    let node = match k {
                        ':' => {
                            let n = self.alt()?;
                            Node::Group(Box::new(n), None)
                        }
                        '=' | '!' => {
                            let n = self.alt()?;
                            Node::Look {
                                node: Box::new(n),
                                behind: false,
                                neg: k == '!',
                            }
                        }
                        '<' => {
                            let k2 = self.peek().ok_or("bad group")?;
                            self.i += 1;
                            if k2 != '=' && k2 != '!' {
                                return Err("named groups are not supported".into());
                            }
                            let n = self.alt()?;
                            Node::Look {
                                node: Box::new(n),
                                behind: true,
                                neg: k2 == '!',
                            }
                        }
                        _ => return Err(format!("unsupported group (?{k}")),
                    };
                    if !self.eat(')') {
                        return Err("missing )".into());
                    }
                    node
                } else {
                    self.groups += 1;
                    let idx = self.groups;
                    let n = self.alt()?;
                    if !self.eat(')') {
                        return Err("missing )".into());
                    }
                    Node::Group(Box::new(n), Some(idx))
                }
            }
            '[' => self.class()?,
            '\\' => self.escape(false)?,
            '*' | '+' | '?' => return Err("nothing to repeat".into()),
            c => Node::Char(c),
        })
    }
    fn escape(&mut self, in_class: bool) -> Result<Node, String> {
        let c = self.peek().ok_or("bad escape")?;
        self.i += 1;
        Ok(match c {
            'd' => Node::Class(vec![Item::Digit(true)], false),
            'D' => Node::Class(vec![Item::Digit(false)], false),
            'w' => Node::Class(vec![Item::Word(true)], false),
            'W' => Node::Class(vec![Item::Word(false)], false),
            's' => Node::Class(vec![Item::Space(true)], false),
            'S' => Node::Class(vec![Item::Space(false)], false),
            'b' if in_class => Node::Char('\u{8}'),
            'b' => Node::WordB(true),
            'B' => Node::WordB(false),
            'n' => Node::Char('\n'),
            'r' => Node::Char('\r'),
            't' => Node::Char('\t'),
            'f' => Node::Char('\u{c}'),
            'v' => Node::Char('\u{b}'),
            '0' => Node::Char('\0'),
            'x' => Node::Char(self.hex(2)?),
            'u' => Node::Char(self.hex(4)?),
            '1'..='9' if !in_class => Node::BackRef(c as usize - '0' as usize),
            c => Node::Char(c),
        })
    }
    fn hex(&mut self, n: usize) -> Result<char, String> {
        if self.i + n > self.s.len() {
            return Err("bad escape".into());
        }
        let t: String = self.s[self.i..self.i + n].iter().collect();
        self.i += n;
        u32::from_str_radix(&t, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| "bad escape".to_string())
    }
    fn class(&mut self) -> Result<Node, String> {
        let neg = self.eat('^');
        let mut items: Vec<Item> = Vec::new();
        loop {
            let c = self.peek().ok_or("unterminated class")?;
            self.i += 1;
            if c == ']' {
                break;
            }
            let lo = if c == '\\' {
                match self.escape(true)? {
                    Node::Char(x) => Item::Ch(x),
                    Node::Class(mut v, _) => {
                        items.append(&mut v);
                        continue;
                    }
                    _ => return Err("bad class escape".into()),
                }
            } else {
                Item::Ch(c)
            };
            // a range a-b (a '-' just before the closing bracket is a literal)
            if let Item::Ch(a) = lo {
                if self.peek() == Some('-') && self.s.get(self.i + 1).is_some_and(|&n| n != ']') {
                    self.i += 1;
                    let n = self.peek().ok_or("unterminated class")?;
                    self.i += 1;
                    let b = if n == '\\' {
                        match self.escape(true)? {
                            Node::Char(x) => x,
                            _ => return Err("bad range".into()),
                        }
                    } else {
                        n
                    };
                    items.push(Item::Range(a, b));
                    continue;
                }
            }
            items.push(lo);
        }
        Ok(Node::Class(items, neg))
    }
}

fn canon(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) if !(c as u32 >= 128 && (x as u32) < 128) => x,
        _ => c,
    }
}

impl Regex {
    /// Compiles a pattern with flags (any of "i", "g", "m"; others are ignored).
    pub fn new(source: &str, flags: &str) -> Result<Regex, String> {
        let chars: Vec<char> = source.chars().collect();
        let mut p = Parser {
            s: &chars,
            i: 0,
            groups: 0,
        };
        let node = p.alt()?;
        if p.i < chars.len() {
            return Err(format!("unmatched ) in /{source}/"));
        }
        Ok(Regex {
            node,
            groups: p.groups,
            icase: flags.contains('i'),
            multiline: flags.contains('m'),
            global: flags.contains('g'),
            source: source.to_string(),
        })
    }

    /// The pattern text (`re.source`).
    pub fn source(&self) -> &str {
        &self.source
    }

    fn ch_eq(&self, a: char, b: char) -> bool {
        a == b || (self.icase && canon(a) == canon(b))
    }

    fn class_has(&self, items: &[Item], neg: bool, c: char) -> bool {
        let mut hit = false;
        for it in items {
            let h = match it {
                Item::Ch(x) => self.ch_eq(*x, c),
                Item::Range(a, b) => {
                    (*a <= c && c <= *b)
                        || (self.icase && {
                            let (lo, up) = (c.to_lowercase().next().unwrap_or(c), canon(c));
                            (*a <= lo && lo <= *b) || (*a <= up && up <= *b)
                        })
                }
                Item::Digit(p) => c.is_ascii_digit() == *p,
                Item::Word(p) => is_word(c) == *p,
                Item::Space(p) => is_space(c) == *p,
            };
            if h {
                hit = true;
                break;
            }
        }
        hit != neg
    }

    fn m(
        &self,
        n: &Node,
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        match n {
            Node::Empty => k(i, caps),
            Node::Char(c) => i < s.len() && self.ch_eq(*c, s[i]) && k(i + 1, caps),
            Node::Any => i < s.len() && !is_line_term(s[i]) && k(i + 1, caps),
            Node::Class(items, neg) => {
                i < s.len() && self.class_has(items, *neg, s[i]) && k(i + 1, caps)
            }
            Node::Start => (i == 0 || (self.multiline && is_line_term(s[i - 1]))) && k(i, caps),
            Node::End => (i == s.len() || (self.multiline && is_line_term(s[i]))) && k(i, caps),
            Node::WordB(want) => {
                let a = i > 0 && is_word(s[i - 1]);
                let b = i < s.len() && is_word(s[i]);
                ((a != b) == *want) && k(i, caps)
            }
            Node::Group(inner, idx) => match idx {
                None => self.m(inner, s, i, caps, k),
                Some(g) => {
                    let g = *g;
                    self.m(inner, s, i, caps, &mut |e, c2: &mut Caps| {
                        let old = c2[g];
                        c2[g] = Some((i, e));
                        if k(e, c2) {
                            true
                        } else {
                            c2[g] = old;
                            false
                        }
                    })
                }
            },
            Node::Cat(v) => self.seq(v, 0, s, i, caps, k),
            Node::Alt(v) => {
                for a in v {
                    let saved = caps.clone();
                    if self.m(a, s, i, caps, k) {
                        return true;
                    }
                    *caps = saved;
                }
                false
            }
            Node::Repeat {
                node,
                min,
                max,
                greedy,
            } => self.rep(node, *min, *max, *greedy, 0, s, i, caps, k),
            Node::Look { node, behind, neg } => {
                let mut c2 = caps.clone();
                let found = if *behind {
                    (0..=i)
                        .rev()
                        .any(|j| self.m(node, s, j, &mut c2, &mut |e, _| e == i))
                } else {
                    self.m(node, s, i, &mut c2, &mut |_, _| true)
                };
                if *neg {
                    !found && k(i, caps)
                } else if found {
                    *caps = c2;
                    k(i, caps)
                } else {
                    false
                }
            }
            Node::BackRef(g) => match caps.get(*g).copied().flatten() {
                None => k(i, caps),
                Some((a, b)) => {
                    let len = b - a;
                    i + len <= s.len()
                        && (0..len).all(|t| self.ch_eq(s[a + t], s[i + t]))
                        && k(i + len, caps)
                }
            },
        }
    }

    fn seq(
        &self,
        v: &[Node],
        j: usize,
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        if j == v.len() {
            return k(i, caps);
        }
        self.m(&v[j], s, i, caps, &mut |e, c2: &mut Caps| {
            self.seq(v, j + 1, s, e, c2, k)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(
        &self,
        node: &Node,
        min: usize,
        max: usize,
        greedy: bool,
        count: usize,
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        if greedy {
            if count < max {
                let saved = caps.clone();
                let ok = self.m(node, s, i, caps, &mut |e, c2: &mut Caps| {
                    if e == i && count >= min {
                        return false;
                    }
                    self.rep(node, min, max, greedy, count + 1, s, e, c2, k)
                });
                if ok {
                    return true;
                }
                *caps = saved;
            }
            count >= min && k(i, caps)
        } else {
            if count >= min && k(i, caps) {
                return true;
            }
            if count < max {
                return self.m(node, s, i, caps, &mut |e, c2: &mut Caps| {
                    if e == i && count >= min {
                        return false;
                    }
                    self.rep(node, min, max, greedy, count + 1, s, e, c2, k)
                });
            }
            false
        }
    }

    /// The match that starts exactly at character `at`, as a sticky expression would find it.
    pub fn match_at(&self, text: &[char], at: usize) -> Option<(usize, Caps)> {
        let mut caps: Caps = vec![None; self.groups + 1];
        let mut end = 0usize;
        let mut snapshot: Caps = Vec::new();
        let ok = self.m(&self.node, text, at, &mut caps, &mut |e, c| {
            end = e;
            snapshot = c.clone();
            true
        });
        if ok { Some((end, snapshot)) } else { None }
    }

    fn captures_from(&self, text: &[char], start: usize, end: usize, caps: &Caps) -> Captures {
        let mut groups: Vec<Option<String>> = vec![Some(text[start..end].iter().collect())];
        for g in 1..=self.groups {
            groups.push(caps[g].map(|(a, b)| text[a..b].iter().collect()));
        }
        Captures {
            index: start,
            end,
            groups,
        }
    }

    /// `re.exec(text)` from character `from` (the first match at or after it).
    pub fn exec_from(&self, text: &[char], from: usize) -> Option<Captures> {
        for st in from..=text.len() {
            let mut caps: Caps = vec![None; self.groups + 1];
            let mut end = 0usize;
            let mut snapshot: Caps = Vec::new();
            let ok = self.m(&self.node, text, st, &mut caps, &mut |e, c| {
                end = e;
                snapshot = c.clone();
                true
            });
            if ok {
                return Some(self.captures_from(text, st, end, &snapshot));
            }
        }
        None
    }

    /// `re.exec(s)`: the first match in the text.
    pub fn exec(&self, s: &str) -> Option<Captures> {
        let t: Vec<char> = s.chars().collect();
        self.exec_from(&t, 0)
    }

    /// `re.test(s)`.
    pub fn test(&self, s: &str) -> bool {
        self.exec(s).is_some()
    }

    /// `s.replace(re, f)`: the first match (every match when the expression has the g flag) replaced by what `f` returns.
    pub fn replace(&self, s: &str, mut f: impl FnMut(&Captures) -> String) -> String {
        let t: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut pos = 0usize;
        let mut from = 0usize;
        while from <= t.len() {
            let Some(m) = self.exec_from(&t, from) else {
                break;
            };
            out.extend(&t[pos..m.index]);
            out.push_str(&f(&m));
            pos = m.end;
            from = if m.end == m.index { m.end + 1 } else { m.end };
            if !self.global {
                break;
            }
        }
        if pos <= t.len() {
            out.extend(&t[pos..]);
        }
        out
    }

    /// `s.replace(re, text)` with a plain replacement string (no `$` patterns).
    pub fn replace_with(&self, s: &str, with: &str) -> String {
        self.replace(s, |_| with.to_string())
    }

    /// `s.split(re)` (capture groups are not spliced in: the rules' expressions have none).
    pub fn split(&self, s: &str) -> Vec<String> {
        let t: Vec<char> = s.chars().collect();
        if t.is_empty() {
            return if self.match_at(&t, 0).is_some() {
                vec![]
            } else {
                vec![String::new()]
            };
        }
        let mut out = Vec::new();
        let (mut p, mut q) = (0usize, 0usize);
        while q < t.len() {
            match self.match_at(&t, q) {
                None => q += 1,
                Some((e, _)) => {
                    let e = e.min(t.len());
                    if e == p {
                        q += 1;
                    } else {
                        out.push(t[p..q].iter().collect());
                        p = e;
                        q = p;
                    }
                }
            }
        }
        out.push(t[p..].iter().collect());
        out
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, String), Rc<Regex>>> = RefCell::new(HashMap::new());
}

/// A compiled expression from a cache (the rules use the same few patterns on every call). Panics on a pattern that does not parse.
pub fn rx(source: &str, flags: &str) -> Rc<Regex> {
    CACHE.with(|c| {
        let key = (source.to_string(), flags.to_string());
        if let Some(r) = c.borrow().get(&key) {
            return r.clone();
        }
        let r = Rc::new(
            Regex::new(source, flags).unwrap_or_else(|e| panic!("bad pattern /{source}/: {e}")),
        );
        c.borrow_mut().insert(key, r.clone());
        r
    })
}

/// `new RegExp(pattern, flags)` for a pattern that may come from data (a settings line): None when it does not parse.
pub fn try_rx(source: &str, flags: &str) -> Option<Rc<Regex>> {
    CACHE.with(|c| {
        let key = (source.to_string(), flags.to_string());
        if let Some(r) = c.borrow().get(&key) {
            return Some(r.clone());
        }
        let r = Rc::new(Regex::new(source, flags).ok()?);
        c.borrow_mut().insert(key, r.clone());
        Some(r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(p: &str, f: &str, s: &str) -> Option<String> {
        Regex::new(p, f)
            .unwrap()
            .exec(s)
            .map(|m| m.text().to_string())
    }

    #[test]
    fn matches_like_javascript() {
        assert_eq!(
            t(r"^(\d{4})-(\d{2})", "", "2026-10-21").as_deref(),
            Some("2026-10")
        );
        assert_eq!(
            t(r"city centre zone|town", "i", "The City Centre Zone").as_deref(),
            Some("City Centre Zone")
        );
        assert_eq!(
            t(
                r"\bAT\b(?:\s*\))?[^.;]{0,20}?\b(?:road controlling authority|RCA)\b",
                "",
                "AT is the road controlling authority"
            )
            .as_deref(),
            Some("AT is the road controlling authority")
        );
        assert_eq!(t(r"^Drury East\b", "i", "Drury Eastern"), None);
        assert_eq!(t(r"a{2,3}?", "", "aaaa").as_deref(), Some("aa"));
        assert_eq!(t(r"a(?=b)", "", "ac ab").as_deref(), Some("a"));
        assert_eq!(
            Regex::new(r"(?<=[.!?])\s+|\n+", "")
                .unwrap()
                .split("One. Two!  Three\nFour"),
            vec!["One.", "Two!", "Three", "Four"]
        );
        assert_eq!(
            Regex::new(r"[^w~-]", "g")
                .unwrap()
                .replace_with("a~b-c w1", "_"),
            "_~_-__w_"
        );
        assert_eq!(
            Regex::new(r"(?:\r?\n)*$", "")
                .unwrap()
                .replace_with("x\r\n\r\n", ""),
            "x"
        );
    }

    #[test]
    fn captures_and_replace() {
        let m = Regex::new(r"^(\d+) (\w{3})\w* ", "")
            .unwrap()
            .exec("31 December 2027")
            .unwrap();
        assert_eq!((m.get(1), m.get(2)), ("31", "Dec"));
        let r = Regex::new(r"^(\d+) (\w{3})\w* ", "")
            .unwrap()
            .replace("31 December 2027", |m| {
                format!("{} {} ", m.get(1), m.get(2))
            });
        assert_eq!(r, "31 Dec 2027");
    }
}
