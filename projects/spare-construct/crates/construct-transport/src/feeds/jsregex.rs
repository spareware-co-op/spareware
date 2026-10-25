//! Regular expressions with JavaScript's rules, for the patterns the prototype's scripts use.
//!
//! The crate takes no regex library, and the prototype's text handling (place search, count field guesses, public
//! holiday and speed limit descriptions) leans on backtracking regular expressions, so this is a small matcher for
//! the part of the syntax those use: literals, `.`, classes (`[a-z]`, `[^/]`, `\d \w \s`), groups, non-capturing
//! groups, lookahead, alternation, the quantifiers `? * + {n,m}` with their lazy forms, `^ $ \b \B`, and the flags `i` and `g`.
//! Text is matched as UTF-16 code units, as JavaScript does without the `u` flag, and positions are code unit indices.
//! The truth tests compare it with Node on many patterns and inputs.

use crate::jsnum::is_js_space;

#[derive(Clone, Debug)]
enum Item {
    Range(u16, u16),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Clone, Debug)]
enum Node {
    Char(u16),
    Any,
    Class { neg: bool, items: Vec<Item> },
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Group(Box<Node>, Option<usize>),
    Repeat { node: Box<Node>, min: usize, max: Option<usize>, greedy: bool, caps: (usize, usize) },
    Start,
    End,
    WordB(bool),
    Look { node: Box<Node>, neg: bool },
}

/// A compiled pattern.
#[derive(Clone, Debug)]
pub struct Regex {
    node: Node,
    ncaps: usize,
    icase: bool,
    global: bool,
}

/// One match: where it starts and ends (code unit indices) and the groups.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// Start of the match.
    pub start: usize,
    /// End of the match (exclusive).
    pub end: usize,
    /// Groups 1 and up as (start, end), None for a group that did not take part.
    pub groups: Vec<Option<(usize, usize)>>,
}

impl Match {
    /// The text of group `i` (0 is the whole match), or None when it did not take part.
    pub fn text(&self, s: &[u16], i: usize) -> Option<String> {
        let (a, b) = if i == 0 { (self.start, self.end) } else { (*self.groups.get(i - 1)?)?.to_owned() };
        Some(String::from_utf16_lossy(&s[a..b]))
    }
}

struct Parser<'a> {
    p: &'a [u16],
    i: usize,
    ncaps: usize,
}

fn is_word_unit(c: u16) -> bool {
    (c >= b'0' as u16 && c <= b'9' as u16) || (c >= b'a' as u16 && c <= b'z' as u16) || (c >= b'A' as u16 && c <= b'Z' as u16) || c == b'_' as u16
}

impl Parser<'_> {
    fn peek(&self) -> Option<u16> {
        self.p.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c as u16) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.seq()?];
        while self.eat('|') {
            alts.push(self.seq()?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { Node::Alt(alts) })
    }
    fn seq(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == b'|' as u16 || c == b')' as u16 {
                break;
            }
            let before = self.ncaps;
            let atom = self.atom()?;
            let after = self.ncaps;
            items.push(self.quant(atom, (before, after))?);
        }
        Ok(Node::Seq(items))
    }
    fn number(&mut self) -> Option<usize> {
        let s = self.i;
        while self.peek().is_some_and(|c| (b'0' as u16..=b'9' as u16).contains(&c)) {
            self.i += 1;
        }
        if self.i == s {
            return None;
        }
        String::from_utf16_lossy(&self.p[s..self.i]).parse().ok()
    }
    fn quant(&mut self, atom: Node, caps: (usize, usize)) -> Result<Node, String> {
        let (min, max) = match self.peek().map(|c| if c < 128 { c as u8 as char } else { '\u{ffff}' }) {
            Some('?') => {
                self.i += 1;
                (0, Some(1))
            }
            Some('*') => {
                self.i += 1;
                (0, None)
            }
            Some('+') => {
                self.i += 1;
                (1, None)
            }
            Some('{') => {
                let save = self.i;
                self.i += 1;
                match self.number() {
                    Some(lo) => {
                        let hi = if self.eat(',') { self.number() } else { Some(lo) };
                        if self.eat('}') {
                            (lo, hi)
                        } else {
                            self.i = save;
                            return Ok(atom);
                        }
                    }
                    None => {
                        self.i = save;
                        return Ok(atom);
                    }
                }
            }
            _ => return Ok(atom),
        };
        let greedy = !self.eat('?');
        Ok(Node::Repeat { node: Box::new(atom), min, max, greedy, caps })
    }
    fn escape_item(&mut self, c: u16) -> Option<Item> {
        Some(match c as u8 as char {
            'd' if c < 128 => Item::Digit(true),
            'D' if c < 128 => Item::Digit(false),
            'w' if c < 128 => Item::Word(true),
            'W' if c < 128 => Item::Word(false),
            's' if c < 128 => Item::Space(true),
            'S' if c < 128 => Item::Space(false),
            _ => return None,
        })
    }
    fn escaped_char(c: u16) -> u16 {
        if c < 128 {
            match c as u8 as char {
                'n' => 10,
                'r' => 13,
                't' => 9,
                'f' => 12,
                'v' => 11,
                '0' => 0,
                _ => c,
            }
        } else {
            c
        }
    }
    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.i += 1;
        match c as u8 as char {
            _ if c >= 128 => Ok(Node::Char(c)),
            '(' => {
                if self.eat('?') {
                    if self.eat(':') {
                        let n = self.alt()?;
                        if !self.eat(')') {
                            return Err("missing )".into());
                        }
                        return Ok(Node::Group(Box::new(n), None));
                    }
                    let neg = if self.eat('=') {
                        false
                    } else if self.eat('!') {
                        true
                    } else {
                        return Err("unsupported group".into());
                    };
                    let n = self.alt()?;
                    if !self.eat(')') {
                        return Err("missing )".into());
                    }
                    return Ok(Node::Look { node: Box::new(n), neg });
                }
                self.ncaps += 1;
                let idx = self.ncaps;
                let n = self.alt()?;
                if !self.eat(')') {
                    return Err("missing )".into());
                }
                Ok(Node::Group(Box::new(n), Some(idx)))
            }
            '.' => Ok(Node::Any),
            '^' => Ok(Node::Start),
            '$' => Ok(Node::End),
            '[' => {
                let neg = self.eat('^');
                let mut items = Vec::new();
                loop {
                    let c = self.peek().ok_or("missing ]")?;
                    self.i += 1;
                    if c == b']' as u16 {
                        break;
                    }
                    let lo = if c == b'\\' as u16 {
                        let e = self.peek().ok_or("bad escape")?;
                        self.i += 1;
                        if let Some(it) = self.escape_item(e) {
                            items.push(it);
                            continue;
                        }
                        if e == b'b' as u16 { 8 } else { Self::escaped_char(e) }
                    } else {
                        c
                    };
                    if self.peek() == Some(b'-' as u16) && self.p.get(self.i + 1).is_some_and(|n| *n != b']' as u16) {
                        self.i += 1;
                        let mut hi = self.peek().ok_or("bad range")?;
                        self.i += 1;
                        if hi == b'\\' as u16 {
                            let e = self.peek().ok_or("bad escape")?;
                            self.i += 1;
                            if let Some(it) = self.escape_item(e) {
                                items.push(Item::Range(lo, lo));
                                items.push(Item::Range(b'-' as u16, b'-' as u16));
                                items.push(it);
                                continue;
                            }
                            hi = Self::escaped_char(e);
                        }
                        items.push(Item::Range(lo, hi));
                    } else {
                        items.push(Item::Range(lo, lo));
                    }
                }
                Ok(Node::Class { neg, items })
            }
            '\\' => {
                let e = self.peek().ok_or("bad escape")?;
                self.i += 1;
                if let Some(it) = self.escape_item(e) {
                    return Ok(Node::Class { neg: false, items: vec![it] });
                }
                match e as u8 as char {
                    'b' if e < 128 => Ok(Node::WordB(true)),
                    'B' if e < 128 => Ok(Node::WordB(false)),
                    _ => Ok(Node::Char(Self::escaped_char(e))),
                }
            }
            _ => Ok(Node::Char(c)),
        }
    }
}

fn canon(c: u16) -> u16 {
    if c < 128 {
        if (b'a' as u16..=b'z' as u16).contains(&c) { c - 32 } else { c }
    } else {
        match char::from_u32(c as u32) {
            Some(ch) => {
                let mut up = ch.to_uppercase();
                match (up.next(), up.next()) {
                    (Some(u), None) if (u as u32) < 0x10000 && (u as u32) >= 128 => u as u16,
                    _ => c,
                }
            }
            None => c,
        }
    }
}

struct Run<'a> {
    s: &'a [u16],
    icase: bool,
}

type Caps = Vec<Option<(usize, usize)>>;

impl Run<'_> {
    fn class_has(&self, neg: bool, items: &[Item], c: u16) -> bool {
        let one = |c: u16| {
            items.iter().any(|it| match it {
                Item::Range(a, b) => *a <= c && c <= *b,
                Item::Digit(p) => ((b'0' as u16..=b'9' as u16).contains(&c)) == *p,
                Item::Word(p) => is_word_unit(c) == *p,
                Item::Space(p) => char::from_u32(c as u32).is_some_and(is_js_space) == *p,
            })
        };
        let mut hit = one(c);
        if !hit && self.icase {
            if let Some(ch) = char::from_u32(c as u32) {
                for alt in ch.to_lowercase().chain(ch.to_uppercase()) {
                    if (alt as u32) < 0x10000 && one(alt as u16) {
                        hit = true;
                        break;
                    }
                }
            }
        }
        hit != neg
    }
    fn word_at(&self, i: usize) -> bool {
        self.s.get(i).is_some_and(|c| is_word_unit(*c))
    }
    fn m(&self, n: &Node, pos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        match n {
            Node::Char(c) => match self.s.get(pos) {
                Some(&x) if x == *c || (self.icase && canon(x) == canon(*c)) => k(pos + 1, caps),
                _ => false,
            },
            Node::Any => match self.s.get(pos) {
                Some(&x) if !matches!(x, 10 | 13 | 0x2028 | 0x2029) => k(pos + 1, caps),
                _ => false,
            },
            Node::Class { neg, items } => match self.s.get(pos) {
                Some(&x) if self.class_has(*neg, items, x) => k(pos + 1, caps),
                _ => false,
            },
            Node::Seq(items) => self.seq(items, 0, pos, caps, k),
            Node::Alt(alts) => {
                for a in alts {
                    if self.m(a, pos, caps, k) {
                        return true;
                    }
                }
                false
            }
            Node::Group(inner, idx) => match idx {
                None => self.m(inner, pos, caps, k),
                Some(i) => {
                    let i = *i;
                    self.m(inner, pos, caps, &mut |p2, caps: &mut Caps| {
                        let old = caps[i - 1];
                        caps[i - 1] = Some((pos, p2));
                        if k(p2, caps) {
                            return true;
                        }
                        caps[i - 1] = old;
                        false
                    })
                }
            },
            Node::Repeat { node, min, max, greedy, caps: cr } => self.rep(node, *min, *max, *greedy, *cr, 0, pos, caps, k),
            Node::Start => pos == 0 && k(pos, caps),
            Node::End => pos == self.s.len() && k(pos, caps),
            Node::WordB(want) => {
                let before = pos > 0 && self.word_at(pos - 1);
                let after = self.word_at(pos);
                ((before != after) == *want) && k(pos, caps)
            }
            Node::Look { node, neg } => {
                let mut c2 = caps.clone();
                let hit = self.m(node, pos, &mut c2, &mut |_, _| true);
                if *neg {
                    !hit && k(pos, caps)
                } else if hit {
                    *caps = c2;
                    k(pos, caps)
                } else {
                    false
                }
            }
        }
    }
    fn seq(&self, items: &[Node], i: usize, pos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        if i == items.len() {
            return k(pos, caps);
        }
        self.m(&items[i], pos, caps, &mut |p2, caps: &mut Caps| self.seq(items, i + 1, p2, caps, k))
    }
    #[allow(clippy::too_many_arguments)]
    fn rep(&self, node: &Node, min: usize, max: Option<usize>, greedy: bool, cr: (usize, usize), count: usize, pos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        let can_more = max.is_none_or(|mx| count < mx);
        let more = |caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool| -> bool {
            // each step starts with the groups inside the repeated part empty
            let saved: Vec<Option<(usize, usize)>> = caps[cr.0..cr.1].to_vec();
            for c in caps[cr.0..cr.1].iter_mut() {
                *c = None;
            }
            let hit = self.m(node, pos, caps, &mut |p2, caps: &mut Caps| {
                // an empty step once the minimum is met cannot make progress
                if p2 == pos && count >= min {
                    return false;
                }
                self.rep(node, min, max, greedy, cr, count + 1, p2, caps, k)
            });
            if !hit {
                caps[cr.0..cr.1].copy_from_slice(&saved);
            }
            hit
        };
        if greedy {
            if can_more && more(caps, k) {
                return true;
            }
            count >= min && k(pos, caps)
        } else {
            if count >= min && k(pos, caps) {
                return true;
            }
            can_more && more(caps, k)
        }
    }
}

impl Regex {
    /// Compiles a pattern with flags ("i", "g", or both); an error for syntax this matcher does not know.
    pub fn new(pattern: &str, flags: &str) -> Result<Regex, String> {
        let p: Vec<u16> = pattern.encode_utf16().collect();
        let mut parser = Parser { p: &p, i: 0, ncaps: 0 };
        let node = parser.alt()?;
        if parser.i != p.len() {
            return Err(format!("unmatched ) at {}", parser.i));
        }
        Ok(Regex { node, ncaps: parser.ncaps, icase: flags.contains('i'), global: flags.contains('g') })
    }
    /// Compiles a pattern that is part of the program (a mistake in it is a bug).
    pub fn lit(pattern: &str, flags: &str) -> Regex {
        Regex::new(pattern, flags).unwrap_or_else(|e| panic!("pattern {pattern}: {e}"))
    }
    /// `re.exec(s)` from a position: the leftmost match starting at or after `from` (code unit index).
    pub fn exec_units(&self, s: &[u16], from: usize) -> Option<Match> {
        let run = Run { s, icase: self.icase };
        for start in from..=s.len() {
            let mut caps: Caps = vec![None; self.ncaps];
            let mut end = None;
            if run.m(&self.node, start, &mut caps, &mut |p, _| {
                end = Some(p);
                true
            }) {
                return Some(Match { start, end: end.unwrap(), groups: caps });
            }
        }
        None
    }
    /// `re.exec(s)` on text from the start.
    pub fn exec(&self, s: &str) -> Option<Match> {
        let u: Vec<u16> = s.encode_utf16().collect();
        self.exec_units(&u, 0)
    }
    /// `re.test(s)`.
    pub fn test(&self, s: &str) -> bool {
        self.exec(s).is_some()
    }
    /// All matches as a global regex finds them one after another (`s.matchAll(re)`); an empty match moves on by one.
    pub fn matches(&self, s: &[u16]) -> Vec<Match> {
        let mut out = Vec::new();
        let mut at = 0;
        while at <= s.len() {
            let Some(m) = self.exec_units(s, at) else { break };
            at = if m.end == m.start { m.end + 1 } else { m.end };
            out.push(m);
            if !self.global {
                break;
            }
        }
        out
    }
    /// `s.replace(re, f)`: the first match (the regex has no `g` flag) or every match (with `g`) replaced by what `f` gives.
    pub fn replace(&self, s: &str, f: impl Fn(&Match, &[u16]) -> String) -> String {
        let u: Vec<u16> = s.encode_utf16().collect();
        let mut out: Vec<u16> = Vec::new();
        let mut last = 0;
        for m in self.matches(&u) {
            out.extend_from_slice(&u[last..m.start]);
            out.extend(f(&m, &u).encode_utf16());
            last = m.end;
        }
        out.extend_from_slice(&u[last..]);
        String::from_utf16_lossy(&out)
    }
}
