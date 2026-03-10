//! Aggregation example — demonstrates the pivoted aggregate endpoint.
//!
//! This example shows how to configure time-series aggregation on a Sea-ORM entity
//! and how the pivoted response format works.
//!
//! Run with: cargo run --example aggregation --features "aggregation"
//!
//! Note: The aggregate endpoint requires TimescaleDB (PostgreSQL). This example
//! uses SQLite for demonstration, so the `/aggregate` endpoint will return 500.
//! The pivot logic is demonstrated with sample data below.

use axum::Router;
use crudcrate::aggregation::{PivotConfig, pivot_to_columnar};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection};
use serde_json::json;

// ============================================================
// Entity: SensorReading (CRUD + aggregate)
// ============================================================

mod sensor_reading {
    use super::*;
    use chrono::{DateTime, Utc};
    use crudcrate::traits::CRUDResource;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "sensor_readings")]
    #[crudcrate(
        api_struct = "SensorReading",
        generate_router,
        aggregate(
            time_column = "recorded_at",
            intervals("1 hour", "1 day", "1 week"),
            metrics("value"),
            group_by("site_id"),
            aggregates(avg, min, max),
        )
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub site_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub recorded_at: DateTime<Utc>,

        #[crudcrate(filterable, sortable)]
        pub value: f64,

        #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================
// Entity: Reading (aggregate-only, no CRUD)
// ============================================================

mod reading {
    use super::*;
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "readings")]
    #[crudcrate(
        api_struct = "ReadingApi",
        aggregate(
            time_column = "time",
            intervals("1 hour", "1 day", "1 week", "1 month"),
            metrics("value"),
            group_by("parameter_id"),
        )
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(filterable)]
        pub parameter_id: Uuid,

        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(filterable, sortable)]
        pub time: DateTime<Utc>,

        #[crudcrate(filterable)]
        pub value: f64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================
// Database setup
// ============================================================

async fn setup_database(url: &str) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let db = Database::connect(url).await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS sensor_readings (
            id TEXT PRIMARY KEY NOT NULL,
            site_id TEXT NOT NULL,
            recorded_at TEXT NOT NULL,
            value REAL NOT NULL,
            created_at TEXT NOT NULL
        );"
        .to_owned(),
    ))
    .await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS readings (
            parameter_id TEXT NOT NULL,
            time TEXT NOT NULL,
            value REAL NOT NULL,
            PRIMARY KEY (parameter_id, time)
        );"
        .to_owned(),
    ))
    .await?;

    Ok(db)
}

// ============================================================
// Main
// ============================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_database("sqlite::memory:").await?;

    // ----------------------------------------------------------
    // Demonstrate the pivot logic with sample data
    // ----------------------------------------------------------
    println!("=== Pivoted Aggregate Response Demo ===\n");

    // Simulate flat rows that would come from TimescaleDB's time_bucket()
    let flat_rows = vec![
        json!({
            "bucket": "2024-01-01T00:00:00Z",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440001",
            "avg_value": 22.5, "min_value": 20.0, "max_value": 25.0, "count": 60
        }),
        json!({
            "bucket": "2024-01-01T01:00:00Z",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440001",
            "avg_value": 23.1, "min_value": 21.0, "max_value": 26.0, "count": 60
        }),
        json!({
            "bucket": "2024-01-01T00:00:00Z",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440002",
            "avg_value": 7.2, "min_value": 6.8, "max_value": 7.5, "count": 60
        }),
        // parameter 2 has no data at 01:00 -- will be null-filled
    ];

    // Create a PivotConfig (normally generated by the proc macro via pivot_config())
    let config = PivotConfig {
        metrics: vec!["value".to_string()],
        aggregates: vec!["avg".to_string(), "min".to_string(), "max".to_string()],
        group_by: vec!["parameter_id".to_string()],
        resolution: "1 hour".to_string(),
    };

    let pivoted = pivot_to_columnar(
        &flat_rows,
        &config,
        Some("2024-01-01T00:00:00Z"),
        Some("2024-01-01T02:00:00Z"),
    );

    println!("{}\n", serde_json::to_string_pretty(&pivoted)?);

    println!("Notice how parameter 2 has null values at 01:00 -- sparse data is null-filled.");
    println!("All groups share the same time axis.\n");

    // ----------------------------------------------------------
    // Set up the router
    // ----------------------------------------------------------
    let app = Router::new()
        // SensorReading: full CRUD + aggregate endpoint
        .nest(
            "/sensor_readings",
            sensor_reading::SensorReading::router(&db).into(),
        )
        // Reading: aggregate-only endpoint (no CRUD)
        .nest(
            "/readings",
            reading::ReadingApi::aggregate_router(&db).into(),
        );

    let bind_addr = "127.0.0.1:3000";
    let base = format!("http://{bind_addr}");

    println!("=== Available Endpoints ===\n");
    println!("CRUD + Aggregate (SensorReading):");
    println!("  GET    {base}/sensor_readings              - List all");
    println!("  GET    {base}/sensor_readings/{{id}}          - Get one");
    println!("  POST   {base}/sensor_readings              - Create");
    println!("  PUT    {base}/sensor_readings/{{id}}          - Update");
    println!("  DELETE {base}/sensor_readings/{{id}}          - Delete");
    println!("  GET    {base}/sensor_readings/aggregate     - Aggregate (requires TimescaleDB)");
    println!();
    println!("Aggregate-only (Reading):");
    println!("  GET    {base}/readings/aggregate            - Aggregate (requires TimescaleDB)");
    println!();
    println!("Example aggregate query:");
    println!("  {base}/readings/aggregate?interval=1%20hour&start=2024-01-01&end=2024-02-01");
    println!();
    println!("Note: aggregate endpoints require TimescaleDB. On SQLite they return 500.");
    println!();

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    println!("Listening on {base}");

    axum::serve(listener, app).await?;

    Ok(())
}
