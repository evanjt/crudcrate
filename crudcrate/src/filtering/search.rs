use sea_orm::{DatabaseBackend, sea_query::SimpleExpr};

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
#[must_use]
pub fn build_like_condition(key: &str, trimmed_value: &str) -> SimpleExpr {
    let escaped_value = trimmed_value.replace('\'', "''");
    let like_sql = format!("UPPER({key}) LIKE UPPER('%{escaped_value}%')");
    SimpleExpr::Custom(like_sql)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::Expr;

    /// TDD: Column names should be safely quoted, not string-interpolated
    /// This test will FAIL until we fix the SQL injection vulnerability
    #[test]
    fn test_column_names_are_safely_quoted() {
        // After fix: column names should be wrapped in proper quoting
        let result = build_like_condition("user_name", "test");
        let sql = format!("{result:?}");

        // After fix with Expr::col(), SQL should contain quoted identifiers
        // For now this will FAIL because we use format!("{key}")
        assert!(
            sql.contains("\"user_name\"") || sql.contains("`user_name`") || sql.contains("[user_name]"),
            "Column names should be properly quoted, got: {}", sql
        );
    }

    /// TDD: Malicious column names should be rejected or safely escaped
    /// This test will FAIL until we add proper validation
    #[test]
    fn test_rejects_malicious_column_names() {
        let malicious_names = vec![
            "id); DROP TABLE users; --",
            "id' OR '1'='1",
        ];

        for malicious_name in malicious_names {
            let result = build_like_condition(malicious_name, "test");
            let sql = format!("{result:?}");

            // After fix: malicious SQL should NOT appear literally in output
            // Should be quoted/escaped or rejected entirely
            assert!(
                !sql.contains("); DROP") && !sql.contains("' OR '"),
                "Malicious SQL should be escaped/quoted, not literal: {}", sql
            );
        }
    }

    /// Test that search query values are properly escaped (this one already works)
    #[test]
    fn test_search_query_value_escaping() {
        let malicious_values = vec![
            "'; DROP TABLE users; --",
            "' OR '1'='1",
        ];

        for malicious_value in malicious_values {
            let result = build_like_condition("title", malicious_value);
            let sql = format!("{result:?}");

            // Single quotes should be doubled ('') to escape them
            assert!(sql.contains("''"), "Should escape single quotes for: {}", malicious_value);
        }
    }

    /// Test that excessively long queries are truncated (this one already works)
    #[test]
    fn test_search_query_length_limit() {
        let very_long_query = "a".repeat(20_000);
        let sanitized = sanitize_search_query(&very_long_query);

        assert!(sanitized.len() <= MAX_SEARCH_QUERY_LENGTH,
            "Query should be truncated to max length");
    }
}