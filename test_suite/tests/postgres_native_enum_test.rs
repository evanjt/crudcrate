// Postgres Native ENUM Test
// Tests that crudcrate works with REAL Postgres ENUM column types (not TEXT).
// This test ONLY runs when DATABASE_URL points to Postgres — it's skipped on SQLite/MySQL.
//
// This validates the full chain:
// 1. CREATE TYPE ... AS ENUM in Postgres
// 2. Sea-ORM DeriveActiveEnum with db_type = "Enum" for INSERT/UPDATE
// 3. Crudcrate's CAST(col AS TEXT) + UPPER filtering on native ENUM columns

use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseBackend, DbErr, Set};
use serde::{Deserialize, Serialize};

/// A standalone enum using db_type = "Enum" — the native Postgres path
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "color")]
pub enum Color {
    #[sea_orm(string_value = "Red")]
    Red,
    #[sea_orm(string_value = "Green")]
    Green,
    #[sea_orm(string_value = "Blue")]
    Blue,
}

/// Minimal entity to test native enum columns
mod widget {
    use super::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "widgets")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub name: String,
        pub color: Color,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

fn is_postgres() -> bool {
    get_database_url().starts_with("postgres")
}

async fn setup_postgres_enum_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Create the Postgres ENUM type and table with raw SQL
    db.execute_unprepared("DROP TABLE IF EXISTS widgets")
        .await?;
    db.execute_unprepared("DROP TYPE IF EXISTS color").await?;
    db.execute_unprepared("CREATE TYPE color AS ENUM ('Red', 'Green', 'Blue')")
        .await?;
    db.execute_unprepared(
        "CREATE TABLE widgets (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL,
            color color NOT NULL
        )",
    )
    .await?;
    Ok(())
}

/// Test that Sea-ORM 1.1.19 can INSERT into a native Postgres ENUM column
/// when the Rust enum uses db_type = "Enum"
#[tokio::test]
async fn test_native_postgres_enum_insert() {
    if !is_postgres() {
        eprintln!("Skipping: not Postgres");
        return;
    }

    let db = Database::connect(get_database_url())
        .await
        .expect("connect");
    assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);

    setup_postgres_enum_table(&db).await.expect("setup table");

    // INSERT using Sea-ORM ActiveModel
    let widget = widget::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        name: Set("Test Widget".to_string()),
        color: Set(Color::Red),
    };

    let result = widget::Entity::insert(widget).exec(&db).await;
    assert!(
        result.is_ok(),
        "INSERT with native Postgres ENUM should work on Sea-ORM 1.1.19: {:?}",
        result.err()
    );
}

/// Test that we can query back enum values from a native Postgres ENUM column
#[tokio::test]
async fn test_native_postgres_enum_query() {
    if !is_postgres() {
        eprintln!("Skipping: not Postgres");
        return;
    }

    let db = Database::connect(get_database_url())
        .await
        .expect("connect");
    setup_postgres_enum_table(&db).await.expect("setup table");

    // Insert all three colors
    for (name, color) in [("R", Color::Red), ("G", Color::Green), ("B", Color::Blue)] {
        let widget = widget::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(name.to_string()),
            color: Set(color),
        };
        widget::Entity::insert(widget)
            .exec(&db)
            .await
            .expect("insert");
    }

    // Query all back
    let widgets = widget::Entity::find().all(&db).await.expect("find all");
    assert_eq!(widgets.len(), 3);

    // Query with filter — raw Sea-ORM ColumnTrait filter
    let reds = widget::Entity::find()
        .filter(widget::Column::Color.eq("Red"))
        .all(&db)
        .await;
    assert!(
        reds.is_ok(),
        "Filtering native ENUM by string should work: {:?}",
        reds.err()
    );
    assert_eq!(reds.unwrap().len(), 1);
}

/// Test that crudcrate's CAST(col AS TEXT) filtering works on native Postgres ENUM columns
#[tokio::test]
async fn test_native_postgres_enum_cast_filter() {
    if !is_postgres() {
        eprintln!("Skipping: not Postgres");
        return;
    }

    let db = Database::connect(get_database_url())
        .await
        .expect("connect");
    setup_postgres_enum_table(&db).await.expect("setup table");

    // Insert test data
    for (name, color) in [("R", Color::Red), ("G", Color::Green), ("B", Color::Blue)] {
        let widget = widget::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set(name.to_string()),
            color: Set(color),
        };
        widget::Entity::insert(widget)
            .exec(&db)
            .await
            .expect("insert");
    }

    // Test the exact SQL pattern crudcrate generates: UPPER(CAST(color AS TEXT))
    let result = db
        .query_all(sea_orm::Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT * FROM widgets WHERE UPPER(CAST(color AS TEXT)) = 'RED'".to_string(),
        ))
        .await;
    assert!(
        result.is_ok(),
        "CAST(enum AS TEXT) + UPPER should work: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().len(), 1);

    // Test the array/IN pattern: UPPER(CAST(color AS TEXT)) IN (...)
    let result = db
        .query_all(sea_orm::Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT * FROM widgets WHERE UPPER(CAST(color AS TEXT)) IN ('RED', 'BLUE')".to_string(),
        ))
        .await;
    assert!(
        result.is_ok(),
        "CAST(enum AS TEXT) IN (...) should work: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().len(), 2);
}
