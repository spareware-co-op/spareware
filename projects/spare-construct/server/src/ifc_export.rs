//! The model as an IFC 4.3 file, to download: the route's handler (`GET .../export/ifc`, wired by
//! one line in `http.rs`) and the two small helpers the service's `export_ifc` uses. The file
//! itself is written by construct-interop from what `construct_api::cloud::ifc` reads.

use crate::http::{ApiError, Me, Shared, id};
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;

/// The model as an IFC 4.3 file, to download.
pub(crate) async fn export_ifc(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let (name, text) = app
        .service
        .export_ifc(&person, &id(&project)?, &id(&model)?)
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-step".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        text,
    ))
}

/// The time now as ISO 8601 text to the second, in UTC (`2026-11-28T09:30:00`): the file header's
/// stamp, the one part of an IFC export that differs between two exports of one model.
pub(crate) fn iso_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    let at = time::OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// A model's name as a file's: letters, digits, spaces, dots, dashes and underscores kept, every
/// other character an underscore, and `model` for an empty name.
pub(crate) fn file_stem(name: &str) -> String {
    let kept: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let kept = kept.trim().to_owned();
    if kept.is_empty() {
        "model".to_owned()
    } else {
        kept
    }
}
