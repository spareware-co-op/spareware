//! How a row is written: JSON, with its format version around it, `{"v":1,"row":...}`. JSON
//! because a row must stay readable in decades, by people, with no Spareware program to hand.
//! Reading goes through [`from_row`] alone, so when a version 2 comes, this is where version 1 is
//! upgraded, and a row from a newer program is refused rather than half understood.

use crate::key::{Key, Prefix};
use crate::kv::{Reader, StoreError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The version every row is written with now.
pub const ROW_FORMAT: u32 = 1;

#[derive(Serialize)]
struct Writing<'a, T> {
    v: u32,
    row: &'a T,
}

#[derive(Deserialize)]
struct Version {
    v: u32,
}

#[derive(Deserialize)]
struct Reading<T> {
    row: T,
}

/// A value as a stored row.
pub fn to_row<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    serde_json::to_vec(&Writing {
        v: ROW_FORMAT,
        row: value,
    })
    .map_err(|e| StoreError::Damaged(e.to_string()))
}

/// Reads a stored row of any version. Bad bytes are an error, never a panic.
pub fn from_row<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    let damaged = |e: serde_json::Error| StoreError::Damaged(e.to_string());
    let version: Version = serde_json::from_slice(bytes).map_err(damaged)?;
    match version.v {
        1 => Ok(serde_json::from_slice::<Reading<T>>(bytes)
            .map_err(damaged)?
            .row),
        found => Err(StoreError::TooNew {
            found,
            known: ROW_FORMAT,
        }),
    }
}

/// Reads one row, if it is there.
pub async fn read_row<T: DeserializeOwned, R: Reader>(
    reader: &mut R,
    key: &Key,
) -> Result<Option<T>, StoreError> {
    match reader.get(key).await? {
        Some(bytes) => from_row(&bytes).map(Some),
        None => Ok(None),
    }
}

/// Reads every row in a range.
pub async fn scan_rows<T: DeserializeOwned, R: Reader>(
    reader: &mut R,
    prefix: &Prefix,
) -> Result<Vec<T>, StoreError> {
    let pairs = reader.scan(prefix).await?;
    pairs.iter().map(|(_, value)| from_row(value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_says_its_version() {
        assert_eq!(to_row(&"hello").unwrap(), br#"{"v":1,"row":"hello"}"#);
    }

    #[test]
    fn a_row_reads_back() {
        assert_eq!(from_row::<u32>(&to_row(&7u32).unwrap()), Ok(7));
    }

    #[test]
    fn a_row_from_a_newer_program_is_refused() {
        let err = from_row::<u32>(br#"{"v":2,"row":7}"#);
        assert_eq!(err, Err(StoreError::TooNew { found: 2, known: 1 }));
    }

    #[test]
    fn damage_is_an_error() {
        for bad in [
            &b""[..],
            b"{",
            b"[]",
            br#"{"v":"1"}"#,
            br#"{"v":1}"#,
            br#"{"row":7}"#,
        ] {
            assert!(
                matches!(from_row::<u32>(bad), Err(StoreError::Damaged(_))),
                "{bad:?}"
            );
        }
    }
}
