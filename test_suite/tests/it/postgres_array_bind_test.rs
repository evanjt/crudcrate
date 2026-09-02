//! Binding an array of timestamps as a single `timestamptz[]` parameter.

// `sea-query-binder` 0.7.0 downcast the `ArrayType::ChronoDateTimeWithTimeZone` arm to
// `Vec<DateTime<Local>>`, so every correctly-typed array panicked inside argument
// binding, before the statement reached Postgres. The stack this workspace requires
// (sea-orm 2.0 / sea-query-sqlx 0.9) downcasts to `Vec<DateTime<FixedOffset>>`. These
// tests pin that: the first on every backend, the second on the wire.

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use sea_orm::sea_query::{ArrayType, ValueType};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, Value};
use uuid::Uuid;

fn stamps() -> Vec<DateTime<FixedOffset>> {
    vec![
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap().into(),
        Utc.with_ymd_and_hms(2026, 2, 1, 13, 45, 30).unwrap().into(),
    ]
}

/// The array type a `Vec<T>` selects must be the one the Postgres binder downcasts
/// back to. A disagreement here is what made the 0.7.0 binder panic.
#[test]
fn chrono_vecs_select_the_array_type_the_binder_downcasts_to() {
    let value: Value = stamps().into();
    assert!(
        matches!(
            value,
            Value::Array(ArrayType::ChronoDateTimeWithTimeZone, _)
        ),
        "expected a timestamptz array, got {value:?}"
    );
    let round_trip = <Vec<DateTime<FixedOffset>> as ValueType>::try_from(value)
        .expect("the binder's downcast must accept the array it was given");
    assert_eq!(round_trip, stamps());

    let utc: Value = vec![Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()].into();
    assert!(matches!(utc, Value::Array(ArrayType::ChronoDateTimeUtc, _)));

    let ids: Value = vec![Uuid::nil()].into();
    assert!(matches!(ids, Value::Array(ArrayType::Uuid, _)));
}

#[tokio::test]
async fn timestamptz_array_binds_and_round_trips_on_postgres() {
    let url = test_suite::database_url();
    if !url.starts_with("postgres") {
        eprintln!("Skipping: not Postgres");
        return;
    }
    let db = Database::connect(&url).await.expect("connect");
    assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT x FROM unnest($1::timestamptz[]) AS x",
            [Value::from(stamps())],
        ))
        .await
        .expect("binding a timestamptz array must reach the database");

    let returned: Vec<DateTime<FixedOffset>> = rows
        .iter()
        .map(|row| row.try_get_by_index(0).expect("timestamptz column"))
        .collect();
    assert_eq!(returned, stamps());
}
