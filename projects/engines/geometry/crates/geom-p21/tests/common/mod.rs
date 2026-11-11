//! Helpers the test files share: reading a text to events, sources that cut their reads small, and
//! a source that makes a file of any length without holding it.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "tests: a failure stops the test"
)]

use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_p21::{Error, Event, Exchange, Options, Reader, Record, Totals};
use geom_run::{Budget, WorkMeter};

/// A meter with no limit.
pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A source that gives at most `step` bytes a call.
pub struct Drip<'a> {
    pub rest: &'a [u8],
    pub step: usize,
}

impl ByteSource for Drip<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let n = buf.len().min(self.rest.len()).min(self.step);
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

/// Every event of `source` with the records its tokens held, or the error and the events before
/// it.
pub fn events_of<S: ByteSource>(
    source: S,
    options: Options,
) -> (Vec<(Event, Vec<Record>)>, Option<Error>) {
    let mut meter = meter();
    let mut reader = Reader::new(source, options, &mut meter);
    let mut events = Vec::new();
    loop {
        match reader.next_event() {
            Ok(Some(event)) => events.push((event, reader.tokens().records())),
            Ok(None) => return (events, None),
            Err(e) => {
                assert_eq!(reader.next_event(), Err(e), "the error is given again");
                assert!(reader.tokens().is_empty());
                return (events, Some(e));
            }
        }
    }
}

/// [`events_of`] a text read whole, with the default options.
pub fn events(text: &[u8]) -> (Vec<(Event, Vec<Record>)>, Option<Error>) {
    events_of(SliceSource::new(text), Options::DEFAULT)
}

/// The error a text is refused with.
pub fn refusal(text: &[u8]) -> Error {
    events(text)
        .1
        .unwrap_or_else(|| panic!("{:?} was read", String::from_utf8_lossy(text)))
}

/// A whole file read into memory with the defaults.
pub fn exchange(text: &[u8]) -> Result<Exchange, Error> {
    Exchange::read(
        SliceSource::new(text),
        Options::DEFAULT,
        Totals::DEFAULT,
        &mut meter(),
    )
}

/// A file with an empty header and one `DATA` section holding `data`.
pub fn data(data: &str) -> Vec<u8> {
    format!("ISO-10303-21;HEADER;ENDSEC;DATA;{data}ENDSEC;END-ISO-10303-21;").into_bytes()
}

/// The records of the instances of `data(text)`.
pub fn instances(text: &str) -> Vec<(u64, Vec<Record>)> {
    let (events, error) = events(&data(text));
    assert_eq!(error, None, "{text}");
    events
        .into_iter()
        .filter_map(|(event, records)| match event {
            Event::Instance(id) => Some((id, records)),
            _ => None,
        })
        .collect()
}

/// A source that writes a file of `instances` instances as it is read, holding one line.
pub struct Synthetic {
    next: u64,
    instances: u64,
    line: Vec<u8>,
    at: usize,
    stage: u8,
    /// The bytes handed out so far.
    pub bytes: u64,
}

impl Synthetic {
    pub fn new(instances: u64) -> Self {
        Self {
            next: 1,
            instances,
            line: Vec::new(),
            at: 0,
            stage: 0,
            bytes: 0,
        }
    }

    /// One line of a typical B-rep file for the instance `id`.
    pub fn instance(id: u64) -> String {
        match id % 4 {
            0 => format!(
                "#{id}=CARTESIAN_POINT('',({}.5,-{}.25E-2,0.));\n",
                id % 1000,
                id % 77
            ),
            1 => format!(
                "#{id}=ORIENTED_EDGE('',*,*,#{},.T.);\n",
                id.saturating_sub(1)
            ),
            2 => format!(
                "#{id}=B_SPLINE_CURVE_WITH_KNOTS('n{id}',3,(#{},#{},#{}),.UNSPECIFIED.,.F.,.F.,(4,4),(0.,1.),.UNSPECIFIED.);\n",
                id / 2,
                id / 3,
                id / 5
            ),
            _ => format!(
                "#{id}=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#{},#{}))REPRESENTATION_CONTEXT('',$));\n",
                id / 2,
                id / 7
            ),
        }
    }

    fn refill(&mut self) -> bool {
        self.at = 0;
        self.line.clear();
        match self.stage {
            0 => {
                self.line.extend_from_slice(
                    b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('big','',(''),(''),'','','');\nFILE_SCHEMA(('S'));\nENDSEC;\nDATA;\n",
                );
                self.stage = 1;
            }
            1 if self.next <= self.instances => {
                self.line
                    .extend_from_slice(Self::instance(self.next).as_bytes());
                self.next += 1;
            }
            1 => {
                self.line.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
                self.stage = 2;
            }
            _ => return false,
        }
        true
    }
}

impl ByteSource for Synthetic {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let mut n = 0;
        while n < buf.len() {
            if self.at == self.line.len() && !self.refill() {
                break;
            }
            let take = (buf.len() - n).min(self.line.len() - self.at);
            buf[n..n + take].copy_from_slice(&self.line[self.at..self.at + take]);
            self.at += take;
            n += take;
        }
        self.bytes += n as u64;
        Ok(n)
    }
}
