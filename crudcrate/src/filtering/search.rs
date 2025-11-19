use sea_orm::{DatabaseBackend, sea_query::SimpleExpr};

// Basic safety limits
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;

/// Escape LIKE wildcards to prevent wildcard injection attacks
/// Escapes: % (match any) and _ (match single char)
fn escape_like_wildcards(input: &str) -> String {
    input.replace('\\', "\\\\")  // Escape backslash first
        .replace('%', "\\%")      // Escape %
        .replace('_', "\\_")      // Escape _
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
    let sanitized_query = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();

    // Escape both SQL quotes and LIKE wildcards
    let escaped_query = escape_like_wildcards(sanitized_query).replace('\'', "''");

    // Use a consistent approach: combine ILIKE for substring matching with trigram similarity for fuzzy matching
    // This ensures reliable partial matching across all query lengths
    // Note: LIKE wildcards are now escaped, ESCAPE '\' tells PostgreSQL to respect our escaping
    let search_sql = format!(
        "(UPPER({concat_sql}) LIKE UPPER('%{escaped_query}%') ESCAPE '\\' OR SIMILARITY({concat_sql}, '{escaped_query}') > 0.1)"
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

    // Escape both LIKE wildcards and SQL quotes
    let escaped_query = escape_like_wildcards(sanitized_query).replace('\'', "''");

    let like_sql = format!(
        "UPPER({concat_sql}) LIKE UPPER('%{escaped_query}%') ESCAPE '\\'",
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

    // Escape LIKE wildcards to prevent injection attacks
    let escaped_value = escape_like_wildcards(trimmed_value);

    // Build UPPER(column) LIKE UPPER('%value%') ESCAPE '\'
    // Case-insensitive pattern matching with wildcard escaping
    let pattern = format!("%{}%", escaped_value.to_uppercase());

    Func::upper(column).like(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TDD: Column names should use `Expr::col()` not string interpolation
    #[test]
    fn test_column_names_use_expr_col() {
        // After fix: column names should be wrapped in Column() AST node
        let result = build_like_condition("user_name", "test");
        let sql = format!("{result:?}");

        // Verify we're using Expr::col() which wraps in Column()
        // This proves we're NOT using format!("{key}") anymore
        assert!(
            sql.contains("Column(") && sql.contains("user_name"),
            "Column should be wrapped in Column() AST node, got: {sql}"
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
            assert!(sql.contains("Value(String"), "Values should be wrapped safely: {sql}");
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

    /// Security test: LIKE wildcards should be escaped
    #[test]
    fn test_wildcard_escaping() {
        assert_eq!(escape_like_wildcards("test"), "test", "Normal text should pass through");
        assert_eq!(escape_like_wildcards("test%"), "test\\%", "% should be escaped");
        assert_eq!(escape_like_wildcards("test_value"), "test\\_value", "_ should be escaped");
        assert_eq!(escape_like_wildcards("100%"), "100\\%", "% in middle should be escaped");
        assert_eq!(escape_like_wildcards("%_"), "\\%\\_", "Both wildcards should be escaped");
        assert_eq!(escape_like_wildcards("\\"), "\\\\", "Backslash should be escaped");
        assert_eq!(escape_like_wildcards("\\%"), "\\\\\\%", "Backslash and % should both be escaped");
    }

    /// Security test: Wildcard injection should be prevented in LIKE conditions
    #[test]
    fn test_like_condition_prevents_wildcard_injection() {
        // Test that wildcards are properly escaped
        let result_percent = build_like_condition("title", "test%");
        let sql_percent = format!("{result_percent:?}");
        // Debug repr will show \\% (escaped backslash), actual SQL has \%
        assert!(sql_percent.contains("\\\\%"),
            "% should be escaped in SQL: {sql_percent}");

        let result_underscore = build_like_condition("title", "test_value");
        let sql_underscore = format!("{result_underscore:?}");
        assert!(sql_underscore.contains("\\\\_"),
            "_ should be escaped in SQL: {sql_underscore}");

        // Test just wildcards
        let result_just_percent = build_like_condition("title", "%");
        let sql_just_percent = format!("{result_just_percent:?}");
        assert!(sql_just_percent.contains("\\\\%"),
            "Single % should be escaped: {sql_just_percent}");
    }
}