//! Claims, not files: starting to work on a piece takes a light, automatic claim on it, and
//! conflicts are caught per piece. A claim ends when its holder lets go or when its time runs
//! out, so someone who walks away can't lock anyone out. Taking one is part of the edit's own
//! transaction, and its time comes from the log, never from a machine's clock.

use kit_core::Id;
use kit_ops::{Claim, ClaimChange};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};

fn place(project: &Id, document: &Id, piece: &Id) -> Place {
    Place::Claim {
        project: project.clone(),
        document: document.clone(),
        piece: piece.clone(),
    }
}

/// Who else holds a piece now, if anyone. A claim that has run out holds nothing.
pub async fn held_by_another<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    piece: &Id,
    person: &Id,
    now_ms: u64,
) -> Result<Option<Claim>, StoreError> {
    let claim: Option<Claim> = read_row(reader, &place(project, document, piece).key()).await?;
    Ok(claim.filter(|claim| claim.person != *person && claim.until_ms > now_ms))
}

/// Takes a claim, or renews one this person holds.
pub async fn take<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    piece: &Id,
    claim: Claim,
) -> Result<ClaimChange, StoreError> {
    txn.put(place(project, document, piece).key(), to_row(&claim)?)
        .await?;
    Ok(ClaimChange {
        piece: piece.clone(),
        claim: Some(claim),
    })
}

/// Lets a claim go.
pub async fn release<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    piece: &Id,
) -> Result<ClaimChange, StoreError> {
    txn.delete(place(project, document, piece).key()).await?;
    Ok(ClaimChange {
        piece: piece.clone(),
        claim: None,
    })
}
