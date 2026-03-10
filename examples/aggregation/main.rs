//! Aggregation example — demonstrates the pivoted aggregate endpoint with TimescaleDB.
//!
//! # Prerequisites
//!
//! Start a TimescaleDB instance:
//!
//! ```bash
//! docker run -d --name timescaledb -p 5432:5432 \
//!   -e POSTGRES_PASSWORD=password \
//!   -e POSTGRES_DB=crudcrate_example \
//!   timescale/timescaledb:latest-pg17
//! ```
//!
//! # Run
//!
//! ```bash
//! DATABASE_URL="postgres://postgres:password@localhost:5432/crudcrate_example" \
//!   cargo run --example aggregation --features "aggregation"
//! ```

use axum::Router;
use chrono::{DateTime, Utc};
use crudcrate::aggregation::{PivotConfig, pivot_to_columnar};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection};
use serde_json::json;
use uuid::Uuid;

// ============================================================
// Entity: SensorReading (CRUD + aggregate)
// ============================================================

mod sensor_reading {
    use super::*;
    use crudcrate::traits::CRUDResource;

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

async fn setup_database(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    // Enable TimescaleDB extension
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        "CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE".to_owned(),
    ))
    .await?;

    // Create sensor_readings table + hypertable
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE IF NOT EXISTS sensor_readings (
            id UUID PRIMARY KEY NOT NULL,
            site_id UUID NOT NULL,
            recorded_at TIMESTAMPTZ NOT NULL,
            value DOUBLE PRECISION NOT NULL,
            created_at TIMESTAMPTZ NOT NULL
        )"#
        .to_owned(),
    ))
    .await?;

    // Convert to hypertable (ignore error if already a hypertable)
    let _ = db
        .execute(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT create_hypertable('sensor_readings', 'recorded_at', if_not_exists => TRUE)"
                .to_owned(),
        ))
        .await;

    // Create readings table + hypertable
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE IF NOT EXISTS readings (
            parameter_id UUID NOT NULL,
            time TIMESTAMPTZ NOT NULL,
            value DOUBLE PRECISION NOT NULL,
            PRIMARY KEY (parameter_id, time)
        )"#
        .to_owned(),
    ))
    .await?;

    let _ = db
        .execute(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT create_hypertable('readings', 'time', if_not_exists => TRUE)".to_owned(),
        ))
        .await;

    Ok(())
}

async fn seed_sample_data(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Duration;

    let param_a = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let param_b = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440002").unwrap();
    let base_time = "2024-06-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();

    // Insert 48 hours of hourly readings for two parameters
    let mut values = Vec::new();
    for hour in 0..48 {
        let t = base_time + Duration::hours(hour);
        let ts = t.to_rfc3339();

        // Parameter A: temperature-like values (20-25 range)
        let val_a = 22.0 + (hour as f64 * 0.1).sin() * 3.0;
        values.push(format!("('{param_a}', '{ts}', {val_a})"));

        // Parameter B: pH-like values (6.5-7.5 range), skip some hours for sparse data
        if hour % 3 != 2 {
            let val_b = 7.0 + (hour as f64 * 0.2).cos() * 0.5;
            values.push(format!("('{param_b}', '{ts}', {val_b})"));
        }
    }

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        format!(
            "INSERT INTO readings (parameter_id, time, value) VALUES {} ON CONFLICT DO NOTHING",
            values.join(", ")
        ),
    ))
    .await?;

    println!("Seeded 48 hours of readings for 2 parameters (parameter B has sparse data).\n");

    Ok(())
}

// ============================================================
// Main
// ============================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("ERROR: DATABASE_URL environment variable is required.\n");
            eprintln!("TimescaleDB is needed for the aggregate endpoint. Start one with:\n");
            eprintln!("  docker run -d --name timescaledb -p 5432:5432 \\");
            eprintln!("    -e POSTGRES_PASSWORD=password \\");
            eprintln!("    -e POSTGRES_DB=crudcrate_example \\");
            eprintln!("    timescale/timescaledb:latest-pg17\n");
            eprintln!("Then run:\n");
            eprintln!("  DATABASE_URL=\"postgres://postgres:password@localhost:5432/crudcrate_example\" \\");
            eprintln!("    cargo run --example aggregation --features \"aggregation\"");
            std::process::exit(1);
        }
    };

    let db = Database::connect(&database_url).await?;
    setup_database(&db).await?;
    seed_sample_data(&db).await?;

    // ----------------------------------------------------------
    // Demonstrate pivot_to_columnar() with hardcoded data
    // ----------------------------------------------------------
    println!("=== pivot_to_columnar() Demo (hardcoded data) ===\n");

    let flat_rows = vec![
        json!({
            "bucket": "2024-01-01T00:00:00+00:00",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440001",
            "avg_value": 22.5, "min_value": 20.0, "max_value": 25.0, "count": 60
        }),
        json!({
            "bucket": "2024-01-01T01:00:00+00:00",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440001",
            "avg_value": 23.1, "min_value": 21.0, "max_value": 26.0, "count": 60
        }),
        json!({
            "bucket": "2024-01-01T00:00:00+00:00",
            "parameter_id": "550e8400-e29b-41d4-a716-446655440002",
            "avg_value": 7.2, "min_value": 6.8, "max_value": 7.5, "count": 60
        }),
        // parameter 2 has no data at 01:00 -- will be null-filled
    ];

    let config = PivotConfig {
        metrics: vec!["value".to_string()],
        aggregates: vec!["avg".to_string(), "min".to_string(), "max".to_string()],
        group_by: vec!["parameter_id".to_string()],
        resolution: "1 hour".to_string(),
    };

    let pivoted = pivot_to_columnar(
        &flat_rows,
        &config,
        Some("2024-01-01T00:00:00+00:00"),
        Some("2024-01-01T02:00:00+00:00"),
    );

    println!("{}\n", serde_json::to_string_pretty(&pivoted)?);
    println!("Notice: parameter 2 has null at 01:00 — sparse data is null-filled.\n");

    // ----------------------------------------------------------
    // Demonstrate aggregate_query() hitting real TimescaleDB
    // ----------------------------------------------------------
    println!("=== aggregate_query() against TimescaleDB ===\n");

    let params = crudcrate::aggregation::AggregateParams {
        interval: "1 day".to_string(),
        start: Some("2024-06-01".to_string()),
        end: Some("2024-06-03".to_string()),
        filter: None,
        timezone: None,
    };

    match reading::ReadingApi::aggregate_query(&db, &params).await {
        Ok(flat) => {
            println!("Flat rows from aggregate_query():");
            for row in &flat {
                println!("  {row}");
            }

            let pivot_config = reading::ReadingApi::pivot_config(&params.interval);
            let pivoted = pivot_to_columnar(
                &flat,
                &pivot_config,
                params.start.as_deref(),
                params.end.as_deref(),
            );
            println!("\nPivoted response:");
            println!("{}\n", serde_json::to_string_pretty(&pivoted)?);
        }
        Err(e) => {
            eprintln!("aggregate_query() error: {e}");
            eprintln!("(This is expected if TimescaleDB extension is not available)\n");
        }
    }

    // ----------------------------------------------------------
    // Set up the router and start server
    // ----------------------------------------------------------
    let app = Router::new()
        .nest(
            "/sensor_readings",
            sensor_reading::SensorReading::router(&db).into(),
        )
        .nest(
            "/readings",
            reading::ReadingApi::aggregate_router(&db).into(),
        );

    let bind_addr = "127.0.0.1:3000";
    let base = format!("http://{bind_addr}");

    println!("=== Available Endpoints ===\n");
    println!("CRUD + Aggregate (SensorReading):");
    println!("  GET    {base}/sensor_readings");
    println!("  GET    {base}/sensor_readings/{{id}}");
    println!("  POST   {base}/sensor_readings");
    println!("  PUT    {base}/sensor_readings/{{id}}");
    println!("  DELETE {base}/sensor_readings/{{id}}");
    println!("  GET    {base}/sensor_readings/aggregate?interval=1%20hour");
    println!();
    println!("Aggregate-only (Reading):");
    println!("  GET    {base}/readings/aggregate?interval=1%20hour");
    println!("  GET    {base}/readings/aggregate?interval=1%20day&start=2024-06-01&end=2024-06-03");
    println!();

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    println!("Listening on {base}");

    axum::serve(listener, app).await?;

    Ok(())
}
