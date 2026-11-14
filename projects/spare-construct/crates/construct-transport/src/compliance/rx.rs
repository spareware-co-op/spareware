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
    Repeat { node: Box<Node>, min: usize, max: usize, greedy: bool },
    Look { node: Box<Node>, behind: bool, neg: bool },
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
    #[must_use]
    pub fn text(&self) -> &str {
        self.get(0)
    }
    /// Capture group n as text (`m[n]`), empty for a group that did not take part.
    #[must_use]
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

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    // Bound: a position in the pattern, which is far shorter than usize::MAX
    fn advance(&mut self) {
        self.i = self.i.saturating_add(1);
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.advance();
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
        Ok(if alts.len() == 1
            && let Some(only) = alts.pop()
        {
            only
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
            v.push(self.quant(atom));
        }
        if v.len() == 1
            && let Some(only) = v.pop()
        {
            return Ok(only);
        }
        Ok(if v.is_empty() { Node::Empty } else { Node::Cat(v) })
    }
    fn number(&mut self) -> Option<usize> {
        let st = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
        if self.i == st {
            None
        } else {
            self.s.get(st..self.i)?.iter().collect::<String>().parse().ok()
        }
    }
    fn quant(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let save = self.i;
                self.advance();
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
                if let Some(p) = r {
                    // the closing brace is consumed below with the other quantifier characters (it was just eaten, so i >= 1)
                    self.i = self.i.saturating_sub(1);
                    p
                } else {
                    self.i = save;
                    return atom;
                }
            }
            _ => return atom,
        };
        self.advance();
        let greedy = !self.eat('?');
        Node::Repeat { node: Box::new(atom), min, max, greedy }
    }
    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.advance();
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                if self.eat('?') {
                    let k = self.peek().ok_or("bad group")?;
                    self.advance();
                    let node = match k {
                        ':' => {
                            let n = self.alt()?;
                            Node::Group(Box::new(n), None)
                        }
                        '=' | '!' => {
                            let n = self.alt()?;
                            Node::Look { node: Box::new(n), behind: false, neg: k == '!' }
                        }
                        '<' => {
                            let k2 = self.peek().ok_or("bad group")?;
                            self.advance();
                            if k2 != '=' && k2 != '!' {
                                return Err("named groups are not supported".into());
                            }
                            let n = self.alt()?;
                            Node::Look { node: Box::new(n), behind: true, neg: k2 == '!' }
                        }
                        _ => return Err(format!("unsupported group (?{k}")),
                    };
                    if !self.eat(')') {
                        return Err("missing )".into());
                    }
                    node
                } else {
                    // Bound: one for each group of the pattern
                    self.groups = self.groups.saturating_add(1);
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
        self.advance();
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
            // c is a digit 1 to 9 here, so to_digit always gives it
            '1'..='9' if !in_class => Node::BackRef(c.to_digit(10).and_then(|d| usize::try_from(d).ok()).unwrap_or(0)),
            c => Node::Char(c),
        })
    }
    fn hex(&mut self, n: usize) -> Result<char, String> {
        // Bound: a position in the pattern plus at most 4
        let end = self.i.saturating_add(n);
        let Some(digits) = self.s.get(self.i..end) else {
            return Err("bad escape".into());
        };
        let t: String = digits.iter().collect();
        self.i = end;
        u32::from_str_radix(&t, 16).ok().and_then(char::from_u32).ok_or_else(|| "bad escape".to_string())
    }
    fn class(&mut self) -> Result<Node, String> {
        let neg = self.eat('^');
        let mut items: Vec<Item> = Vec::new();
        loop {
            let c = self.peek().ok_or("unterminated class")?;
            self.advance();
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
            if let Item::Ch(a) = lo
                && self.peek() == Some('-') && self.s.get(self.i.saturating_add(1)).is_some_and(|&n| n != ']') {
                    self.advance();
                    let n = self.peek().ok_or("unterminated class")?;
                    self.advance();
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
    ///
    /// # Errors
    ///
    /// A message naming what is wrong when the pattern does not parse or uses syntax this engine does not read (a named
    /// group, an unknown group kind, a missing `)` or `]`, a bad escape, a quantifier with nothing to repeat).
    pub fn new(source: &str, flags: &str) -> Result<Regex, String> {
        let chars: Vec<char> = source.chars().collect();
        let mut p = Parser { s: &chars, i: 0, groups: 0 };
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

    /// An expression that matches nothing (an empty class matches no character), standing in for a pattern that does not parse.
    fn never(source: &str, flags: &str) -> Regex {
        Regex {
            node: Node::Class(Vec::new(), false),
            groups: 0,
            icase: flags.contains('i'),
            multiline: flags.contains('m'),
            global: flags.contains('g'),
            source: source.to_string(),
        }
    }

    /// The pattern text (`re.source`).
    #[must_use]
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

    fn m(&self, pat: &Node, text: &[char], i: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        // Bound: a position in the text, at most its length, so one past it stays far below usize::MAX
        let next = i.saturating_add(1);
        match pat {
            Node::Empty => k(i, caps),
            Node::Char(c) => text.get(i).is_some_and(|&ch| self.ch_eq(*c, ch)) && k(next, caps),
            Node::Any => text.get(i).is_some_and(|&ch| !is_line_term(ch)) && k(next, caps),
            Node::Class(items, neg) => text.get(i).is_some_and(|&ch| self.class_has(items, *neg, ch)) && k(next, caps),
            Node::Start => (i == 0 || (self.multiline && i.checked_sub(1).and_then(|prev| text.get(prev)).is_some_and(|&ch| is_line_term(ch)))) && k(i, caps),
            Node::End => (i == text.len() || (self.multiline && text.get(i).is_some_and(|&ch| is_line_term(ch)))) && k(i, caps),
            Node::WordB(want) => {
                let before = i.checked_sub(1).and_then(|prev| text.get(prev)).is_some_and(|&ch| is_word(ch));
                let after = text.get(i).is_some_and(|&ch| is_word(ch));
                ((before != after) == *want) && k(i, caps)
            }
            Node::Group(inner, idx) => match idx {
                None => self.m(inner, text, i, caps, k),
                Some(slot) => {
                    let slot = *slot;
                    // caps has a slot for every group (match_at and exec_from size it so)
                    self.m(inner, text, i, caps, &mut |end, c2: &mut Caps| {
                        let old = c2.get(slot).copied().flatten();
                        if let Some(cell) = c2.get_mut(slot) {
                            *cell = Some((i, end));
                        }
                        if k(end, c2) {
                            true
                        } else {
                            if let Some(cell) = c2.get_mut(slot) {
                                *cell = old;
                            }
                            false
                        }
                    })
                }
            },
            Node::Cat(items) => self.seq(items, 0, text, i, caps, k),
            Node::Alt(alts) => {
                for alt in alts {
                    let saved = caps.clone();
                    if self.m(alt, text, i, caps, k) {
                        return true;
                    }
                    *caps = saved;
                }
                false
            }
            Node::Repeat { node, min, max, greedy } => self.rep(node, *min, *max, *greedy, 0, text, i, caps, k),
            Node::Look { node, behind, neg } => {
                let mut c2 = caps.clone();
                let found = if *behind {
                    (0..=i).rev().any(|start| self.m(node, text, start, &mut c2, &mut |end, _| end == i))
                } else {
                    self.m(node, text, i, &mut c2, &mut |_, _| true)
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
            Node::BackRef(group) => match caps.get(*group).copied().flatten() {
                None => k(i, caps),
                Some((from, to)) => {
                    // Bound: a capture lies inside the text (from <= to <= its length), so these stay far below usize::MAX
                    let len = to.saturating_sub(from);
                    let end = i.saturating_add(len);
                    // the captured text and the text at i compared character by character (None past the end: no match)
                    let same = match (text.get(from..to), text.get(i..end)) {
                        (Some(want), Some(got)) => want.iter().zip(got).all(|(&wc, &gc)| self.ch_eq(wc, gc)),
                        _ => false,
                    };
                    same && k(end, caps)
                }
            },
        }
    }

    fn seq(&self, items: &[Node], j: usize, text: &[char], i: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        let Some(item) = items.get(j) else {
            return k(i, caps);
        };
        // Bound: j is below the number of items here
        self.m(item, text, i, caps, &mut |end, c2: &mut Caps| self.seq(items, j.saturating_add(1), text, end, c2, k))
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(&self, node: &Node, min: usize, max: usize, greedy: bool, count: usize, s: &[char], i: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        if greedy {
            if count < max {
                let saved = caps.clone();
                let ok = self.m(node, s, i, caps, &mut |e, c2: &mut Caps| {
                    if e == i && count >= min {
                        return false;
                    }
                    self.rep(node, min, max, greedy, count.saturating_add(1), s, e, c2, k)
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
                    self.rep(node, min, max, greedy, count.saturating_add(1), s, e, c2, k)
                });
            }
            false
        }
    }

    /// The match that starts exactly at character `at`, as a sticky expression would find it.
    #[must_use]
    pub fn match_at(&self, text: &[char], at: usize) -> Option<(usize, Caps)> {
        // Bound: a count of the pattern's groups
        let mut caps: Caps = vec![None; self.groups.saturating_add(1)];
        let mut end = 0usize;
        let mut snapshot: Caps = Vec::new();
        let ok = self.m(&self.node, text, at, &mut caps, &mut |e, c| {
            end = e;
            snapshot.clone_from(c);
            true
        });
        if ok { Some((end, snapshot)) } else { None }
    }

    fn captures_from(&self, text: &[char], start: usize, end: usize, caps: &Caps) -> Captures {
        // A match and its captures lie inside the text, and caps has a slot for the match and each group
        let mut groups: Vec<Option<String>> = vec![Some(text.get(start..end).unwrap_or_default().iter().collect())];
        for &slot in caps.iter().skip(1).take(self.groups) {
            groups.push(slot.map(|(from, to)| text.get(from..to).unwrap_or_default().iter().collect()));
        }
        Captures { index: start, end, groups }
    }

    /// `re.exec(text)` from character `from` (the first match at or after it).
    #[must_use]
    pub fn exec_from(&self, text: &[char], from: usize) -> Option<Captures> {
        for st in from..=text.len() {
            // Bound: a count of the pattern's groups
            let mut caps: Caps = vec![None; self.groups.saturating_add(1)];
            let mut end = 0usize;
            let mut snapshot: Caps = Vec::new();
            let ok = self.m(&self.node, text, st, &mut caps, &mut |e, c| {
                end = e;
                snapshot.clone_from(c);
                true
            });
            if ok {
                return Some(self.captures_from(text, st, end, &snapshot));
            }
        }
        None
    }

    /// `re.exec(s)`: the first match in the text.
    #[must_use]
    pub fn exec(&self, s: &str) -> Option<Captures> {
        let t: Vec<char> = s.chars().collect();
        self.exec_from(&t, 0)
    }

    /// `re.test(s)`.
    #[must_use]
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
            let Some(m) = self.exec_from(&t, from) else { break };
            // Matches come in order and do not overlap, so pos..m.index lies inside the text
            out.extend(t.get(pos..m.index).unwrap_or_default());
            out.push_str(&f(&m));
            pos = m.end;
            // Bound: a position at most one past the end of the text
            from = if m.end == m.index { m.end.saturating_add(1) } else { m.end };
            if !self.global {
                break;
            }
        }
        if pos <= t.len() {
            out.extend(t.get(pos..).unwrap_or_default());
        }
        out
    }

    /// `s.replace(re, text)` with a plain replacement string (no `$` patterns).
    #[must_use]
    pub fn replace_with(&self, s: &str, with: &str) -> String {
        self.replace(s, |_| with.to_string())
    }

    /// `s.split(re)` (capture groups are not spliced in: the rules' expressions have none).
    #[must_use]
    pub fn split(&self, s: &str) -> Vec<String> {
        let chars: Vec<char> = s.chars().collect();
        if chars.is_empty() {
            return if self.match_at(&chars, 0).is_some() { vec![] } else { vec![String::new()] };
        }
        let mut out = Vec::new();
        // Bound: last <= at <= the text's length, far below usize::MAX, so the ranges below are inside the text
        let (mut last, mut at) = (0usize, 0usize);
        while at < chars.len() {
            match self.match_at(&chars, at) {
                None => at = at.saturating_add(1),
                Some((end, _)) => {
                    let end = end.min(chars.len());
                    if end == last {
                        at = at.saturating_add(1);
                    } else {
                        out.push(chars.get(last..at).unwrap_or_default().iter().collect());
                        last = end;
                        at = last;
                    }
                }
            }
        }
        out.push(chars.get(last..).unwrap_or_default().iter().collect());
        out
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, String), Rc<Regex>>> = RefCell::new(HashMap::new());
}

/// A compiled expression from a cache (the rules use the same few patterns on every call).
///
/// The patterns passed here are the program's own, and every one parses (the tests run each); one that did not would
/// give an expression that matches nothing.
#[must_use]
pub fn rx(source: &str, flags: &str) -> Rc<Regex> {
    CACHE.with(|c| {
        let key = (source.to_string(), flags.to_string());
        if let Some(r) = c.borrow().get(&key) {
            return r.clone();
        }
        let r = Rc::new(Regex::new(source, flags).unwrap_or_else(|_| Regex::never(source, flags)));
        c.borrow_mut().insert(key, r.clone());
        r
    })
}

/// `new RegExp(pattern, flags)` for a pattern that may come from data (a settings line): None when it does not parse.
#[must_use]
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
        Regex::new(p, f).unwrap().exec(s).map(|m| m.text().to_string())
    }

    #[test]
    fn matches_like_javascript() {
        assert_eq!(t(r"^(\d{4})-(\d{2})", "", "2026-10-21").as_deref(), Some("2026-10"));
        assert_eq!(t(r"city centre zone|town", "i", "The City Centre Zone").as_deref(), Some("City Centre Zone"));
        assert_eq!(t(r"\bAT\b(?:\s*\))?[^.;]{0,20}?\b(?:road controlling authority|RCA)\b", "", "AT is the road controlling authority").as_deref(), Some("AT is the road controlling authority"));
        assert_eq!(t(r"^Drury East\b", "i", "Drury Eastern"), None);
        assert_eq!(t(r"a{2,3}?", "", "aaaa").as_deref(), Some("aa"));
        assert_eq!(t(r"a(?=b)", "", "ac ab").as_deref(), Some("a"));
        assert_eq!(Regex::new(r"(?<=[.!?])\s+|\n+", "").unwrap().split("One. Two!  Three\nFour"), vec!["One.", "Two!", "Three", "Four"]);
        assert_eq!(Regex::new(r"[^w~-]", "g").unwrap().replace_with("a~b-c w1", "_"), "_~_-__w_");
        assert_eq!(Regex::new(r"(?:\r?\n)*$", "").unwrap().replace_with("x\r\n\r\n", ""), "x");
    }

    #[test]
    fn captures_and_replace() {
        let m = Regex::new(r"^(\d+) (\w{3})\w* ", "").unwrap().exec("31 December 2027").unwrap();
        assert_eq!((m.get(1), m.get(2)), ("31", "Dec"));
        let r = Regex::new(r"^(\d+) (\w{3})\w* ", "").unwrap().replace("31 December 2027", |m| format!("{} {} ", m.get(1), m.get(2)));
        assert_eq!(r, "31 Dec 2027");
    }
}
