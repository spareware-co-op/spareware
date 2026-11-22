//! A whole Part 21 file in memory: its header and every entity instance by id, read with caps on
//! the whole and written back canonically.
//!
//! What it does: [`Exchange::read`] drives the streaming reader ([`crate::read`]) to the end and
//! collects the header ([`Header`]) and the instances of every `DATA` section into one map by id.
//! [`Exchange::write`] writes it with the canonical writer ([`crate::write`]): the header, then
//! one `DATA` section of every instance in id order.
//!
//! Guarantees:
//!
//! - **Bounded by [`Totals`].** A read refuses, with a typed error, a text with more bytes,
//!   instances, records or parameters than the caller's totals, before it holds them.
//! - **Deterministic order.** The instances are in a `BTreeMap`, so every walk is in id order.
//! - **A repeated id is reported, never merged.** The first definition is kept and the id is
//!   listed in [`Exchange::duplicates`]; a repeat still counts against the totals.
//! - **Reading what was written gives the same file**: `read(write(x))` writes the same bytes as
//!   `x`. Header records other than the three Part 21 requires are kept as they were read.
//!
//! Method: the header section and data sections of ISO 10303-21
//! (<https://www.iso.org/standard/63141.html>), collected as chapter 24.8 of
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` describes.
//!
//! Cost: linear in the file for the read and the write, with a map insertion (logarithmic in
//! the instances so far) for each instance. Memory is the tree of every instance. Recursion:
//! none here.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-record/src/step/p21.rs>
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-step/src/write.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: `Exchange`, `Header` and the collecting half of `parse_limited`, on
//!   the streaming reader in `no_std` Rust. The caps on entities, records and parameters are
//!   the caller's `Totals` (the origin's constants are its defaults), the byte cap is checked as
//!   the text is read and not on a string already in memory, and progress and cancellation go
//!   through the work meter (the origin had a trait of its own). The header's schemas moved into
//!   `Header`, its other records are kept, and the three copies of header fields the origin held
//!   beside it are gone. `write` is the origin's `Writer::file`, taking the header as given.
//! SPDX-License-Identifier: Apache-2.0

use alloc::collections::BTreeMap;
use alloc::collections::btree_map::Entry;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use geom_format::source::ByteSource;
use geom_run::WorkMeter;

use crate::error::{Error, ErrorKind, WriteError};
use crate::read::{Event, Options, Reader};
use crate::value::{Entity, List, Param, Record};
use crate::write::{ByteSink, Writer};

/// The most a read of a whole file may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Totals {
    /// The most bytes of text.
    pub bytes: u64,
    /// The most entity instances, a repeated id counted each time.
    pub instances: usize,
    /// The most records, each part of a complex instance counted.
    pub records: usize,
    /// The most parameters, nested ones and the header's included.
    pub params: usize,
}

impl Totals {
    /// The defaults: 512 MiB of text and 20,000,000 each of instances, records and parameters,
    /// which hold the parameters themselves to about 1 GB.
    pub const DEFAULT: Self = Self {
        bytes: 512 << 20,
        instances: 20_000_000,
        records: 20_000_000,
        params: 20_000_000,
    };
}

impl Default for Totals {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The HEADER section, as written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    /// `FILE_DESCRIPTION` lines.
    pub descriptions: Vec<String>,
    /// `FILE_DESCRIPTION` implementation level.
    pub implementation_level: String,
    /// `FILE_NAME` name.
    pub file_name: String,
    /// `FILE_NAME` time stamp.
    pub time_stamp: String,
    /// `FILE_NAME` authors.
    pub authors: Vec<String>,
    /// `FILE_NAME` organizations.
    pub organizations: Vec<String>,
    /// `FILE_NAME` preprocessor version.
    pub preprocessor_version: String,
    /// `FILE_NAME` originating system.
    pub originating_system: String,
    /// `FILE_NAME` authorization.
    pub authorization: String,
    /// `FILE_SCHEMA` names.
    pub schemas: Vec<String>,
    /// Every other header record, in file order.
    pub other: Vec<Record>,
}

/// The strings among `param`'s items, when it is a list.
fn strings(param: Option<&Param>) -> Vec<String> {
    param
        .and_then(Param::as_list)
        .unwrap_or(&[])
        .iter()
        .filter_map(|p| p.as_str().map(String::from))
        .collect()
}

/// `param` when it is a string, or the empty string.
fn string(param: Option<&Param>) -> String {
    param.and_then(Param::as_str).unwrap_or("").into()
}

/// The list of `strings` as a parameter.
fn list(strings: &[String]) -> Param {
    Param::List(List::within_bound(
        strings.iter().cloned().map(Param::Str).collect(),
    ))
}

impl Header {
    /// Takes one header record in: one of the three Part 21 requires fills its fields, a later
    /// one replacing an earlier; any other is kept in [`Header::other`].
    fn take(&mut self, record: Record) {
        let p = &record.params;
        match record.name.as_str() {
            "FILE_DESCRIPTION" => {
                self.descriptions = strings(p.first());
                self.implementation_level = string(p.get(1));
            }
            "FILE_NAME" => {
                self.file_name = string(p.first());
                self.time_stamp = string(p.get(1));
                self.authors = strings(p.get(2));
                self.organizations = strings(p.get(3));
                self.preprocessor_version = string(p.get(4));
                self.originating_system = string(p.get(5));
                self.authorization = string(p.get(6));
            }
            "FILE_SCHEMA" => self.schemas = strings(p.first()),
            _ => self.other.push(record),
        }
    }

    /// The three records Part 21 requires, in its order.
    fn records(&self) -> [Record; 3] {
        let text = |s: &String| Param::Str(s.clone());
        [
            Record {
                name: "FILE_DESCRIPTION".into(),
                params: vec![list(&self.descriptions), text(&self.implementation_level)],
            },
            Record {
                name: "FILE_NAME".into(),
                params: vec![
                    text(&self.file_name),
                    text(&self.time_stamp),
                    list(&self.authors),
                    list(&self.organizations),
                    text(&self.preprocessor_version),
                    text(&self.originating_system),
                    text(&self.authorization),
                ],
            },
            Record {
                name: "FILE_SCHEMA".into(),
                params: vec![list(&self.schemas)],
            },
        ]
    }
}

/// A parsed exchange file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exchange {
    /// The HEADER section.
    pub header: Header,
    /// The DATA sections' entity instances by id.
    pub entities: BTreeMap<u64, Entity>,
    /// Ids defined more than once (the first definition is kept), in file order.
    pub duplicates: Vec<u64>,
    /// How many replacement characters lossy text decoding put in ([`Options::lossy_text`]).
    pub replaced: u64,
}

impl Exchange {
    /// The entity instance `#id`.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Reads a whole exchange file from `source`, each record within `options` and the whole
    /// within `totals`, charging `meter` one unit for each byte.
    ///
    /// # Errors
    ///
    /// The reader's [`Error`]s, and [`ErrorKind::TooLarge`], [`ErrorKind::TooManyInstances`],
    /// [`ErrorKind::TooManyRecords`] or [`ErrorKind::TooManyParams`] past a total.
    pub fn read<S: ByteSource>(
        source: S,
        options: Options,
        totals: Totals,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(source, options, meter);
        let mut exchange = Self::default();
        let mut instances = 0_usize;
        let mut records = 0_usize;
        let mut params = 0_usize;
        while let Some(event) = reader.next_event()? {
            if reader.offset() > totals.bytes {
                return Err(reader.error(ErrorKind::TooLarge));
            }
            params = params.saturating_add(reader.tokens().params());
            if params > totals.params {
                return Err(reader.error(ErrorKind::TooManyParams));
            }
            match event {
                Event::HeaderRecord => {
                    for record in reader.tokens().records() {
                        exchange.header.take(record);
                    }
                }
                Event::Instance(id) => {
                    // A repeat's records were read too, so it counts as an instance.
                    instances = instances.saturating_add(1);
                    if instances > totals.instances {
                        return Err(reader.error(ErrorKind::TooManyInstances));
                    }
                    records = records.saturating_add(reader.tokens().record_count());
                    if records > totals.records {
                        return Err(reader.error(ErrorKind::TooManyRecords));
                    }
                    // A repeated id is invalid; some writers emit them for geometry nobody
                    // reads. Keep the first definition and report it.
                    match exchange.entities.entry(id) {
                        Entry::Occupied(_) => exchange.duplicates.push(id),
                        Entry::Vacant(slot) => {
                            slot.insert(Entity {
                                records: reader.tokens().records(),
                            });
                        }
                    }
                }
                Event::Section(_) | Event::EndSection | Event::Skipped(_) => {}
            }
        }
        if reader.offset() > totals.bytes {
            return Err(reader.error(ErrorKind::TooLarge));
        }
        exchange.replaced = reader.replaced();
        Ok(exchange)
    }

    /// Writes the file canonically to `sink`: the header, then one `DATA` section of every
    /// instance in id order.
    ///
    /// # Errors
    ///
    /// The writer's [`WriteError`]s: a value Part 21 cannot hold, or the sink's failure.
    pub fn write<K: ByteSink>(&self, sink: K) -> Result<K, WriteError> {
        let mut writer = Writer::new(sink);
        writer.begin()?;
        for record in &self.header.records() {
            writer.header_record(record)?;
        }
        for record in &self.header.other {
            writer.header_record(record)?;
        }
        writer.end_section()?;
        writer.data()?;
        for (id, entity) in &self.entities {
            writer.instance(*id, &entity.records)?;
        }
        writer.end_section()?;
        writer.finish()
    }
}
