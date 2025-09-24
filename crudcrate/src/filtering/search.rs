use sea_orm::{
    DatabaseBackend,
    sea_query::{Alias, Expr, SimpleExpr},
};
use std::sync::atomic::AtomicBool;

static FULLTEXT_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);

// Basic safety limits
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;

/// Simple sanitization for search queries
fn sanitize_search_query(query: &str) -> String {
    if query.len() > MAX_SEARCH_QUERY_LENGTH {
        query[..MAX_SEARCH_QUERY_LENGTH].trim().to_string()
    } else {
        query.trim().to_string()
    }
}

/// Build fulltext search condition with database-specific optimizations
pub fn build_fulltext_condition<T: crate::traits::CRUDResource>(
    query: &str,
    backend: DatabaseBackend,
) -> Option<SimpleExpr> {
    let fulltext_columns = T::fulltext_searchable_columns();
    if fulltext_columns.is_empty() {
        return None;
    }

    match backend {
        DatabaseBackend::Postgres => build_postgres_fulltext_condition(query, &fulltext_columns),
        _ => build_fallback_fulltext_condition(query, &fulltext_columns),
    }
}

/// Build PostgreSQL-specific fulltext search using trigrams with relevance scoring
fn build_postgres_fulltext_condition(
    query: &str,
    columns: &[(&'static str, impl sea_orm::ColumnTrait)],
) -> Option<SimpleExpr> {
    if columns.is_empty() || query.is_empty() {
        return None;
    }

    let mut concat_parts = Vec::new();

    for (name, _column) in columns {
        // COALESCE(column_name::text, '')
        concat_parts.push(format!("COALESCE({name}::text, '')"));
    }

    let concat_sql = concat_parts.join(" || ' ' || ");
    let sanitized_query = sanitize_search_query(query);
    let escaped_query = sanitized_query.replace('\'', "''");

    // Use a consistent approach: combine ILIKE for substring matching with trigram similarity for fuzzy matching
    // This ensures reliable partial matching across all query lengths
    let search_sql = format!(
        "(UPPER({concat_sql}) LIKE UPPER('%{escaped_query}%') OR SIMILARITY({concat_sql}, '{escaped_query}') > 0.1)"
    );

    // Use custom SQL expression
    Some(SimpleExpr::Custom(search_sql))
}

/// Build fallback fulltext search using LIKE concatenation for other databases
fn build_fallback_fulltext_condition(
    query: &str,
    columns: &[(&'static str, impl sea_orm::ColumnTrait)],
) -> Option<SimpleExpr> {
    if columns.is_empty() {
        return None;
    }

    // For SQLite and MySQL, use concatenation with LIKE
    let mut concat_parts = Vec::new();

    for (name, _column) in columns {
        concat_parts.push(format!("CAST({name} AS TEXT)"));
    }

    let concat_sql = concat_parts.join(" || ' ' || ");
    // Additional security: validate and sanitize query
    let sanitized_query = sanitize_search_query(query);
    let like_sql = format!(
        "UPPER({concat_sql}) LIKE UPPER('%{}%')",
        sanitized_query.replace('\'', "''")
    );

    // Use custom SQL expression
    Some(SimpleExpr::Custom(like_sql))
}

/// Build condition for string field with LIKE queries (case-insensitive)
pub fn build_like_condition(key: &str, trimmed_value: &str) -> SimpleExpr {
    let escaped_value = trimmed_value.replace('\'', "''");
    let like_sql = format!("UPPER({key}) LIKE UPPER('%{escaped_value}%')");
    SimpleExpr::Custom(like_sql)
}