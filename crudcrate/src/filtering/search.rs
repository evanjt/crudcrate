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
#[path = "search_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "search_prop_tests.rs"]
mod prop_tests;
