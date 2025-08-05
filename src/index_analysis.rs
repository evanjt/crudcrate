/*!
# Index Analysis Module

Database-agnostic index analysis to help optimize CRUD performance.
Provides startup warnings for missing indexes on filterable, sortable, and fulltext fields.
*/

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

static INDEX_ANALYSIS_SHOWN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct IndexRecommendation {
    pub table_name: String,
    pub column_name: String,
    pub index_type: IndexType,
    pub reason: String,
    pub priority: Priority,
    pub suggested_sql: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexType {
    BTree,
    GIN,      // PostgreSQL only
    Fulltext, // MySQL only
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug)]
struct ExistingIndex {
    #[allow(dead_code)]
    table_name: String,
    column_name: String,
    #[allow(dead_code)]
    index_name: String,
    index_type: String,
}

/// Analyze database indexes and provide recommendations for CRUD resources
/// 
/// # Errors
/// 
/// Returns a `sea_orm::DbErr` if database queries fail or connection issues occur.
pub async fn analyze_indexes_for_resource<T: crate::traits::CRUDResource>(
    db: &DatabaseConnection,
) -> Result<Vec<IndexRecommendation>, sea_orm::DbErr> {
    let table_name = T::RESOURCE_NAME_PLURAL;
    let backend = db.get_database_backend();

    // Get existing indexes for this table
    let existing_indexes = get_existing_indexes(db, table_name, backend).await?;
    let indexed_columns: HashSet<String> = existing_indexes
        .iter()
        .map(|idx| idx.column_name.clone())
        .collect();

    let mut recommendations = Vec::new();

    // Check filterable columns
    for (field_name, _column) in T::filterable_columns() {
        if !indexed_columns.contains(field_name) {
            recommendations.push(IndexRecommendation {
                table_name: table_name.to_string(),
                column_name: field_name.to_string(),
                index_type: IndexType::BTree,
                reason: format!("Field '{field_name}' is filterable but not indexed"),
                priority: Priority::Medium,
                suggested_sql: generate_btree_index_sql(backend, table_name, field_name),
            });
        }
    }

    // Check sortable columns
    for (field_name, _column) in T::sortable_columns() {
        if !indexed_columns.contains(field_name) {
            recommendations.push(IndexRecommendation {
                table_name: table_name.to_string(),
                column_name: field_name.to_string(),
                index_type: IndexType::BTree,
                reason: format!("Field '{field_name}' is sortable but not indexed"),
                priority: Priority::Medium,
                suggested_sql: generate_btree_index_sql(backend, table_name, field_name),
            });
        }
    }

    // Check fulltext columns (high priority)
    let fulltext_columns = T::fulltext_searchable_columns();
    if !fulltext_columns.is_empty() {
        let has_fulltext_index =
            check_fulltext_index_exists(&existing_indexes, &fulltext_columns, backend);

        if !has_fulltext_index {
            recommendations.push(IndexRecommendation {
                table_name: table_name.to_string(),
                column_name: fulltext_columns
                    .iter()
                    .map(|(name, _)| (*name).to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                index_type: match backend {
                    DatabaseBackend::Postgres => IndexType::GIN,
                    DatabaseBackend::MySql => IndexType::Fulltext,
                    DatabaseBackend::Sqlite => IndexType::BTree,
                },
                reason: format!(
                    "Fulltext search on {} columns without proper index",
                    fulltext_columns.len()
                ),
                priority: Priority::High,
                suggested_sql: generate_fulltext_index_sql(backend, table_name, &fulltext_columns),
            });
        }
    }

    Ok(recommendations)
}

/// Display index recommendations with pretty formatting
pub fn display_index_recommendations(recommendations: &[IndexRecommendation]) {
    if recommendations.is_empty() {
        return;
    }

    // Only show analysis once per application startup
    if INDEX_ANALYSIS_SHOWN.load(Ordering::Relaxed) {
        return;
    }
    INDEX_ANALYSIS_SHOWN.store(true, Ordering::Relaxed);

    println!("\n🔍 crudcrate Index Analysis");
    println!("═══════════════════════════");

    let mut by_priority: HashMap<Priority, Vec<&IndexRecommendation>> = HashMap::new();
    for rec in recommendations {
        by_priority
            .entry(rec.priority.clone())
            .or_default()
            .push(rec);
    }

    // Display by priority
    for priority in [
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ] {
        if let Some(recs) = by_priority.get(&priority) {
            let (icon, color) = match priority {
                Priority::Critical => ("🚨", "\x1b[91m"), // Bright red
                Priority::High => ("⚠️ ", "\x1b[93m"),    // Yellow
                Priority::Medium => ("💡", "\x1b[94m"),   // Blue
                Priority::Low => ("ℹ️ ", "\x1b[92m"),     // Green
            };

            println!("\n{icon} \x1b[1m{priority:?} Priority\x1b[0m");

            for rec in recs {
                println!("{}┌─ Table: {}\x1b[0m", color, rec.table_name);
                println!("{}│  Column(s): {}\x1b[0m", color, rec.column_name);
                println!("{}│  Reason: {}\x1b[0m", color, rec.reason);
                println!("{color}│  Suggested SQL:\x1b[0m");
                println!("{}│    {}\x1b[0m", color, rec.suggested_sql);
                println!("{color}└─\x1b[0m");
            }
        }
    }

    println!("\n💡 \x1b[1mTips:\x1b[0m");
    println!("   • Run these SQL commands in your database migration");
    println!("   • Indexes improve query performance but use additional storage");
    println!("   • PostgreSQL GIN indexes are highly recommended for fulltext search");
    println!("   • Consider compound indexes for frequently combined filters");
    println!();
}

/// Get existing indexes for a table (database-agnostic)
async fn get_existing_indexes(
    db: &DatabaseConnection,
    table_name: &str,
    backend: DatabaseBackend,
) -> Result<Vec<ExistingIndex>, sea_orm::DbErr> {
    let query = match backend {
        DatabaseBackend::Postgres => {
            format!(
                r"
                SELECT 
                    t.relname as table_name,
                    a.attname as column_name,
                    i.relname as index_name,
                    am.amname as index_type
                FROM pg_class t
                JOIN pg_index ix ON t.oid = ix.indrelid
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_attribute a ON t.oid = a.attrelid AND a.attnum = ANY(ix.indkey)
                JOIN pg_am am ON i.relam = am.oid
                WHERE t.relname = '{table_name}'
                AND t.relkind = 'r'
                ORDER BY t.relname, i.relname
                "
            )
        }
        DatabaseBackend::MySql => {
            format!(
                r"
                SELECT 
                    TABLE_NAME as table_name,
                    COLUMN_NAME as column_name,
                    INDEX_NAME as index_name,
                    INDEX_TYPE as index_type
                FROM information_schema.statistics 
                WHERE TABLE_NAME = '{table_name}' 
                AND TABLE_SCHEMA = DATABASE()
                ORDER BY TABLE_NAME, INDEX_NAME
                "
            )
        }
        DatabaseBackend::Sqlite => {
            // SQLite requires a different approach - we'll use PRAGMA index_list and index_info
            format!("PRAGMA index_list({table_name})")
        }
    };

    let mut indexes = Vec::new();

    if backend == DatabaseBackend::Sqlite {
        // SQLite requires special handling
        indexes = get_sqlite_indexes(db, table_name).await?;
    } else {
        let statement = Statement::from_string(backend, query);
        let query_results = db.query_all(statement).await?;

        for row in query_results {
            let table_name: String = row.try_get("", "table_name")?;
            let column_name: String = row.try_get("", "column_name")?;
            let index_name: String = row.try_get("", "index_name")?;
            let index_type: String = row.try_get("", "index_type")?;

            indexes.push(ExistingIndex {
                table_name,
                column_name,
                index_name,
                index_type,
            });
        }
    }

    Ok(indexes)
}

/// Get `SQLite` indexes (special handling required)
async fn get_sqlite_indexes(
    db: &DatabaseConnection,
    table_name: &str,
) -> Result<Vec<ExistingIndex>, sea_orm::DbErr> {
    let mut indexes = Vec::new();

    // Get index list
    let index_list_query = Statement::from_string(
        DatabaseBackend::Sqlite,
        format!("PRAGMA index_list({table_name})"),
    );

    let index_results = db.query_all(index_list_query).await?;

    for row in index_results {
        let index_name: String = row.try_get("", "name")?;

        // Get index info for each index
        let index_info_query = Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA index_info({index_name})"),
        );

        let info_results = db.query_all(index_info_query).await?;

        for info_row in info_results {
            let column_name: String = info_row.try_get("", "name")?;

            indexes.push(ExistingIndex {
                table_name: table_name.to_string(),
                column_name,
                index_name: index_name.clone(),
                index_type: "btree".to_string(), // SQLite uses B-tree by default
            });
        }
    }

    Ok(indexes)
}

/// Check if fulltext index exists for the given columns
fn check_fulltext_index_exists(
    existing_indexes: &[ExistingIndex],
    fulltext_columns: &[(&str, impl std::fmt::Debug)],
    backend: DatabaseBackend,
) -> bool {
    match backend {
        DatabaseBackend::Postgres => {
            // Look for GIN indexes on the fulltext columns
            existing_indexes.iter().any(|idx| {
                idx.index_type.to_lowercase().contains("gin")
                    && fulltext_columns
                        .iter()
                        .any(|(col, _)| idx.column_name == *col)
            })
        }
        DatabaseBackend::MySql => {
            // Look for FULLTEXT indexes
            existing_indexes
                .iter()
                .any(|idx| idx.index_type.to_lowercase().contains("fulltext"))
        }
        DatabaseBackend::Sqlite => {
            // For SQLite, just check if the columns are indexed
            fulltext_columns
                .iter()
                .all(|(col, _)| existing_indexes.iter().any(|idx| idx.column_name == *col))
        }
    }
}

/// Generate B-tree index SQL for different databases
fn generate_btree_index_sql(
    backend: DatabaseBackend,
    table_name: &str,
    column_name: &str,
) -> String {
    let index_name = format!("idx_{table_name}_{column_name}");

    match backend {
        DatabaseBackend::Postgres | DatabaseBackend::MySql | DatabaseBackend::Sqlite => {
            format!("CREATE INDEX {index_name} ON {table_name} ({column_name});")
        }
    }
}

/// Generate fulltext index SQL for different databases
fn generate_fulltext_index_sql(
    backend: DatabaseBackend,
    table_name: &str,
    columns: &[(&str, impl std::fmt::Debug)],
) -> String {
    let column_names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();

    match backend {
        DatabaseBackend::Postgres => {
            let combined_columns = column_names.join(" || ' ' || ");
            format!(
                "CREATE INDEX idx_{table_name}_fulltext ON {table_name} USING GIN (to_tsvector('english', {combined_columns}));"
            )
        }
        DatabaseBackend::MySql => {
            let column_list = column_names.join(", ");
            format!(
                "CREATE FULLTEXT INDEX idx_{table_name}_fulltext ON {table_name} ({column_list});"
            )
        }
        DatabaseBackend::Sqlite => {
            // SQLite doesn't have native fulltext search in our setup, suggest regular indexes
            column_names
                .iter()
                .map(|col| generate_btree_index_sql(backend, table_name, col))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}
