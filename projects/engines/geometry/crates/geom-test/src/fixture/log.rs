//! The operation log of a fixture: its stored inputs and its operations, as one standalone `.skgf`
//! file (chapter 28.16: "operations and inputs, in the native format"; piece G97).
//!
//! What it does: an [`OpLog`] holds the document's healing distance, the records of its stored
//! inputs (bodies a log starts from, by content ID) and its [`Operation`]s, in order.
//! [`OpLog::encode`] writes the file: the header, the inputs, one operation record each, then END;
//! [`OpLog::decode`] reads it back.
//!
//! Guarantees:
//!
//! - **The native format, and nothing else.** The file is what `geom_format::file` writes, so a log
//!   also seeds the readers' fuzz corpora and lands in the format museum unchanged.
//! - **One encoding.** Inputs come before operations, and [`OpLog::decode`] refuses bytes that are
//!   not [`OpLog::encode`]'s, so a log has one spelling and a fixture's text one form.
//! - **Versions come from the log**: [`OpLog::versions`] and [`OpLog::at_versions`] are how a
//!   fixture's keys and a replay at another version are found.
//!
//! Method: chapter 24.2's file and record layers, chapter 21's replay
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`,
//! `projects/engines/geometry/architecture/21-determinism.md`).
//!
//! Cost: linear in the log.

use core::fmt;

use geom_format::file::{self, FileError, HEADER_TAG};
use geom_format::header::{Header, HeaderError, rung};
use geom_format::operation::{
    OPERATION_TAG, Operation, OperationError, operation_record, read_operation,
};
use geom_format::record::Record;
use geom_format::source::SliceSource;
use geom_num::Steps;
use geom_op::cbor::EncodeError;
use geom_run::{Budget, WorkMeter};

use super::OpVersions;
use super::run::Ran;

/// The program a log's header names.
pub const PROGRAM: &str = "spare-kit fixture";

/// One operation log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpLog {
    /// The document's healing distance, part of every operation's context.
    pub heal: Steps,
    /// The stored inputs: records of the bodies the log starts from.
    pub inputs: Vec<Record>,
    /// The operations, in order.
    pub operations: Vec<Operation>,
}

/// Why a log cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum LogError {
    /// The file layer refused it.
    File(FileError),
    /// The header cannot be read or written.
    Header(HeaderError),
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
    /// An operation record cannot be read or written.
    Operation(OperationError),
    /// An operation before a stored input, or a header without a healing distance.
    Shape(&'static str),
    /// The bytes are a log, but not its one encoding.
    NotCanonical,
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::File(e) => write!(f, "{e}"),
            LogError::Header(e) => write!(f, "{e}"),
            LogError::Encode(e) => write!(f, "{e}"),
            LogError::Operation(e) => write!(f, "{e}"),
            LogError::Shape(what) => f.write_str(what),
            LogError::NotCanonical => f.write_str("the bytes are not the log's one encoding"),
        }
    }
}

impl core::error::Error for LogError {}

impl From<FileError> for LogError {
    fn from(e: FileError) -> Self {
        LogError::File(e)
    }
}

impl From<HeaderError> for LogError {
    fn from(e: HeaderError) -> Self {
        LogError::Header(e)
    }
}

impl From<EncodeError> for LogError {
    fn from(e: EncodeError) -> Self {
        LogError::Encode(e)
    }
}

impl From<OperationError> for LogError {
    fn from(e: OperationError) -> Self {
        LogError::Operation(e)
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

impl OpLog {
    /// A log with `inputs` and `operations`, at the default healing distance.
    #[must_use]
    pub fn new(inputs: Vec<Record>, operations: Vec<Operation>) -> OpLog {
        OpLog {
            heal: geom_num::ladder::HEAL_DEFAULT,
            inputs,
            operations,
        }
    }

    /// The log's file.
    ///
    /// # Errors
    ///
    /// [`LogError`] for an operation or record with no encoding.
    pub fn encode(&self) -> Result<Vec<u8>, LogError> {
        let header = Header::v1(PROGRAM, self.heal).record()?;
        let mut records = self.inputs.clone();
        for operation in &self.operations {
            records.push(operation_record(operation, &mut meter())?);
        }
        Ok(file::write(&header, &records, &mut meter())?)
    }

    /// Reads a log's file.
    ///
    /// # Errors
    ///
    /// [`LogError`] for bytes that are not a log, or not its one encoding.
    pub fn decode(bytes: &[u8]) -> Result<OpLog, LogError> {
        let read = file::read(&mut SliceSource::new(bytes), |_, _| true, &mut meter())?;
        if read.header.tag != HEADER_TAG {
            return Err(LogError::Shape("a log opens with its header"));
        }
        let header = Header::read(&read.header.payload, &mut meter())?;
        let heal = header
            .ladder
            .get(&rung::HEAL)
            .and_then(|v| u64::try_from(*v).ok())
            .ok_or(LogError::Shape("the header holds no healing distance"))?;
        let mut inputs = Vec::new();
        let mut operations = Vec::new();
        for record in read.records {
            if record.tag == OPERATION_TAG {
                operations.push(read_operation(&record.payload, &mut meter())?);
            } else if operations.is_empty() {
                inputs.push(record);
            } else {
                return Err(LogError::Shape("a stored input after an operation"));
            }
        }
        let log = OpLog {
            heal: Steps(heal),
            inputs,
            operations,
        };
        if log.encode()? == bytes {
            Ok(log)
        } else {
            Err(LogError::NotCanonical)
        }
    }

    /// Adds `operation` to the log unless it must not stay there: `ran` says how it ended. A run
    /// stopped by its caller or its machine (`Cancelled`, `OutOfMemory`) is never the operation's
    /// result (28.13), and an operation the engine refused never ran. Whether it was added.
    pub fn record(&mut self, operation: &Operation, ran: &Ran) -> bool {
        let stays = match ran {
            Ran::Done(_) => true,
            Ran::Failed(error, _) => error.kind().is_replayable(),
            Ran::Refused(_) => false,
        };
        if stays {
            self.operations.push(operation.clone());
        }
        stays
    }

    /// The operation versions the log names. An operation that runs twice at two versions keeps
    /// the later one here; [`OpLog::at_versions`] gives every run of a name one version.
    #[must_use]
    pub fn versions(&self) -> OpVersions {
        self.operations
            .iter()
            .map(|o| (o.name.clone(), o.version))
            .collect()
    }

    /// The same log with each operation at the version `versions` names; a name it does not name
    /// keeps its own.
    #[must_use]
    pub fn at_versions(&self, versions: &OpVersions) -> OpLog {
        let mut log = self.clone();
        for operation in &mut log.operations {
            if let Some(version) = versions.get(&operation.name) {
                operation.version = *version;
            }
        }
        log
    }
}
