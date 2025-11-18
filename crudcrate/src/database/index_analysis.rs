/*!
# Index Analysis Module

Database-agnostic index analysis to help optimise CRUD performance.
Provides startup warnings for missing indexes on filterable, sortable, and fulltext fields.
*/

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Quote SQL identifier to prevent injection (double quotes for Postgres/SQLite, backticks for MySQL)
fn quote_identifier(identifier: &str, backend: DatabaseBackend) -> String {
    match backend {
        DatabaseBackend::MySql => {
            // MySQL uses backticks, escape by doubling them
            format!("`{}`", identifier.replace('`', "``"))
        }
        DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
            // Postgres and SQLite use double quotes, escape by doubling them
            format!("\"{}\"", identifier.replace('"', "\"\""))
        }
    }
}

static INDEX_ANALYSIS_SHOWN: AtomicBool = AtomicBool::new(false);

// Global registry for models that should be analysed
type IndexAnalyzer = Box<
    dyn Fn(
            &DatabaseConnection,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Vec<IndexRecommendation>, sea_orm::DbErr>>
                    + Send,
            >,
        > + Send
        + Sync,
>;
static GLOBAL_ANALYZERS: std::sync::LazyLock<Arc<Mutex<Vec<IndexAnalyzer>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

#[derive(Debug, Clone)]
pub struct IndexRecommendation {
    pub table_name: String,
    pub column_name: String,
    pub index_type: IndexType,
    pub reason: String,
    pub priority: Priority,
    pub suggested_sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    column_name: String,

    index_name: String,
    index_type: String,
}

/// Analyse database indexes and provide recommendations for CRUD resources
///
/// # Errors
///
/// Returns a `sea_orm::DbErr` if database queries fail or connection issues occur.
pub async fn analyse_indexes_for_resource<T: crate::traits::CRUDResource>(
    db: &DatabaseConnection,
) -> Result<Vec<IndexRecommendation>, sea_orm::DbErr> {
    let table_name = T::TABLE_NAME;
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
                suggested_sql: generate_fulltext_index_sql(
                    backend,
                    table_name,
                    &fulltext_columns,
                    T::FULLTEXT_LANGUAGE,
                ),
            });
        }
    }

    Ok(recommendations)
}

/// Display index recommendations with compact formatting
pub fn display_index_recommendations(recommendations: &[IndexRecommendation]) {
    display_index_recommendations_internal(recommendations, false);
}

/// Display index recommendations with SQL examples
pub fn display_index_recommendations_with_examples(recommendations: &[IndexRecommendation]) {
    display_index_recommendations_internal(recommendations, true);
}

/// Internal function to display index recommendations with optional SQL examples
fn display_index_recommendations_internal(
    recommendations: &[IndexRecommendation],
    show_examples: bool,
) {
    if recommendations.is_empty() {
        return;
    }

    // Only show analysis once per application startup
    if INDEX_ANALYSIS_SHOWN.load(Ordering::Relaxed) {
        return;
    }
    INDEX_ANALYSIS_SHOWN.store(true, Ordering::Relaxed);

    println!("\ncrudcrate Index Analysis");
    println!("═══════════════════════════");

    let mut by_priority: HashMap<Priority, Vec<&IndexRecommendation>> = HashMap::new();
    let mut all_sql_commands: Vec<String> = Vec::new();

    for rec in recommendations {
        by_priority
            .entry(rec.priority.clone())
            .or_default()
            .push(rec);
        // Handle multi-line SQL commands (e.g., SQLite fulltext which generates multiple indexes)
        for line in rec.suggested_sql.lines() {
            if !line.trim().is_empty() {
                all_sql_commands.push(line.trim().to_string());
            }
        }
    }

    // Display by priority with compact single-line format
    for priority in [
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ] {
        if let Some(recs) = by_priority.get(&priority) {
            let (icon, _color) = match priority {
                Priority::Critical => ("CRITICAL", "\x1b[91m"), // Bright red
                Priority::High => ("HIGH", "\x1b[93m"),         // Yellow
                Priority::Medium => ("MEDIUM", "\x1b[94m"),     // Blue
                Priority::Low => ("LOW", "\x1b[92m"),           // Green
            };

            if !recs.is_empty() {
                println!("\n{icon} {priority:?} Priority:");
                for rec in recs {
                    // Compact single-line format: table.column - reason
                    println!("  {} - {}", rec.table_name, rec.reason);
                }
            }
        }
    }

    // Always show consolidated SQL commands for easy copy-paste
    if show_examples && !all_sql_commands.is_empty() {
        println!("\n═══════════════════════════");
        println!("Copy-paste SQL commands:");
        println!("═══════════════════════════");

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        for sql in &all_sql_commands {
            if seen.insert(sql.clone()) {
                println!("{sql}");
            }
        }

        println!("\nExecute these commands to optimize your database indexes");
    } else if !show_examples {
        println!("\nUse analyse_all_registered_models(&db, true) for SQL commands");
    }
}

/// Register a model for automatic index analysis
///
/// If the global analyzers mutex is poisoned, this logs an error and skips registration.
/// Index analysis is a diagnostic feature, so poisoning should not crash the application.
pub fn register_analyser<T: crate::traits::CRUDResource + 'static>() {
    let analyser: IndexAnalyzer = Box::new(|db: &DatabaseConnection| {
        let db = db.clone();
        Box::pin(async move { analyse_indexes_for_resource::<T>(&db).await })
    });

    match GLOBAL_ANALYZERS.lock() {
        Ok(mut guard) => guard.push(analyser),
        Err(e) => {
            eprintln!("Warning: Failed to register index analyzer due to poisoned mutex: {}", e);
            eprintln!("Index analysis will not be available for this resource");
        }
    }
}

/// Run index analysis for all registered models with optional SQL examples
///
/// # Parameters
/// - `db`: Database connection for analyzing existing indexes
/// - `show_examples`: If true, displays SQL CREATE INDEX commands; if false, shows compact summary
///
/// # Examples
/// ```rust,no_run
/// use crudcrate::analyse_all_registered_models;
/// use sea_orm::Database;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let db = Database::connect("sqlite::memory:").await?;
///
/// // Compact output (default for production)
/// let _ = analyse_all_registered_models(&db, false).await;
///
/// // Detailed output with SQL commands (useful for development)
/// let _ = analyse_all_registered_models(&db, true).await;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns a `sea_orm::DbErr` if database operations fail during index analysis.
///
/// If the global analyzers mutex is poisoned, this function will attempt to recover
/// the data and continue. Index analysis is a diagnostic feature, not critical functionality.
#[allow(clippy::await_holding_lock)]
pub async fn analyse_all_registered_models(
    db: &DatabaseConnection,
    show_examples: bool,
) -> Result<(), sea_orm::DbErr> {
    let mut all_recommendations = Vec::new();

    {
        let guard = match GLOBAL_ANALYZERS.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("Warning: Index analyzers mutex was poisoned, recovering data");
                poisoned.into_inner()
            }
        };
        for analyser in guard.iter() {
            let recommendations = analyser(db).await?;
            all_recommendations.extend(recommendations);
        }
    }

    if show_examples {
        display_index_recommendations_with_examples(&all_recommendations);
    } else {
        display_index_recommendations(&all_recommendations);
    }
    Ok(())
}

/// Force all lazy static analysers to register by triggering their initialization
/// This is a workaround for the fact that `LazyLock` only initializes when first accessed
pub async fn ensure_all_analysers_registered() {
    // This function intentionally does nothing - the mere act of calling it
    // ensures this module is loaded, which should trigger any LazyLock registrations
    // in modules that have been compiled but not yet loaded
}

/// Get existing indexes for a table (database-agnostic)
async fn get_existing_indexes(
    db: &DatabaseConnection,
    table_name: &str,
    backend: DatabaseBackend,
) -> Result<Vec<ExistingIndex>, sea_orm::DbErr> {
    let quoted_table = quote_identifier(table_name, backend);
    let query = match backend {
        DatabaseBackend::Postgres => {
            // Use quoted table name in WHERE clause
            format!(
                r"
                SELECT DISTINCT
                    t.relname as table_name,
                    COALESCE(a.attname, 'expression') as column_name,
                    i.relname as index_name,
                    am.amname as index_type
                FROM pg_class t
                JOIN pg_index ix ON t.oid = ix.indrelid
                JOIN pg_class i ON i.oid = ix.indexrelid
                LEFT JOIN pg_attribute a ON t.oid = a.attrelid AND a.attnum = ANY(ix.indkey)
                JOIN pg_am am ON i.relam = am.oid
                WHERE t.relname = {quoted_table}
                AND t.relkind = 'r'
                ORDER BY t.relname, i.relname
                "
            )
        }
        DatabaseBackend::MySql => {
            // Use quoted table name in WHERE clause
            format!(
                r"
                SELECT
                    TABLE_NAME as table_name,
                    COLUMN_NAME as column_name,
                    INDEX_NAME as index_name,
                    INDEX_TYPE as index_type
                FROM information_schema.statistics
                WHERE TABLE_NAME = {quoted_table}
                AND TABLE_SCHEMA = DATABASE()
                ORDER BY TABLE_NAME, INDEX_NAME
                "
            )
        }
        DatabaseBackend::Sqlite => {
            // SQLite PRAGMA requires quoted identifier
            format!("PRAGMA index_list({quoted_table})")
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
            let column_name: String = row.try_get("", "column_name")?;
            let index_name: String = row.try_get("", "index_name")?;
            let index_type: String = row.try_get("", "index_type")?;

            indexes.push(ExistingIndex {
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

    // Get index list with quoted table name
    let quoted_table = quote_identifier(table_name, DatabaseBackend::Sqlite);
    let index_list_query = Statement::from_string(
        DatabaseBackend::Sqlite,
        format!("PRAGMA index_list({quoted_table})"),
    );

    let index_results = db.query_all(index_list_query).await?;

    for row in index_results {
        let index_name: String = row.try_get("", "name")?;

        // Get index info for each index with quoted index name
        let quoted_index = quote_identifier(&index_name, DatabaseBackend::Sqlite);
        let index_info_query = Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA index_info({quoted_index})"),
        );

        let info_results = db.query_all(index_info_query).await?;

        for info_row in info_results {
            let column_name: String = info_row.try_get("", "name")?;

            indexes.push(ExistingIndex {
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
            // Look for GIN indexes - check both individual column indexes and expression indexes
            existing_indexes.iter().any(|idx| {
                let is_gin = idx.index_type.to_lowercase().contains("gin");
                if !is_gin {
                    return false;
                }

                // Check if it's a traditional fulltext index (matches individual columns)
                let matches_column = fulltext_columns
                    .iter()
                    .any(|(col, _)| idx.column_name == *col);

                // Check if it's a trigram or expression index (look for common patterns)
                let is_expression_index = idx.column_name == "expression"
                    || idx.index_name.contains("trigram")
                    || idx.index_name.contains("fulltext");

                matches_column || is_expression_index
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
    // Quote all identifiers to prevent injection
    let index_name = format!("idx_{}_{}", table_name, column_name);
    let quoted_index = quote_identifier(&index_name, backend);
    let quoted_table = quote_identifier(table_name, backend);
    let quoted_column = quote_identifier(column_name, backend);

    match backend {
        DatabaseBackend::Postgres | DatabaseBackend::MySql | DatabaseBackend::Sqlite => {
            format!("CREATE INDEX {quoted_index} ON {quoted_table} ({quoted_column});")
        }
    }
}

/// Generate fulltext index SQL for different databases
fn generate_fulltext_index_sql(
    backend: DatabaseBackend,
    table_name: &str,
    columns: &[(&str, impl std::fmt::Debug)],
    language: &str,
) -> String {
    let column_names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();

    match backend {
        DatabaseBackend::Postgres => {
            // Quote all identifiers to prevent injection
            let quoted_columns: Vec<String> = column_names
                .iter()
                .map(|col| quote_identifier(col, backend))
                .collect();
            let combined_columns = quoted_columns.join(" || ' ' || ");
            let index_name = format!("idx_{}_fulltext", table_name);
            let quoted_index = quote_identifier(&index_name, backend);
            let quoted_table = quote_identifier(table_name, backend);
            // Language is a string literal, but sanitize it to prevent injection
            let safe_language = language.replace('\'', "''");
            format!(
                "CREATE INDEX {quoted_index} ON {quoted_table} USING GIN (to_tsvector('{safe_language}', {combined_columns}));"
            )
        }
        DatabaseBackend::MySql => {
            // Quote all identifiers to prevent injection
            let quoted_columns: Vec<String> = column_names
                .iter()
                .map(|col| quote_identifier(col, backend))
                .collect();
            let column_list = quoted_columns.join(", ");
            let index_name = format!("idx_{}_fulltext", table_name);
            let quoted_index = quote_identifier(&index_name, backend);
            let quoted_table = quote_identifier(table_name, backend);
            format!(
                "CREATE FULLTEXT INDEX {quoted_index} ON {quoted_table} ({column_list});"
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
