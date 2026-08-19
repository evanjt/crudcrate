use sea_orm::{DatabaseBackend, sea_query::Expr};

const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;

/// Escape LIKE wildcards to prevent wildcard injection attacks.
/// Uses `!` as the escape character (declared via `ESCAPE '!'` in the SQL).
/// Avoids backslash which conflicts with Postgres string quoting.
///
/// Shared with [`crate::filtering::conditions`] so the LIKE-fallback and joined
/// `_like` paths use the same escape convention and always pair it with an
/// explicit `ESCAPE '!'` clause.
pub(crate) fn escape_like_wildcards(input: &str) -> String {
    input
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_")
}

/// Truncate `s` to at most `max_bytes`, snapping down to the nearest UTF-8 char
/// boundary so we never slice through a multi-byte codepoint.
///
/// A raw `&s[..max_bytes]` panics when `max_bytes` lands inside a multi-byte
/// character (e.g. an attacker-supplied query of `9_999` ASCII bytes followed by
/// `é`). `str::floor_char_boundary` would do this but is still unstable, so we
/// walk back to a boundary manually.
fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Build fulltext search condition with database-specific optimizations
#[must_use]
pub fn build_fulltext_condition<T: crate::traits::CRUDResource>(
    query: &str,
    backend: DatabaseBackend,
) -> Option<Expr> {
    let fulltext_columns = T::fulltext_searchable_columns();
    if fulltext_columns.is_empty() {
        return None;
    }
    let column_names: Vec<&'static str> = fulltext_columns.iter().map(|(name, _)| *name).collect();

    match backend {
        DatabaseBackend::Postgres => build_postgres_fulltext_condition(query, &column_names),
        DatabaseBackend::MySql => build_mysql_fulltext_condition(query, &column_names),
        // SQLite, and any backend added to the non-exhaustive `DatabaseBackend`, use the
        // portable LIKE-based fallback.
        _ => build_fallback_fulltext_condition(query, &column_names),
    }
}

/// Build PostgreSQL-specific fulltext search using ILIKE for case-insensitive matching.
///
/// Column names come from the macro-generated `fulltext_searchable_columns()`; they are
/// compile-time-known `&'static str` Rust identifiers and never user input. The query
/// value is routed through a bind parameter via `Expr::cust_with_values`.
fn build_postgres_fulltext_condition(query: &str, column_names: &[&'static str]) -> Option<Expr> {
    if column_names.is_empty() || query.is_empty() {
        return None;
    }

    let concat_sql = column_names
        .iter()
        .map(|name| format!("COALESCE({name}::text, '')"))
        .collect::<Vec<_>>()
        .join(" || ' ' || ");

    let sanitized = truncate_to_char_boundary(query, MAX_SEARCH_QUERY_LENGTH).trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized));

    Some(Expr::cust_with_values(
        format!("({concat_sql}) ILIKE $1 ESCAPE '!'"),
        [pattern],
    ))
}

/// Build MySQL-specific fulltext search using CONCAT and LIKE.
///
/// See [`build_postgres_fulltext_condition`] for the safety rationale.
fn build_mysql_fulltext_condition(query: &str, column_names: &[&'static str]) -> Option<Expr> {
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

    let sanitized = truncate_to_char_boundary(query, MAX_SEARCH_QUERY_LENGTH).trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized).to_uppercase());

    Some(Expr::cust_with_values(
        format!("UPPER({concat_sql}) LIKE ? ESCAPE '!'"),
        [pattern],
    ))
}

/// Build fallback fulltext search for `SQLite` and other standard SQL databases.
///
/// See [`build_postgres_fulltext_condition`] for the safety rationale.
fn build_fallback_fulltext_condition(query: &str, column_names: &[&'static str]) -> Option<Expr> {
    if column_names.is_empty() || query.is_empty() {
        return None;
    }

    let concat_sql = column_names
        .iter()
        .map(|name| format!("CAST({name} AS TEXT)"))
        .collect::<Vec<_>>()
        .join(" || ' ' || ");

    let sanitized = truncate_to_char_boundary(query, MAX_SEARCH_QUERY_LENGTH).trim();
    let pattern = format!("%{}%", escape_like_wildcards(sanitized).to_uppercase());

    Some(Expr::cust_with_values(
        format!("UPPER({concat_sql}) LIKE ? ESCAPE '!'"),
        [pattern],
    ))
}

/// Build condition for string field with LIKE queries (case-insensitive).
///
/// Column names come from the derive macro (compile-time Rust identifiers), not user input.
/// The search value is routed through a bind parameter via `Expr::cust_with_values`.
#[must_use]
pub fn build_like_condition(key: &str, trimmed_value: &str, backend: DatabaseBackend) -> Expr {
    let escaped_value = escape_like_wildcards(trimmed_value);
    let pattern = format!("%{}%", escaped_value.to_uppercase());

    let placeholder = placeholder_for(backend);
    Expr::cust_with_values(
        format!("UPPER({key}) LIKE {placeholder} ESCAPE '!'"),
        [pattern],
    )
}

fn placeholder_for(backend: DatabaseBackend) -> &'static str {
    match backend {
        DatabaseBackend::Postgres => "$1",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Column name appears in the SQL template; value is a bind parameter.
    #[test]
    fn test_column_name_in_template_value_bound() {
        let result = build_like_condition("user_name", "test", DatabaseBackend::Sqlite);
        let debug = format!("{result:?}");

        assert!(debug.starts_with("CustomWithExpr"), "got {debug}");
        let (template, values) = split_custom_with_expr(&debug);
        assert!(
            template.contains("UPPER(user_name)"),
            "column should appear in SQL template, got: {template}"
        );
        assert!(
            template.contains("LIKE ? ESCAPE '!'"),
            "template should use bind param with ESCAPE, got: {template}"
        );
        assert!(
            values.contains("Value(String"),
            "search value should be a bind parameter, got: {values}"
        );
    }

    /// Test that search query values cannot inject SQL
    #[test]
    fn test_search_query_value_safe() {
        let malicious_values = vec!["'; DROP TABLE users; --", "' OR '1'='1"];

        for malicious_value in malicious_values {
            let result = build_like_condition("title", malicious_value, DatabaseBackend::Sqlite);
            let debug = format!("{result:?}");

            assert!(debug.starts_with("CustomWithExpr"), "got {debug}");
            let (template, values) = split_custom_with_expr(&debug);
            assert!(
                !template.contains(malicious_value),
                "malicious value must not appear in SQL template: {template}"
            );
            assert!(
                values.contains("Value(String"),
                "value should be a bind parameter: {values}"
            );
        }
    }

    /// `truncate_to_char_boundary` caps the byte length at the limit and always
    /// returns a string that ends on a UTF-8 char boundary, including when the
    /// limit lands inside a multi-byte codepoint.
    #[test]
    fn test_search_query_length_limit() {
        let very_long_query = "a".repeat(20_000);
        let sanitized = truncate_to_char_boundary(&very_long_query, MAX_SEARCH_QUERY_LENGTH);
        assert_eq!(
            sanitized.len(),
            MAX_SEARCH_QUERY_LENGTH,
            "ASCII query should truncate exactly to the byte limit"
        );

        // 9_999 ASCII bytes then `é` (2 bytes) puts the cap inside the `é`, so the
        // boundary must snap back to 9_999 rather than slice the codepoint in half.
        let multibyte = format!("{}é", "a".repeat(MAX_SEARCH_QUERY_LENGTH - 1));
        let sanitized = truncate_to_char_boundary(&multibyte, MAX_SEARCH_QUERY_LENGTH);
        assert!(
            sanitized.len() <= MAX_SEARCH_QUERY_LENGTH,
            "multi-byte query must stay within the byte limit, got {}",
            sanitized.len()
        );
        assert!(
            multibyte.is_char_boundary(sanitized.len()),
            "truncation must end on a char boundary, got len {}",
            sanitized.len()
        );
        assert_eq!(
            sanitized.len(),
            MAX_SEARCH_QUERY_LENGTH - 1,
            "boundary should snap back past the start of the `é`"
        );

        // A string already within the limit is returned unchanged.
        assert_eq!(
            truncate_to_char_boundary("short", MAX_SEARCH_QUERY_LENGTH),
            "short"
        );
    }

    /// Security test: LIKE wildcards should be escaped with `!` prefix
    #[test]
    fn test_wildcard_escaping() {
        assert_eq!(
            escape_like_wildcards("test"),
            "test",
            "Normal text should pass through"
        );
        assert_eq!(
            escape_like_wildcards("test%"),
            "test!%",
            "% should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("test_value"),
            "test!_value",
            "_ should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("100%"),
            "100!%",
            "% in middle should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("%_"),
            "!%!_",
            "Both wildcards should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("!"),
            "!!",
            "Escape character itself should be escaped"
        );
        assert_eq!(
            escape_like_wildcards("!%"),
            "!!!%",
            "Escape char and % should both be escaped"
        );
    }

    /// Security test: Wildcard injection should be prevented in LIKE conditions
    #[test]
    fn test_like_condition_prevents_wildcard_injection() {
        let result_percent = build_like_condition("title", "test%", DatabaseBackend::Sqlite);
        let debug_percent = format!("{result_percent:?}");
        let (_, values_percent) = split_custom_with_expr(&debug_percent);
        assert!(
            values_percent.contains("TEST!%"),
            "% should be escaped with ! in bound value: {values_percent}"
        );

        let result_underscore =
            build_like_condition("title", "test_value", DatabaseBackend::Sqlite);
        let debug_underscore = format!("{result_underscore:?}");
        let (_, values_underscore) = split_custom_with_expr(&debug_underscore);
        assert!(
            values_underscore.contains("TEST!_VALUE"),
            "_ should be escaped with ! in bound value: {values_underscore}"
        );

        let result_just_percent = build_like_condition("title", "%", DatabaseBackend::Sqlite);
        let debug_just_percent = format!("{result_just_percent:?}");
        let (_, values_just_percent) = split_custom_with_expr(&debug_just_percent);
        assert!(
            values_just_percent.contains("!%"),
            "Single % should be escaped: {values_just_percent}"
        );
    }

    /// Test build_like_condition with empty value
    #[test]
    fn test_build_like_condition_empty_value() {
        let result = build_like_condition("field", "", DatabaseBackend::Sqlite);
        let sql = format!("{result:?}");
        assert!(sql.contains("field"), "Should include field name");
    }

    /// Test build_like_condition case insensitivity
    #[test]
    fn test_build_like_condition_case_insensitive() {
        let result = build_like_condition("title", "TeSt", DatabaseBackend::Sqlite);
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
        let result = build_like_condition("title", "test@email.com", DatabaseBackend::Sqlite);
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
        let result = build_like_condition("field", "", DatabaseBackend::Sqlite);
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
        let result = build_like_condition("field", "   ", DatabaseBackend::Sqlite);
        let sql = format!("{result:?}");
        // Pattern will be uppercased but still contain the spaces
        assert!(sql.contains("field"), "Should include field name");
    }

    /// Test case-insensitive matching in LIKE condition
    #[test]
    fn test_like_condition_case_insensitive_pattern() {
        // Test that the pattern is uppercased for case-insensitive matching
        let result = build_like_condition("field", "MiXeD CaSe", DatabaseBackend::Sqlite);
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
            template.contains("ILIKE $1"),
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
        assert!(
            debug.contains("100!%"),
            "expected LIKE wildcard escaped with ! in pattern, got {debug}"
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

    /// A2 regression: a query longer than the byte cap whose boundary lands inside
    /// a multi-byte codepoint must not panic the truncation slice. 9_999 ASCII bytes
    /// followed by `é` (2 bytes) puts byte index 10_000 inside the `é`.
    #[test]
    fn test_fulltext_multibyte_truncation_does_not_panic() {
        let query = format!("{}é", "a".repeat(MAX_SEARCH_QUERY_LENGTH - 1));
        assert!(query.len() > MAX_SEARCH_QUERY_LENGTH);
        // None of these should panic on the byte-index slice.
        assert!(build_postgres_fulltext_condition(&query, &["name"]).is_some());
        assert!(build_mysql_fulltext_condition(&query, &["name"]).is_some());
        assert!(build_fallback_fulltext_condition(&query, &["name"]).is_some());
    }

    /// `build_like_condition` on Postgres must use the `$1` placeholder, not `?`.
    /// All other unit tests use SQLite; this covers the Postgres `placeholder_for` arm.
    #[test]
    fn test_build_like_condition_postgres_placeholder() {
        let result = build_like_condition("title", "x", DatabaseBackend::Postgres);
        let debug = format!("{result:?}");
        let (template, _values) = split_custom_with_expr(&debug);
        assert!(
            template.contains("LIKE $1 ESCAPE '!'"),
            "Postgres must use $1 placeholder, got: {template}"
        );
    }

    /// MySQL single-column fulltext uses a bare `COALESCE(...)` (no `CONCAT`), unlike
    /// the multi-column path. Exercises the `coalesced.len() == 1` branch.
    #[test]
    fn test_mysql_fulltext_single_column_no_concat() {
        let result = build_mysql_fulltext_condition("hello", &["name"])
            .expect("non-empty input produces a condition");
        let debug = format!("{result:?}");
        let (template, _values) = split_custom_with_expr(&debug);
        assert!(
            !template.contains("CONCAT"),
            "single-column MySQL fulltext should not wrap in CONCAT: {template}"
        );
        assert!(
            template.contains("COALESCE"),
            "expected COALESCE in template: {template}"
        );
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    /// Reverse the `!`-escaping that `escape_like_wildcards` applies, per SQL
    /// `LIKE ... ESCAPE '!'` semantics: a `!` consumes and literalises the next
    /// character. Used to prove escaping is lossless.
    fn unescape_like(escaped: &str) -> String {
        let mut out = String::new();
        let mut chars = escaped.chars();
        while let Some(c) = chars.next() {
            if c == '!' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    proptest! {
        /// Escaping any string is lossless (de-escaping recovers the original) and
        /// leaves no live `%`/`_` wildcard: every one is preceded by the `!` escape.
        #[test]
        fn escape_like_wildcards_is_lossless_and_neutralises_wildcards(s in ".*") {
            let escaped = escape_like_wildcards(&s);
            prop_assert_eq!(unescape_like(&escaped), s.clone());

            let chars: Vec<char> = escaped.chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                if c == '%' || c == '_' {
                    prop_assert!(
                        i > 0 && chars[i - 1] == '!',
                        "wildcard {:?} at {} not escaped in {:?}",
                        c, i, escaped
                    );
                }
            }
        }

        /// Truncation never panics on a multi-byte boundary, returns a prefix of the
        /// input, and respects the byte budget.
        #[test]
        fn truncate_to_char_boundary_never_panics(s in ".*", n in 0usize..64) {
            let t = truncate_to_char_boundary(&s, n);
            prop_assert!(t.len() <= n);
            prop_assert!(s.starts_with(t));
            // The result is always valid UTF-8 (it is a &str slice), the implicit
            // guarantee that the raw `&s[..n]` slice would violate mid-codepoint.
        }
    }
}
