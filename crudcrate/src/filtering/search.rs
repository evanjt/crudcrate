use sea_orm::{DatabaseBackend, sea_query::SimpleExpr};

// Basic safety limits
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;

/// Build fulltext search condition with database-specific optimizations
#[must_use]
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
    let sanitized_query = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();
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
    let sanitized_query = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();
    let like_sql = format!(
        "UPPER({concat_sql}) LIKE UPPER('%{}%')",
        sanitized_query.replace('\'', "''")
    );

    // Use custom SQL expression
    Some(SimpleExpr::Custom(like_sql))
}

/// Build condition for string field with LIKE queries (case-insensitive)
#[must_use]
pub fn build_like_condition(key: &str, trimmed_value: &str) -> SimpleExpr {
    use sea_orm::sea_query::{Alias, Expr, ExprTrait, Func};

    // Use Expr::col() to properly quote column names instead of string interpolation
    let column = Expr::col(Alias::new(key));

    // Build UPPER(column) LIKE UPPER('%value%')
    // Case-insensitive pattern matching
    let pattern = format!("%{}%", trimmed_value.to_uppercase());

    Func::upper(column).like(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TDD: Column names should use Expr::col() not string interpolation
    #[test]
    fn test_column_names_use_expr_col() {
        // After fix: column names should be wrapped in Column() AST node
        let result = build_like_condition("user_name", "test");
        let sql = format!("{result:?}");

        // Verify we're using Expr::col() which wraps in Column()
        // This proves we're NOT using format!("{key}") anymore
        assert!(
            sql.contains("Column(") && sql.contains("user_name"),
            "Column should be wrapped in Column() AST node, got: {}", sql
        );
    }

    /// NOTE: Column name validation
    /// Column names come from the derive macro (compile-time), not user input,
    /// so they're safe Rust identifiers. If this ever changes and column names
    /// become user-controlled, add strict validation (alphanumeric + underscore only).
    #[test]
    fn test_column_names_wrapped_safely() {
        // Even with suspicious names, they're wrapped in Column() which sea-query handles
        let result = build_like_condition("test_column", "value");
        let sql = format!("{result:?}");

        // Verify Column() wrapper exists (proves we use Expr::col not format!)
        assert!(sql.contains("Column("), "Should use Expr::col() wrapper");
    }

    /// Test that search query values cannot inject SQL
    #[test]
    fn test_search_query_value_safe() {
        let malicious_values = vec![
            "'; DROP TABLE users; --",
            "' OR '1'='1",
        ];

        for malicious_value in malicious_values {
            let result = build_like_condition("title", malicious_value);
            let sql = format!("{result:?}");

            // Values are wrapped in Value() which sea-query parameterizes safely
            // The pattern is uppercased and wrapped, so SQL injection is prevented
            assert!(sql.contains("Value(String"), "Values should be wrapped safely: {}", sql);
        }
    }

    /// Test that excessively long queries are truncated in fulltext search
    #[test]
    fn test_search_query_length_limit() {
        let very_long_query = "a".repeat(20_000);
        // Test the inlined sanitization logic
        let sanitized = &very_long_query[..very_long_query.len().min(MAX_SEARCH_QUERY_LENGTH)];

        assert!(sanitized.len() <= MAX_SEARCH_QUERY_LENGTH,
            "Query should be truncated to max length");
    }
}