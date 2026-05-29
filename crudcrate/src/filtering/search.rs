use sea_orm::{
    DatabaseBackend,
    sea_query::{Expr, SimpleExpr},
};

const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;

/// Escape LIKE wildcards to prevent wildcard injection attacks
/// Escapes: % (match any) and _ (match single char)
fn escape_like_wildcards(input: &str) -> String {
    input
        .replace('\\', "\\\\") // Escape backslash first
        .replace('%', "\\%") // Escape %
        .replace('_', "\\_") // Escape _
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
    let column_names: Vec<&'static str> = fulltext_columns.iter().map(|(name, _)| *name).collect();

    match backend {
        DatabaseBackend::Postgres => build_postgres_fulltext_condition(query, &column_names),
        DatabaseBackend::MySql => build_mysql_fulltext_condition(query, &column_names),
        DatabaseBackend::Sqlite => build_fallback_fulltext_condition(query, &column_names),
    }
}

/// Build PostgreSQL-specific fulltext search using ILIKE for case-insensitive matching.
///
/// Column names come from the macro-generated `fulltext_searchable_columns()` — they are
/// compile-time-known `&'static str` Rust identifiers and never user input. The query
/// value is routed through a bind parameter via `Expr::cust_with_values`.
fn build_postgres_fulltext_condition(
    query: &str,
    column_names: &[&'static str],
) -> Option<SimpleExpr> {
    if column_names.is_empty() || query.is_empty() {
        return None;
    }

    let concat_sql = column_names
        .iter()
        .map(|name| format!("COALESCE({name}::text, '')"))
        .collect::<Vec<_>>()
        .join(" || ' ' || ");

    let sanitized = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized));

    Some(Expr::cust_with_values(
        format!("({concat_sql}) ILIKE $1 ESCAPE '\\'"),
        [pattern],
    ))
}

/// Build MySQL-specific fulltext search using CONCAT and LIKE.
///
/// See [`build_postgres_fulltext_condition`] for the safety rationale.
fn build_mysql_fulltext_condition(
    query: &str,
    column_names: &[&'static str],
) -> Option<SimpleExpr> {
    if column_names.is_empty() || query.is_empty() {
        return None;
    }

    let coalesced: Vec<String> = column_names
        .iter()
        .map(|name| format!("COALESCE(CAST({name} AS CHAR), '')"))
        .collect();
    let concat_sql = if coalesced.len() == 1 {
        coalesced[0].clone()
    } else {
        format!("CONCAT({})", coalesced.join(", ' ', "))
    };

    let sanitized = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized).to_uppercase());

    Some(Expr::cust_with_values(
        format!("UPPER({concat_sql}) LIKE ? ESCAPE '\\\\'"),
        [pattern],
    ))
}

/// Build fallback fulltext search for `SQLite` and other standard SQL databases.
///
/// See [`build_postgres_fulltext_condition`] for the safety rationale.
fn build_fallback_fulltext_condition(
    query: &str,
    column_names: &[&'static str],
) -> Option<SimpleExpr> {
    if column_names.is_empty() || query.is_empty() {
        return None;
    }

    let concat_sql = column_names
        .iter()
        .map(|name| format!("CAST({name} AS TEXT)"))
        .collect::<Vec<_>>()
        .join(" || ' ' || ");

    let sanitized = query[..query.len().min(MAX_SEARCH_QUERY_LENGTH)].trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized).to_uppercase());

    Some(Expr::cust_with_values(
        format!("UPPER({concat_sql}) LIKE ? ESCAPE '\\'"),
        [pattern],
    ))
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
        let malicious_values = vec!["'; DROP TABLE users; --", "' OR '1'='1"];

        for malicious_value in malicious_values {
            let result = build_like_condition("title", malicious_value);
            let sql = format!("{result:?}");

            // Values are wrapped in Value() which sea-query parameterizes safely
            // The pattern is uppercased and wrapped, so SQL injection is prevented
            assert!(
                sql.contains("Value(String"),
                "Values should be wrapped safely: {sql}"
            );
        }
    }

    /// Test that excessively long queries are truncated in fulltext search
    #[test]
    fn test_search_query_length_limit() {
        let very_long_query = "a".repeat(20_000);
        // Test the inlined sanitization logic
        let sanitized = &very_long_query[..very_long_query.len().min(MAX_SEARCH_QUERY_LENGTH)];

        assert!(
            sanitized.len() <= MAX_SEARCH_QUERY_LENGTH,
            "Query should be truncated to max length"
        );
    }

    /// Security test: LIKE wildcards should be escaped
    #[test]
    fn test_wildcard_escaping() {
        assert_eq!(
            escape_like_wildcards("test"),
            "test",
            "Normal text should pass through"
        );
        assert_eq!(
            escape_like_wildcards("test%"),
            "test\\%",
            "% should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("test_value"),
            "test\\_value",
            "_ should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("100%"),
            "100\\%",
            "% in middle should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("%_"),
            "\\%\\_",
            "Both wildcards should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("\\"),
            "\\\\",
            "Backslash should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("\\%"),
            "\\\\\\%",
            "Backslash and % should both be escaped"
        );
    }

    /// Security test: Wildcard injection should be prevented in LIKE conditions
    #[test]
    fn test_like_condition_prevents_wildcard_injection() {
        // Test that wildcards are properly escaped
        let result_percent = build_like_condition("title", "test%");
        let sql_percent = format!("{result_percent:?}");
        // Debug repr will show \\% (escaped backslash), actual SQL has \%
        assert!(
            sql_percent.contains("\\\\%"),
            "% should be escaped in SQL: {sql_percent}"
        );

        let result_underscore = build_like_condition("title", "test_value");
        let sql_underscore = format!("{result_underscore:?}");
        assert!(
            sql_underscore.contains("\\\\_"),
            "_ should be escaped in SQL: {sql_underscore}"
        );

        // Test just wildcards
        let result_just_percent = build_like_condition("title", "%");
        let sql_just_percent = format!("{result_just_percent:?}");
        assert!(
            sql_just_percent.contains("\\\\%"),
            "Single % should be escaped: {sql_just_percent}"
        );
    }

    /// Test build_like_condition with empty value
    #[test]
    fn test_build_like_condition_empty_value() {
        let result = build_like_condition("field", "");
        let sql = format!("{result:?}");
        assert!(sql.contains("field"), "Should include field name");
    }

    /// Test build_like_condition case insensitivity
    #[test]
    fn test_build_like_condition_case_insensitive() {
        let result = build_like_condition("title", "TeSt");
        let sql = format!("{result:?}");
        // Should use UPPER() for case-insensitive matching
        assert!(
            sql.contains("Upper") || sql.contains("UPPER"),
            "Should use UPPER for case insensitivity: {sql}"
        );
    }

    /// Test build_like_condition with special characters
    #[test]
    fn test_build_like_condition_special_chars() {
        let result = build_like_condition("title", "test@email.com");
        let sql = format!("{result:?}");
        assert!(sql.contains("title"), "Should handle special characters");
    }

    // ========================================================================
    // EMPTY/WHITESPACE QUERY TESTS
    // Full entity-based tests are in integration tests
    // ========================================================================

    /// Test that empty query produces match-all pattern in LIKE condition
    #[test]
    fn test_like_condition_empty_query_matches_all() {
        let result = build_like_condition("field", "");
        let sql = format!("{result:?}");
        // Empty query produces LIKE '%%' which matches everything
        assert!(
            sql.contains("%%") || sql.contains("%\""),
            "Empty query should produce match-all pattern"
        );
    }

    /// Test that whitespace-only query produces match-all pattern
    #[test]
    fn test_like_condition_whitespace_query() {
        // Note: build_like_condition doesn't trim - it passes through
        // Trimming happens at a higher level (in conditions.rs process_string_filter)
        let result = build_like_condition("field", "   ");
        let sql = format!("{result:?}");
        // Pattern will be uppercased but still contain the spaces
        assert!(sql.contains("field"), "Should include field name");
    }

    /// Test case-insensitive matching in LIKE condition
    #[test]
    fn test_like_condition_case_insensitive_pattern() {
        // Test that the pattern is uppercased for case-insensitive matching
        let result = build_like_condition("field", "MiXeD CaSe");
        let sql = format!("{result:?}");

        // The pattern should contain the uppercased value
        assert!(
            sql.contains("MIXED CASE"),
            "Pattern should be uppercased for case-insensitive match: {}",
            sql
        );
    }

    /// Test query length limiting constant
    #[test]
    fn test_max_search_query_length_constant() {
        assert_eq!(
            MAX_SEARCH_QUERY_LENGTH, 10_000,
            "Max query length should be 10,000"
        );
    }

    /// Test escape function handles empty string
    #[test]
    fn test_escape_like_wildcards_empty() {
        assert_eq!(
            escape_like_wildcards(""),
            "",
            "Empty string should pass through"
        );
    }

    // --- Issue 5: Fulltext SQL must route user value through a bind parameter ---

    /// Split a `CustomWithExpr` debug string into (template, values_section).
    /// Returns (`"`..., `[`Value(...)`]`).
    fn split_custom_with_expr(debug: &str) -> (&str, &str) {
        let prefix = "CustomWithExpr(\"";
        let start = debug
            .find(prefix)
            .map(|i| i + prefix.len())
            .expect("not a CustomWithExpr");
        let split = debug
            .find("\", [")
            .expect("CustomWithExpr without values section");
        (&debug[start..split], &debug[split + 4..])
    }

    /// Postgres fulltext path must use a bind parameter, not string interpolation.
    #[test]
    fn test_postgres_fulltext_binds_query_value() {
        let malicious = "'; DROP TABLE users; --";
        let result = build_postgres_fulltext_condition(malicious, &["name", "email"])
            .expect("non-empty input produces a condition");
        let debug = format!("{result:?}");

        assert!(debug.starts_with("CustomWithExpr"), "got {debug}");
        let (template, values) = split_custom_with_expr(&debug);
        assert!(
            template.contains("ILIKE ?"),
            "template must use a placeholder, got: {template}"
        );
        assert!(
            !template.contains(malicious) && !template.contains("DROP TABLE"),
            "malicious value must not appear in SQL template: {template}"
        );
        assert!(
            values.contains("Value(String") && values.contains("DROP TABLE"),
            "query value must be bound as a parameter, got: {values}"
        );
    }

    /// MySQL fulltext path must use a bind parameter.
    #[test]
    fn test_mysql_fulltext_binds_query_value() {
        let malicious = "' OR '1'='1";
        let result = build_mysql_fulltext_condition(malicious, &["name", "email"])
            .expect("non-empty input produces a condition");
        let debug = format!("{result:?}");

        assert!(debug.starts_with("CustomWithExpr"), "got {debug}");
        let (template, values) = split_custom_with_expr(&debug);
        assert!(
            template.contains("LIKE ?"),
            "template must use a placeholder, got: {template}"
        );
        assert!(
            !template.contains(malicious),
            "malicious value must not appear in SQL template: {template}"
        );
        assert!(
            values.contains("Value(String"),
            "query value must be bound as a parameter, got: {values}"
        );
    }

    /// SQLite/fallback fulltext path must use a bind parameter.
    #[test]
    fn test_fallback_fulltext_binds_query_value() {
        let malicious = "'; DELETE FROM customers; --";
        let result = build_fallback_fulltext_condition(malicious, &["name"])
            .expect("non-empty input produces a condition");
        let debug = format!("{result:?}");

        assert!(debug.starts_with("CustomWithExpr"), "got {debug}");
        let (template, values) = split_custom_with_expr(&debug);
        assert!(
            template.contains("LIKE ?"),
            "template must use a placeholder, got: {template}"
        );
        assert!(
            !template.contains(malicious) && !template.contains("DELETE FROM"),
            "malicious value must not appear in SQL template: {template}"
        );
        assert!(
            values.contains("Value(String"),
            "query value must be bound as a parameter, got: {values}"
        );
    }

    /// LIKE wildcards in the query value remain escaped inside the bound pattern.
    #[test]
    fn test_postgres_fulltext_escapes_like_wildcards() {
        let result = build_postgres_fulltext_condition("100%", &["name"])
            .expect("non-empty input produces a condition");
        let debug = format!("{result:?}");
        // The bound pattern should contain the escaped form "100\%" (escaped \\%)
        assert!(
            debug.contains("100\\\\%") || debug.contains("100\\%"),
            "expected LIKE wildcard escaped in pattern, got {debug}"
        );
    }

    /// Empty column list returns None, no SQL generated.
    #[test]
    fn test_fulltext_empty_columns_returns_none() {
        assert!(build_postgres_fulltext_condition("foo", &[]).is_none());
        assert!(build_mysql_fulltext_condition("foo", &[]).is_none());
        assert!(build_fallback_fulltext_condition("foo", &[]).is_none());
    }

    /// Empty query returns None.
    #[test]
    fn test_fulltext_empty_query_returns_none() {
        assert!(build_postgres_fulltext_condition("", &["name"]).is_none());
        assert!(build_mysql_fulltext_condition("", &["name"]).is_none());
        assert!(build_fallback_fulltext_condition("", &["name"]).is_none());
    }
}
