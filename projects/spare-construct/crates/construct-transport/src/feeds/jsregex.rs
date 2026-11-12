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
    #[must_use]
    pub fn text(&self, s: &[u16], i: usize) -> Option<String> {
        let (a, b) = if i == 0 { (self.start, self.end) } else { (*self.groups.get(i.checked_sub(1)?)?)? };
        Some(String::from_utf16_lossy(s.get(a..b)?))
    }
}

struct Parser<'a> {
    p: &'a [u16],
    i: usize,
    ncaps: usize,
}

// The ASCII character of a code unit below 128; U+FFFF (which no pattern syntax uses) for any other.
fn ascii_char(c: u16) -> char {
    u8::try_from(c).ok().filter(u8::is_ascii).map_or('\u{ffff}', char::from)
}

fn is_word_unit(c: u16) -> bool {
    (c >= u16::from(b'0') && c <= u16::from(b'9')) || (c >= u16::from(b'a') && c <= u16::from(b'z')) || (c >= u16::from(b'A') && c <= u16::from(b'Z')) || c == u16::from(b'_')
}

impl Parser<'_> {
    fn peek(&self) -> Option<u16> {
        self.p.get(self.i).copied()
    }
    // Bound: a position in the pattern, which is far shorter than usize::MAX
    fn advance(&mut self) {
        self.i = self.i.saturating_add(1);
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c as u16) {
            self.advance();
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
        Ok(if alts.len() == 1
            && let Some(only) = alts.pop()
        {
            only
        } else {
            Node::Alt(alts)
        })
    }
    fn seq(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == u16::from(b'|') || c == u16::from(b')') {
                break;
            }
            let before = self.ncaps;
            let atom = self.atom()?;
            let after = self.ncaps;
            items.push(self.quant(atom, (before, after)));
        }
        Ok(Node::Seq(items))
    }
    fn number(&mut self) -> Option<usize> {
        let s = self.i;
        while self.peek().is_some_and(|c| (u16::from(b'0')..=u16::from(b'9')).contains(&c)) {
            self.advance();
        }
        if self.i == s {
            return None;
        }
        self.p.get(s..self.i).map(String::from_utf16_lossy)?.parse().ok()
    }
    fn quant(&mut self, atom: Node, caps: (usize, usize)) -> Node {
        let (min, max) = match self.peek().map(ascii_char) {
            Some('?') => {
                self.advance();
                (0, Some(1))
            }
            Some('*') => {
                self.advance();
                (0, None)
            }
            Some('+') => {
                self.advance();
                (1, None)
            }
            Some('{') => {
                let save = self.i;
                self.advance();
                if let Some(lo) = self.number() {
                    let hi = if self.eat(',') { self.number() } else { Some(lo) };
                    if self.eat('}') {
                        (lo, hi)
                    } else {
                        self.i = save;
                        return atom;
                    }
                } else {
                    self.i = save;
                    return atom;
                }
            }
            _ => return atom,
        };
        let greedy = !self.eat('?');
        Node::Repeat { node: Box::new(atom), min, max, greedy, caps }
    }
    fn escape_item(c: u16) -> Option<Item> {
        Some(match ascii_char(c) {
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
            match ascii_char(c) {
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
        self.advance();
        match ascii_char(c) {
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
                // Bound: one for each group of the pattern
                self.ncaps = self.ncaps.saturating_add(1);
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
                    self.advance();
                    if c == u16::from(b']') {
                        break;
                    }
                    let lo = if c == u16::from(b'\\') {
                        let e = self.peek().ok_or("bad escape")?;
                        self.advance();
                        if let Some(it) = Self::escape_item(e) {
                            items.push(it);
                            continue;
                        }
                        if e == u16::from(b'b') { 8 } else { Self::escaped_char(e) }
                    } else {
                        c
                    };
                    if self.peek() == Some(u16::from(b'-')) && self.i.checked_add(1).and_then(|j| self.p.get(j)).is_some_and(|n| *n != u16::from(b']')) {
                        self.advance();
                        let mut hi = self.peek().ok_or("bad range")?;
                        self.advance();
                        if hi == u16::from(b'\\') {
                            let e = self.peek().ok_or("bad escape")?;
                            self.advance();
                            if let Some(it) = Self::escape_item(e) {
                                items.push(Item::Range(lo, lo));
                                items.push(Item::Range(u16::from(b'-'), u16::from(b'-')));
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
                self.advance();
                if let Some(it) = Self::escape_item(e) {
                    return Ok(Node::Class { neg: false, items: vec![it] });
                }
                match ascii_char(e) {
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
        if (u16::from(b'a')..=u16::from(b'z')).contains(&c) { c.saturating_sub(32) } else { c }
    } else {
        match char::from_u32(u32::from(c)) {
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
                Item::Digit(p) => ((u16::from(b'0')..=u16::from(b'9')).contains(&c)) == *p,
                Item::Word(p) => is_word_unit(c) == *p,
                Item::Space(p) => char::from_u32(u32::from(c)).is_some_and(is_js_space) == *p,
            })
        };
        let mut hit = one(c);
        if !hit && self.icase
            && let Some(ch) = char::from_u32(u32::from(c)) {
                for alt in ch.to_lowercase().chain(ch.to_uppercase()) {
                    if (alt as u32) < 0x10000 && one(alt as u16) {
                        hit = true;
                        break;
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
                // Bound: a position in the text, below its length
                Some(&x) if x == *c || (self.icase && canon(x) == canon(*c)) => k(pos.saturating_add(1), caps),
                _ => false,
            },
            Node::Any => match self.s.get(pos) {
                Some(&x) if !matches!(x, 10 | 13 | 0x2028 | 0x2029) => k(pos.saturating_add(1), caps),
                _ => false,
            },
            Node::Class { neg, items } => match self.s.get(pos) {
                Some(&x) if self.class_has(*neg, items, x) => k(pos.saturating_add(1), caps),
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
                    // Groups are numbered from 1, and caps has a slot for each
                    let slot = i.saturating_sub(1);
                    self.m(inner, pos, caps, &mut |p2, caps: &mut Caps| {
                        let old = caps.get(slot).copied().flatten();
                        if let Some(c) = caps.get_mut(slot) {
                            *c = Some((pos, p2));
                        }
                        if k(p2, caps) {
                            return true;
                        }
                        if let Some(c) = caps.get_mut(slot) {
                            *c = old;
                        }
                        false
                    })
                }
            },
            Node::Repeat { node, min, max, greedy, caps: cr } => self.rep(node, *min, *max, *greedy, *cr, 0, pos, caps, k),
            Node::Start => pos == 0 && k(pos, caps),
            Node::End => pos == self.s.len() && k(pos, caps),
            Node::WordB(want) => {
                let before = pos.checked_sub(1).is_some_and(|b| self.word_at(b));
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
        let Some(item) = items.get(i) else {
            return k(pos, caps);
        };
        // Bound: i is below the number of items here
        self.m(item, pos, caps, &mut |p2, caps: &mut Caps| self.seq(items, i.saturating_add(1), p2, caps, k))
    }
    #[allow(clippy::too_many_arguments)]
    fn rep(&self, node: &Node, min: usize, max: Option<usize>, greedy: bool, cr: (usize, usize), count: usize, pos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool) -> bool {
        let can_more = max.is_none_or(|mx| count < mx);
        let more = |caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool| -> bool {
            // each step starts with the groups inside the repeated part empty
            // cr is the range of the groups inside the repeated part, which caps always holds
            let saved: Vec<Option<(usize, usize)>> = caps.get(cr.0..cr.1).map(<[_]>::to_vec).unwrap_or_default();
            if let Some(slots) = caps.get_mut(cr.0..cr.1) {
                slots.fill(None);
            }
            let hit = self.m(node, pos, caps, &mut |p2, caps: &mut Caps| {
                // an empty step once the minimum is met cannot make progress
                if p2 == pos && count >= min {
                    return false;
                }
                // Bound: a count of steps, each taking at least one code unit past the minimum
                self.rep(node, min, max, greedy, cr, count.saturating_add(1), p2, caps, k)
            });
            if !hit && let Some(slots) = caps.get_mut(cr.0..cr.1) {
                for (slot, old) in slots.iter_mut().zip(&saved) {
                    *slot = *old;
                }
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
    ///
    /// # Errors
    ///
    /// A message naming what is wrong for syntax this matcher does not know (an unsupported group, a missing `)` or `]`, a bad escape).
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
    ///
    /// Every such pattern is one this matcher knows (the truth tests compile each); one it did not know would give a
    /// regex that matches nothing.
    #[must_use]
    pub fn lit(pattern: &str, flags: &str) -> Regex {
        Regex::new(pattern, flags).unwrap_or_else(|_| Regex {
            // An empty class matches no code unit
            node: Node::Class { neg: false, items: Vec::new() },
            ncaps: 0,
            icase: flags.contains('i'),
            global: flags.contains('g'),
        })
    }
    /// `re.exec(s)` from a position: the leftmost match starting at or after `from` (code unit index).
    #[must_use]
    pub fn exec_units(&self, s: &[u16], from: usize) -> Option<Match> {
        let run = Run { s, icase: self.icase };
        for start in from..=s.len() {
            let mut caps: Caps = vec![None; self.ncaps];
            let mut end = None;
            let hit = run.m(&self.node, start, &mut caps, &mut |p, _| {
                end = Some(p);
                true
            });
            // The last step of a match that succeeds always sets the end
            if hit && let Some(end) = end {
                return Some(Match { start, end, groups: caps });
            }
        }
        None
    }
    /// `re.exec(s)` on text from the start.
    #[must_use]
    pub fn exec(&self, s: &str) -> Option<Match> {
        let u: Vec<u16> = s.encode_utf16().collect();
        self.exec_units(&u, 0)
    }
    /// `re.test(s)`.
    #[must_use]
    pub fn test(&self, s: &str) -> bool {
        self.exec(s).is_some()
    }
    /// All matches as a global regex finds them one after another (`s.matchAll(re)`); an empty match moves on by one.
    #[must_use]
    pub fn matches(&self, s: &[u16]) -> Vec<Match> {
        let mut out = Vec::new();
        let mut at = 0;
        while at <= s.len() {
            let Some(m) = self.exec_units(s, at) else { break };
            // Bound: a position at most one past the end of the text
            at = if m.end == m.start { m.end.saturating_add(1) } else { m.end };
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
        // Matches come in order and do not overlap, so each range lies inside the text
        for m in self.matches(&u) {
            out.extend_from_slice(u.get(last..m.start).unwrap_or(&[]));
            out.extend(f(&m, &u).encode_utf16());
            last = m.end;
        }
        out.extend_from_slice(u.get(last..).unwrap_or(&[]));
        String::from_utf16_lossy(&out)
    }
}
