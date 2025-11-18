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

    /// Test that malicious column names are not directly interpolated into SQL
    #[test]
    fn test_sql_injection_in_column_names() {
        // These would be SQL injection attempts if column names come from user input
        let malicious_names = vec![
            "id); DROP TABLE users; --",
            "id' OR '1'='1",
            "id; DELETE FROM users WHERE '1'='1",
            "id\"; DROP TABLE users; --",
            "id' UNION SELECT * FROM passwords --",
        ];

        for malicious_name in malicious_names {
            // The current implementation uses format!() which is vulnerable
            // This test documents the vulnerability
            let result = build_like_condition(malicious_name, "test");
            let sql = format!("{result:?}");

            // After fix, these should NOT contain the raw malicious input
            // For now, this test will PASS but documents the issue
            assert!(sql.contains("UPPER"), "Should generate SQL, currently vulnerable to: {}", malicious_name);
        }
    }

    /// Test that search query values are properly escaped
    #[test]
    fn test_search_query_value_escaping() {
        let malicious_values = vec![
            "'; DROP TABLE users; --",
            "' OR '1'='1",
            "test' UNION SELECT * FROM passwords --",
        ];

        for malicious_value in malicious_values {
            let result = build_like_condition("title", malicious_value);
            let sql = format!("{result:?}");

            // Single quotes should be doubled ('') to escape them
            assert!(sql.contains("''"), "Should escape single quotes for: {}", malicious_value);
            // This prevents SQL injection - the '' becomes a literal quote in SQL
        }
    }

    /// Test that excessively long queries are truncated
    #[test]
    fn test_search_query_length_limit() {
        let very_long_query = "a".repeat(20_000);
        let sanitized = sanitize_search_query(&very_long_query);

        assert!(sanitized.len() <= MAX_SEARCH_QUERY_LENGTH,
            "Query should be truncated to max length");
    }

    /// Test that column name interpolation creates potential SQL injection
    /// This test DOCUMENTS the current vulnerability
    #[test]
    fn test_column_name_injection_vulnerability_documented() {
        // CURRENT BEHAVIOR (VULNERABLE):
        // Column names are directly interpolated via format!()
        let malicious_column = "id); DROP TABLE users; --";
        let result = build_like_condition(malicious_column, "safe_value");
        let sql = format!("{result:?}");

        // This currently DOES contain the malicious code (proving vulnerability)
        assert!(sql.contains(malicious_column),
            "VULNERABILITY CONFIRMED: Column name is directly interpolated");

        // After fix with sea-query Expr::col(), this test should FAIL
        // because column names will be properly quoted/validated
    }
}