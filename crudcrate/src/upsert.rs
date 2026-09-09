//! Registration: find-or-insert a row on an alternate unique key, and say what it did.
//!
//! A source system that re-sends its own content needs the write to be idempotent and the answer
//! to say whether the row is new, changed or already stood, keyed by what the sender holds rather
//! than by position in the request. That is a different shape from [`crate::BatchResult`], which is
//! per-item success-or-error: an item can be stored and still have something to report, and a
//! status a client branches on is not an error string.

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    IdenStatic, IntoActiveModel, ModelTrait, QueryFilter, TransactionSession, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ApiError;
use crate::core::traits::CRUDResource;

/// What a registration did to one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpsertStatus {
    /// The key was not stored here before.
    Created,
    /// The key was stored and at least one of the sent columns differed.
    Updated,
    /// The key was stored and every sent column already read as sent.
    Unchanged,
}

/// One item's outcome, keyed by what the sender holds.
///
/// `status` is generic so that a caller with refusals of its own (a row it kept against what the
/// source sent, a row whose owner does not exist here) reports them in the same list rather than
/// as failures. `note` is what an accepted write has to say beyond its status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UpsertOutcome<K, I, S = UpsertStatus> {
    pub key: K,
    /// The row's own id, absent where nothing was stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<I>,
    pub status: S,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Register one row under the resource's [`CRUDResource::upsert_key`], in one transaction.
///
/// The caller supplies an active model with every key column set. Columns declared by
/// [`CRUDResource::upsert_comparable`] are compared and updated only when set on that model.
/// Equal content returns `Unchanged` without writing; `on_update` fields advance on a change.
///
/// # Errors
///
/// Returns `ApiError::bad_request` where the resource declares no key or the model leaves a key
/// column unset, and the database's error where a statement fails.
pub async fn upsert<R, C: ConnectionTrait + TransactionTrait>(
    db: &C,
    active: R::ActiveModelType,
) -> Result<(R, UpsertStatus), ApiError>
where
    R: CRUDResource,
    <R::EntityType as EntityTrait>::Model: IntoActiveModel<R::ActiveModelType>,
{
    let key = R::upsert_key();
    if key.is_empty() {
        return Err(ApiError::bad_request(format!(
            "{} declares no registration key",
            R::RESOURCE_NAME_SINGULAR
        )));
    }

    let mut condition = Condition::all();
    for column in key {
        match active.get(*column) {
            ActiveValue::Set(value) | ActiveValue::Unchanged(value) => {
                condition = condition.add(column.eq(value));
            }
            ActiveValue::NotSet => {
                return Err(ApiError::bad_request(format!(
                    "{} was registered without {:?}, which is part of its key",
                    R::RESOURCE_NAME_SINGULAR,
                    column
                )));
            }
        }
    }

    let txn = db.begin().await.map_err(ApiError::database)?;
    let stored = R::EntityType::find()
        .filter(condition)
        .one(&txn)
        .await
        .map_err(ApiError::database)?;

    let (model, status) = match stored {
        None => {
            let model = active.insert(&txn).await.map_err(ApiError::database)?;
            (model, UpsertStatus::Created)
        }
        Some(stored) if sends_nothing_new::<R>(&stored, &active) => {
            (stored, UpsertStatus::Unchanged)
        }
        Some(stored) => {
            let mut merged = stored.into_active_model();
            for column in sent_columns::<R>(&active) {
                // A key column identifies the row; only what the source says about it is written.
                if key.iter().any(|k| k.as_str() == column.as_str()) {
                    continue;
                }
                if let ActiveValue::Set(value) = active.get(column) {
                    merged.set(column, value);
                }
            }
            R::apply_on_update(&mut merged);
            let model = merged.update(&txn).await.map_err(ApiError::database)?;
            (model, UpsertStatus::Updated)
        }
    };
    txn.commit().await.map_err(ApiError::database)?;
    Ok((R::from(model), status))
}

/// Comparable columns set on the active model.
fn sent_columns<R>(active: &R::ActiveModelType) -> Vec<<R::EntityType as EntityTrait>::Column>
where
    R: CRUDResource,
{
    R::upsert_comparable()
        .iter()
        .copied()
        .filter(|column| matches!(active.get(*column), ActiveValue::Set(_)))
        .collect()
}

/// Whether every column the source sent already reads as sent on the stored row.
fn sends_nothing_new<R>(
    stored: &<R::EntityType as EntityTrait>::Model,
    active: &R::ActiveModelType,
) -> bool
where
    R: CRUDResource,
{
    sent_columns::<R>(active).into_iter().all(|column| {
        matches!(active.get(column), ActiveValue::Set(value) if stored.get(column) == value)
    })
}

#[cfg(test)]
mod tests {
    use super::{UpsertOutcome, UpsertStatus};

    /// A refusal of the caller's own, reported in the same list as the writes.
    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum Reported {
        Unchanged,
        OwnerUnknownHere,
    }

    #[test]
    fn an_outcome_carrying_nothing_beyond_its_status_serializes_key_and_status_alone() {
        let outcome: UpsertOutcome<String, uuid::Uuid> = UpsertOutcome {
            key: "DOC:plate-7".to_string(),
            id: None,
            status: UpsertStatus::Unchanged,
            note: None,
        };
        let json = serde_json::to_value(&outcome).expect("serializes");
        assert_eq!(
            json,
            serde_json::json!({"key": "DOC:plate-7", "status": "unchanged"})
        );
    }

    #[test]
    fn an_outcome_reports_the_row_it_wrote_and_what_it_has_to_say() {
        let id = uuid::Uuid::nil();
        let outcome = UpsertOutcome {
            key: "DOC:plate-7".to_string(),
            id: Some(id),
            status: UpsertStatus::Updated,
            note: Some("slope replaced".to_string()),
        };
        let json = serde_json::to_value(&outcome).expect("serializes");
        assert_eq!(json["id"], serde_json::json!(id));
        assert_eq!(json["status"], serde_json::json!("updated"));
        assert_eq!(json["note"], serde_json::json!("slope replaced"));
    }

    /// The status is generic so a caller's own refusals sit in one list with the writes, which is
    /// what `BatchResult`'s success-or-error shape cannot express.
    #[test]
    fn a_caller_reports_its_own_statuses_in_the_same_list() {
        let reported: Vec<UpsertOutcome<&str, uuid::Uuid, Reported>> = vec![
            UpsertOutcome {
                key: "DOC:plate-7",
                id: Some(uuid::Uuid::nil()),
                status: Reported::Unchanged,
                note: None,
            },
            UpsertOutcome {
                key: "DOC:plate-9",
                id: None,
                status: Reported::OwnerUnknownHere,
                note: Some("no such station here".to_string()),
            },
        ];
        let json = serde_json::to_value(&reported).expect("serializes");
        assert_eq!(json[0]["status"], serde_json::json!("unchanged"));
        assert_eq!(json[1]["status"], serde_json::json!("owner_unknown_here"));
        assert!(
            json[1].get("id").is_none(),
            "nothing was stored, so there is no id to report"
        );
    }
}
