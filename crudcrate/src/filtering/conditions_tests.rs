use super::*;

/// Test that malicious field names are rejected
#[test]
fn test_field_name_validation_rejects_sql_injection() {
    // These are currently rejected by the basic validation
    let rejected_names = vec![
        "../../../etc/passwd", // Path traversal (contains ..)
        "id..name",            // Double dots
        "_internal",           // Starts with underscore
        "",                    // Empty
    ];

    for malicious_name in rejected_names {
        assert!(
            !is_valid_field_name(malicious_name),
            "Should reject malicious field name: {malicious_name}"
        );
    }

    // Test too long separately
    let too_long = "a".repeat(101);
    assert!(
        !is_valid_field_name(&too_long),
        "Should reject field names longer than 100 chars"
    );
}

/// Test that valid field names are accepted
#[test]
fn test_field_name_validation_accepts_valid_names() {
    let valid_names = vec!["id", "user_name", "created_at", "field123"];

    for valid_name in valid_names {
        assert!(
            is_valid_field_name(valid_name),
            "Should accept valid field name: {valid_name}"
        );
    }

    // Test max length separately
    let max_length_name = "a".repeat(100);
    assert!(
        is_valid_field_name(&max_length_name),
        "Should accept 100-char field name"
    );
}

/// Test that excessively long field values are rejected
#[test]
fn test_field_value_length_validation() {
    let short_value = "a".repeat(100);
    let max_value = "a".repeat(MAX_FIELD_VALUE_LENGTH);
    let too_long_value = "a".repeat(MAX_FIELD_VALUE_LENGTH + 1);

    assert!(
        validate_field_value(&short_value),
        "Short values should be valid"
    );
    assert!(
        validate_field_value(&max_value),
        "Max length values should be valid"
    );
    assert!(
        !validate_field_value(&too_long_value),
        "Overly long values should be invalid"
    );
}

/// TDD: Pagination should enforce maximum page size
/// This test will FAIL until we add `MAX_PAGE_SIZE` enforcement
#[test]
fn test_pagination_enforces_max_page_size() {
    const MAX_PAGE_SIZE: u64 = 1000;

    let params = crate::models::FilterOptions {
        page: Some(1),
        per_page: Some(999_999), // Requesting huge page size
        ..Default::default()
    };

    let (_offset, limit) = parse_pagination(&params);

    // After fix: Should be capped at MAX_PAGE_SIZE
    assert!(
        limit <= MAX_PAGE_SIZE,
        "Page size should be capped at {MAX_PAGE_SIZE}, got {limit}"
    );
}

/// TDD: Pagination should enforce maximum offset
/// This test will FAIL until we add `MAX_OFFSET` enforcement
#[test]
fn test_pagination_enforces_max_offset() {
    const MAX_OFFSET: u64 = 1_000_000;

    let params = crate::models::FilterOptions {
        page: Some(1_000_000), // Huge page number
        per_page: Some(100),
        ..Default::default()
    };

    let (offset, _limit) = parse_pagination(&params);

    // After fix: Should be capped at MAX_OFFSET
    assert!(
        offset <= MAX_OFFSET,
        "Offset should be capped at {MAX_OFFSET}, got {offset}"
    );
}

/// TDD: Pagination should handle overflow with `saturating_mul`
/// This test will FAIL until we fix the overflow panic
#[test]
fn test_pagination_handles_overflow_gracefully() {
    let params = crate::models::FilterOptions {
        page: Some(u64::MAX),
        per_page: Some(u64::MAX),
        ..Default::default()
    };

    // Should NOT panic - should use saturating arithmetic
    let (_offset, _limit) = parse_pagination(&params);
    // After fix: This should succeed without panic
}

/// A zero page size is clamped up to one row rather than yielding an empty page
#[test]
fn test_pagination_clamps_zero_page_size() {
    let params = crate::models::FilterOptions {
        page: Some(1),
        per_page: Some(0),
        ..Default::default()
    };

    let (offset, limit) = parse_pagination(&params);
    assert_eq!(offset, 0);
    assert_eq!(limit, 1);
}

/// Test comparison operator parsing
#[test]
fn test_comparison_operator_parsing() {
    assert_eq!(parse_comparison_operator("age_gte"), Some(("age", ">=")));
    assert_eq!(parse_comparison_operator("age_lte"), Some(("age", "<=")));
    assert_eq!(parse_comparison_operator("age_gt"), Some(("age", ">")));
    assert_eq!(parse_comparison_operator("age_lt"), Some(("age", "<")));
    assert_eq!(parse_comparison_operator("age_neq"), Some(("age", "!=")));
    assert_eq!(parse_comparison_operator("age"), None);
}

/// The LIKE paths in this module share search.rs's `!`-based escaper, which is
/// always paired with an explicit `ESCAPE '!'` clause (see
/// `test_build_comparison_expr_like_escapes_wildcards`). Backslash escaping was
/// removed because it is a no-op on `SQLite` (`.like()` emits no ESCAPE clause).
#[test]
fn test_escape_like_wildcards() {
    assert_eq!(escape_like_wildcards("normal text"), "normal text");
    assert_eq!(escape_like_wildcards("test%"), "test!%");
    assert_eq!(escape_like_wildcards("test_value"), "test!_value");
    assert_eq!(escape_like_wildcards("%_"), "!%!_");
    assert_eq!(escape_like_wildcards("!"), "!!");
    assert_eq!(escape_like_wildcards("100% complete"), "100!% complete");
}

/// Enum columns compare as text: Postgres needs `CAST(col AS TEXT)`, the
/// other backends use the column directly.
#[test]
fn test_enum_text_expr_casts_only_on_postgres() {
    use crate::filtering::test_support::entity;

    let pg = format!(
        "{:?}",
        enum_text_expr(entity::Column::Status, DatabaseBackend::Postgres)
    );
    assert!(pg.contains("TEXT"), "expected a cast on Postgres: {pg}");

    for backend in [DatabaseBackend::MySql, DatabaseBackend::Sqlite] {
        let other = format!("{:?}", enum_text_expr(entity::Column::Status, backend));
        assert!(!other.contains("TEXT"), "no cast expected: {other}");
    }
}

/// The LIKE fallback for `q` searches over an enum column uppercases the
/// (cast) column so native Postgres enums match string bind parameters.
#[test]
fn test_fulltext_like_fallback_enum_column() {
    use crate::filtering::test_support::{EnumSearchResource, entity};

    let filters = HashMap::from([(
        "q".to_string(),
        serde_json::Value::String("act".to_string()),
    )]);
    let searchable = [("status", entity::Column::Status)];

    let pg = handle_fulltext_search::<EnumSearchResource>(
        &filters,
        &searchable,
        DatabaseBackend::Postgres,
    )
    .expect("q over a searchable column yields a condition");
    let pg_sql = format!("{pg:?}");
    assert!(
        pg_sql.contains("TEXT") && pg_sql.contains("Upper"),
        "expected upper-cased cast on Postgres: {pg_sql}"
    );

    let sqlite = handle_fulltext_search::<EnumSearchResource>(
        &filters,
        &searchable,
        DatabaseBackend::Sqlite,
    )
    .expect("q over a searchable column yields a condition");
    let sqlite_sql = format!("{sqlite:?}");
    assert!(
        !sqlite_sql.contains("TEXT") && sqlite_sql.contains("Upper"),
        "expected plain upper-cased column on SQLite: {sqlite_sql}"
    );
}

/// Enum array (`IN`) filters cast the column on Postgres and uppercase the
/// bound values on every backend.
#[test]
fn test_process_array_filter_enum_postgres_cast() {
    use crate::filtering::test_support::entity;

    let values = [serde_json::json!("active"), serde_json::json!("archived")];
    let expr = process_array_filter(
        &values,
        "status",
        entity::Column::Status,
        true,
        DatabaseBackend::Postgres,
    )
    .expect("enum IN list is buildable")
    .expect("enum IN list yields a condition");
    let sql = format!("{expr:?}");
    assert!(
        sql.contains("TEXT") && sql.contains("ACTIVE"),
        "expected cast column and upper-cased values: {sql}"
    );
}

/// `apply_filters` combines the fulltext `q` term with per-field filters of
/// every JSON value kind; unknown fields and unsupported values are skipped.
#[test]
fn test_apply_filters_all_value_kinds() {
    use crate::filtering::test_support::{EnumSearchResource, entity};

    let searchable = [
        ("name", entity::Column::Name),
        ("status", entity::Column::Status),
        ("id", entity::Column::Id),
    ];
    let filter = serde_json::json!({
        "q": "act",
        "name": "todo",
        "status_neq": "archived",
        "id": 3,
        "name_like": null,
        "status": ["active", "draft"],
        "unknown_field": "ignored",
        "name;drop": "ignored",
        "id_gt": {"not": "supported"},
    })
    .to_string();

    let cond =
        apply_filters::<EnumSearchResource>(Some(filter), &searchable, DatabaseBackend::Sqlite)
            .expect("valid filter JSON");
    let sql = format!("{cond:?}");
    assert!(sql.contains("Upper"), "q and string filters render: {sql}");
    assert!(sql.contains("TODO"), "equality value uppercased: {sql}");

    let null_checks = apply_filters::<EnumSearchResource>(
        Some(serde_json::json!({"name": null, "status_neq": null}).to_string()),
        &searchable,
        DatabaseBackend::Sqlite,
    )
    .expect("valid filter JSON");
    let sql = format!("{null_checks:?}");
    assert!(
        sql.contains("Is, Keyword(Null)") && sql.contains("IsNot, Keyword(Null)"),
        "{sql}"
    );

    let bool_filter = apply_filters::<EnumSearchResource>(
        Some(serde_json::json!({"id": true}).to_string()),
        &searchable,
        DatabaseBackend::Sqlite,
    )
    .expect("valid filter JSON");
    assert!(format!("{bool_filter:?}").contains("Bool"));
}

/// String comparison operators map onto the expected SQL comparisons,
/// including the default equality arm.
#[test]
fn test_apply_string_comparison_operators() {
    use crate::filtering::test_support::entity;

    for (op, needle) in [
        ("=", "Equal"),
        ("!=", "NotEqual"),
        (">=", "GreaterThanOrEqual"),
        ("<=", "SmallerThanOrEqual"),
        (">", "GreaterThan,"),
        ("<", "SmallerThan,"),
    ] {
        let expr = apply_string_comparison(entity::Column::Name, op, "x");
        let sql = format!("{expr:?}");
        assert!(
            sql.contains(needle),
            "operator {op} renders {needle}: {sql}"
        );
    }
}

/// Enum equality filters route through the same Postgres cast.
#[test]
fn test_process_string_filter_enum_postgres_cast() {
    use crate::filtering::test_support::{EnumSearchResource, entity};

    let expr = process_string_filter::<EnumSearchResource>(
        "status",
        "=",
        "active",
        entity::Column::Status,
        DatabaseBackend::Postgres,
    )
    .expect("enum equality yields a condition");
    let sql = format!("{expr:?}");
    assert!(
        sql.contains("TEXT") && sql.contains("ACTIVE"),
        "expected cast column and upper-cased value: {sql}"
    );
}

// ========================================================================
// build_comparison_expr: direct coverage of the public joined-filter
// expression builder used by derive-generated resolve_joined_filters.
// ========================================================================

mod cmp_entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "cmp_things")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub service_date: DateTimeWithTimeZone,
        pub cost: Decimal,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use crate::filtering::joined::FilterOperator;

/// `build_comparison_expr` for the cases that cannot fail: an unparseable element
/// of an array value is a 400, which the tests below assert on directly.
fn cmp_expr<C: sea_orm::ColumnTrait + Copy>(
    column: C,
    operator: FilterOperator,
    value: &serde_json::Value,
) -> Option<Expr> {
    build_comparison_expr(column, operator, value).expect("filter value accepted")
}

/// Render an expression to inlined `SQLite` SQL so the ESCAPE clause and the
/// (escaped) bound pattern are both visible as text.
fn cmp_sql(expr: Expr) -> String {
    use sea_orm::sea_query::{Query, SqliteQueryBuilder};
    Query::select()
        .column(cmp_entity::Column::Id)
        .from(cmp_entity::Entity)
        .and_where(expr)
        .to_string(SqliteQueryBuilder)
}

/// `table_column_ref` must render as a table-qualified column, matching what
/// the removed `ColumnRef::TableColumn` produced in Sea-Query 0.x.
#[test]
fn test_table_column_ref_renders_qualified() {
    use sea_orm::sea_query::{Query, SqliteQueryBuilder};
    let sql = Query::select()
        .column(table_column_ref("vehicles", "customer_id"))
        .from(cmp_entity::Entity)
        .to_string(SqliteQueryBuilder);
    assert!(
        sql.contains(r#""vehicles"."customer_id""#),
        "expected table-qualified column: {sql}"
    );
}

/// `table_column_ref` accepts the `(DynIden, DynIden)` pair returned by
/// `ColumnTrait::as_column_ref`, the form the generated join code uses.
#[test]
fn test_table_column_ref_accepts_as_column_ref_pair() {
    use sea_orm::ColumnTrait;
    use sea_orm::sea_query::{Query, SqliteQueryBuilder};
    let (table, column) = cmp_entity::Column::Name.as_column_ref();
    let sql = Query::select()
        .column(table_column_ref(table, column))
        .from(cmp_entity::Entity)
        .to_string(SqliteQueryBuilder);
    assert!(
        sql.contains(r#""cmp_things"."name""#),
        "expected table-qualified column: {sql}"
    );
}

/// A1 regression: the joined `_like` path must escape user wildcards with `!`
/// AND declare `ESCAPE '!'` so the escaping is not a no-op on `SQLite`.
#[test]
fn test_build_comparison_expr_like_escapes_wildcards() {
    let expr = cmp_expr(
        cmp_entity::Column::Name,
        FilterOperator::Like,
        &serde_json::json!("100%"),
    )
    .expect("Like on a string builds an expression");
    let sql = cmp_sql(expr);
    assert!(
        sql.contains("ESCAPE '!'"),
        "LIKE must declare ESCAPE '!': {sql}"
    );
    assert!(
        sql.contains("100!%"),
        "user-supplied wildcard must be escaped with !: {sql}"
    );
}

#[test]
fn test_build_comparison_expr_string_ops_build() {
    for op in [
        FilterOperator::Eq,
        FilterOperator::Neq,
        FilterOperator::Gt,
        FilterOperator::Gte,
        FilterOperator::Lt,
        FilterOperator::Lte,
    ] {
        assert!(
            cmp_expr(cmp_entity::Column::Name, op, &serde_json::json!("abc")).is_some(),
            "string {op:?} should build an expression"
        );
    }
}

#[test]
fn test_build_comparison_expr_empty_and_overlong_string_none() {
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!("")
        )
        .is_none()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!("   "),
        )
        .is_none()
    );
    let overlong = "a".repeat(10_001);
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!(overlong),
        )
        .is_none()
    );
}

#[test]
fn test_build_comparison_expr_uuid_only_eq_neq() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!(uuid)
        )
        .is_some()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Neq,
            &serde_json::json!(uuid)
        )
        .is_some()
    );
    // Ranges and LIKE on a UUID are meaningless -> None.
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Gt,
            &serde_json::json!(uuid)
        )
        .is_none()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Like,
            &serde_json::json!(uuid)
        )
        .is_none()
    );
}

#[test]
fn test_build_comparison_expr_number_ops_and_rejections() {
    for op in [
        FilterOperator::Eq,
        FilterOperator::Neq,
        FilterOperator::Gt,
        FilterOperator::Gte,
        FilterOperator::Lt,
        FilterOperator::Lte,
    ] {
        assert!(cmp_expr(cmp_entity::Column::Id, op, &serde_json::json!(42)).is_some());
        assert!(cmp_expr(cmp_entity::Column::Id, op, &serde_json::json!(3.5)).is_some());
    }
    // In / IsNull are not valid against a scalar number -> None.
    assert!(
        cmp_expr(
            cmp_entity::Column::Id,
            FilterOperator::In,
            &serde_json::json!(42)
        )
        .is_none()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Id,
            FilterOperator::IsNull,
            &serde_json::json!(42),
        )
        .is_none()
    );
}

/// A JSON integer above `i64::MAX` must bind as an exact `u64`, not fall through
/// to a lossy `f64`. 9223372036854775810 (= `i64::MAX` as u64 + 3) is NOT exactly
/// representable in `f64` (it rounds to 9223372036854775808), so the rendered SQL
/// proves whether the value was preserved.
#[test]
fn test_build_comparison_expr_u64_above_i64_max_binds_exact() {
    let big: u64 = (i64::MAX as u64) + 3;
    assert_eq!(big, 9_223_372_036_854_775_810);
    let v = serde_json::json!(big);
    assert!(v.as_i64().is_none(), "value must exceed i64::MAX");

    let expr = cmp_expr(cmp_entity::Column::Id, FilterOperator::Gte, &v)
        .expect("u64 value builds an expression");
    let sql = cmp_sql(expr);
    assert!(
        sql.contains("9223372036854775810"),
        "u64 above i64::MAX must bind exactly, got lossy SQL: {sql}"
    );
}

/// Direct callers of the public `build_comparison_expr` are also protected: an
/// array longer than the element cap yields `None` instead of an oversized `IN`.
#[test]
fn test_build_comparison_expr_rejects_overlong_array() {
    let arr: Vec<serde_json::Value> = (0..=i64::try_from(super::MAX_FILTER_ARRAY_LEN).unwrap())
        .map(|n| serde_json::json!(n))
        .collect();
    assert!(
        cmp_expr(
            cmp_entity::Column::Id,
            FilterOperator::In,
            &serde_json::Value::Array(arr)
        )
        .is_none(),
        "array over MAX_FILTER_ARRAY_LEN must not build an IN expression"
    );
}

#[test]
fn test_build_comparison_expr_bool_eq_neq_only() {
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!(true)
        )
        .is_some()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Neq,
            &serde_json::json!(false),
        )
        .is_some()
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Gt,
            &serde_json::json!(true)
        )
        .is_none()
    );
}

#[test]
fn test_build_comparison_expr_array_null_object() {
    // Non-empty array -> IN.
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::In,
            &serde_json::json!(["a", "b"]),
        )
        .is_some()
    );
    // An array of only objects has no extractable scalars -> None.
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::In,
            &serde_json::json!([{"k": "v"}]),
        )
        .is_none()
    );
    // Null + Eq/IsNull -> IS NULL; Null + Neq -> IS NOT NULL; other operators -> None.
    let eq_null = cmp_expr(
        cmp_entity::Column::Name,
        FilterOperator::Eq,
        &serde_json::Value::Null,
    )
    .expect("Eq + null builds an expression");
    assert!(
        cmp_sql(eq_null).contains("IS NULL"),
        "Eq + null must render IS NULL"
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::IsNull,
            &serde_json::Value::Null,
        )
        .is_some()
    );
    let neq_null = cmp_expr(
        cmp_entity::Column::Name,
        FilterOperator::Neq,
        &serde_json::Value::Null,
    )
    .expect("Neq + null builds an expression");
    assert!(
        cmp_sql(neq_null).contains("IS NOT NULL"),
        "Neq + null must render IS NOT NULL (paired/has-value filter)"
    );
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Gt,
            &serde_json::Value::Null
        )
        .is_none()
    );
    // Object value is unsupported -> None.
    assert!(
        cmp_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::json!({"k": "v"}),
        )
        .is_none()
    );
}

/// The bound parameters an expression carries, in order.
fn cmp_bound_values(expr: Expr) -> Vec<sea_orm::Value> {
    use sea_orm::sea_query::{Query, SqliteQueryBuilder};
    Query::select()
        .column(cmp_entity::Column::Id)
        .from(cmp_entity::Entity)
        .and_where(expr)
        .build(SqliteQueryBuilder)
        .1
        .0
}

/// An `IN` list over a `timestamptz` column binds `DateTime<FixedOffset>` values,
/// not text. Postgres rejects `ts_col IN ('2026-01-01T00:00:00Z')`.
#[test]
fn test_timestamptz_array_binds_typed_values() {
    let values = serde_json::json!(["2026-01-01T00:00:00Z", "2026-02-01T13:45:30+02:00"]);
    let expr = cmp_expr(cmp_entity::Column::ServiceDate, FilterOperator::In, &values)
        .expect("timestamptz IN list yields a condition");
    let bound = cmp_bound_values(expr);
    assert_eq!(bound.len(), 2);
    assert!(
        bound
            .iter()
            .all(|v| matches!(v, sea_orm::Value::ChronoDateTimeWithTimeZone(_))),
        "expected typed timestamp binds, got {bound:?}"
    );
}

/// One unparseable element rejects the request. Dropping the clause would return
/// every row, and dropping the element would silently answer a different query.
#[test]
fn test_timestamptz_array_rejects_unparseable_element() {
    let values = serde_json::json!(["2026-01-01T00:00:00Z", "not-a-date"]);
    let err = build_comparison_expr(cmp_entity::Column::ServiceDate, FilterOperator::In, &values)
        .expect_err("an unparseable element is a client error");
    assert!(
        matches!(err, crate::errors::ApiError::BadRequest { .. }),
        "expected 400, got {err:?}"
    );
}

/// The 400 names the field the client sent, but not the column's SQL type: joined
/// filters deliberately keep the schema hidden.
#[test]
fn test_typed_array_error_does_not_name_the_column_type() {
    let values = serde_json::json!(["not-a-date"]);
    let err = build_comparison_expr(cmp_entity::Column::ServiceDate, FilterOperator::In, &values)
        .expect_err("an unparseable element is a client error");
    let message = format!("{err:?}");
    assert!(message.contains("service_date"), "{message}");
    assert!(
        !message.to_lowercase().contains("timestamp"),
        "message must not disclose the SQL type: {message}"
    );
}

/// Numeric strings against an integer column bind integers, matching what the
/// scalar path already does.
#[test]
fn test_integer_array_of_strings_binds_integers() {
    let values = serde_json::json!(["1", "3"]);
    let expr = cmp_expr(cmp_entity::Column::Id, FilterOperator::In, &values)
        .expect("integer IN list yields a condition");
    let bound = cmp_bound_values(expr);
    assert_eq!(bound.len(), 2);
    assert!(
        bound.iter().all(|v| matches!(v, sea_orm::Value::BigInt(_))),
        "expected integer binds, got {bound:?}"
    );
}

/// Decimals arrive as JSON numbers or as strings; strings are the exact form.
#[test]
fn test_decimal_array_accepts_numbers_and_strings() {
    let values = serde_json::json!([10.5, "10.50"]);
    let expr = cmp_expr(cmp_entity::Column::Cost, FilterOperator::In, &values)
        .expect("decimal IN list yields a condition");
    let bound = cmp_bound_values(expr);
    assert_eq!(bound.len(), 2);
    assert!(
        bound
            .iter()
            .all(|v| matches!(v, sea_orm::Value::Decimal(_))),
        "expected decimal binds, got {bound:?}"
    );
}

/// A float against an integer column can never match a row, so it is rejected
/// rather than binding a value the column cannot hold.
#[test]
fn test_integer_array_rejects_fractional_element() {
    let values = serde_json::json!([2020, 2020.5]);
    assert!(
        build_comparison_expr(cmp_entity::Column::Id, FilterOperator::In, &values).is_err(),
        "a fractional value on an integer column must be rejected"
    );
}

/// Text columns keep the string `IN` list: the type routing is for columns whose
/// SQL type the backend compares natively.
#[test]
fn test_text_array_keeps_string_binds() {
    let values = serde_json::json!(["alpha", "beta"]);
    let expr = cmp_expr(cmp_entity::Column::Name, FilterOperator::In, &values)
        .expect("text IN list yields a condition");
    let bound = cmp_bound_values(expr);
    assert_eq!(bound.len(), 2);
    assert!(
        bound.iter().all(|v| matches!(v, sea_orm::Value::String(_))),
        "expected string binds, got {bound:?}"
    );
}

/// The scalar arm routes by column type too, so a hand-written
/// `resolve_joined_filters` comparing a timestamptz child column binds a timestamp.
#[test]
fn test_scalar_string_on_typed_column_binds_typed_value() {
    let expr = cmp_expr(
        cmp_entity::Column::ServiceDate,
        FilterOperator::Gte,
        &serde_json::json!("2026-01-15T13:45:30+02:00"),
    )
    .expect("RFC 3339 value on a timestamptz column yields a condition");
    let bound = cmp_bound_values(expr);
    assert!(
        matches!(
            bound.as_slice(),
            [sea_orm::Value::ChronoDateTimeWithTimeZone(_)]
        ),
        "expected a typed timestamp bind, got {bound:?}"
    );
}

/// A scalar that does not parse for the column type is still dropped rather than
/// rejected, matching the main-entity path.
#[test]
fn test_scalar_unparseable_on_typed_column_is_skipped() {
    assert!(
        cmp_expr(
            cmp_entity::Column::ServiceDate,
            FilterOperator::Gte,
            &serde_json::json!("2026-01-15"),
        )
        .is_none()
    );
}

/// Each operator renders a native `col <op> value` against the real column, with
/// no `UPPER()` wrapper and the value bound rather than spliced. The unknown
/// operator falls back to equality.
#[test]
fn test_apply_typed_comparison_operators() {
    // (input operator, symbol sea-query renders; inequality is `<>`, not `!=`)
    let cases = [
        (">=", ">="),
        ("<=", "<="),
        (">", ">"),
        ("<", "<"),
        ("!=", "<>"),
        ("unknown", "="),
    ];
    for (op, rendered) in cases {
        let expr = apply_typed_comparison(cmp_entity::Column::Id, op, 18_i64);
        let sql = cmp_sql(expr);
        assert!(
            sql.contains(&format!(r#""id" {rendered} 18"#)),
            "operator {op} should render `id {rendered} 18`: {sql}"
        );
    }
}

/// Test JSON filter parsing
#[test]
fn test_parse_filter_json_valid() {
    let filter_str = Some(r#"{"name": "John", "age": 30}"#.to_string());
    let parsed = parse_filter_json(filter_str).expect("valid filter");

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed.get("name").and_then(|v| v.as_str()), Some("John"));
    assert_eq!(
        parsed.get("age").and_then(sea_orm::JsonValue::as_i64),
        Some(30)
    );
}

#[test]
fn test_parse_filter_json_invalid() {
    // Invalid JSON historically returns empty (preserved behavior); over-limit
    // is the new strict path tested separately below.
    let filter_str = Some("{invalid json}".to_string());
    let parsed = parse_filter_json(filter_str).expect("invalid-json path is lenient");
    assert_eq!(parsed.len(), 0);
}

#[test]
fn test_parse_filter_json_none() {
    let parsed = parse_filter_json(None).expect("None is valid");
    assert_eq!(parsed.len(), 0);
}

#[test]
fn test_parse_filter_json_empty() {
    let filter_str = Some("{}".to_string());
    let parsed = parse_filter_json(filter_str).expect("empty object is valid");
    assert_eq!(parsed.len(), 0);
}

#[test]
fn test_parse_filter_json_at_limit_is_accepted() {
    let mut entries: Vec<String> = Vec::with_capacity(MAX_FILTER_CLAUSES);
    for i in 0..MAX_FILTER_CLAUSES {
        entries.push(format!("\"f{i}\":{i}"));
    }
    let filter_str = Some(format!("{{{}}}", entries.join(",")));
    let parsed = parse_filter_json(filter_str).expect("at-limit filter must be accepted");
    assert_eq!(parsed.len(), MAX_FILTER_CLAUSES);
}

#[test]
fn test_parse_filter_json_rejects_when_over_limit() {
    let mut entries: Vec<String> = Vec::with_capacity(MAX_FILTER_CLAUSES + 1);
    for i in 0..=MAX_FILTER_CLAUSES {
        entries.push(format!("\"f{i}\":{i}"));
    }
    let filter_str = Some(format!("{{{}}}", entries.join(",")));
    let err = parse_filter_json(filter_str)
        .expect_err("over-limit filter must be rejected, not silently dropped");
    assert!(
        matches!(err, crate::errors::ApiError::BadRequest { .. }),
        "expected BadRequest, got {err:?}"
    );
}

/// An array-valued filter at the element cap is accepted, and one element over is
/// rejected with `BadRequest` rather than fanning out into an oversized `IN (...)`.
#[test]
fn test_parse_filter_json_rejects_overlong_array() {
    let at_limit: Vec<i64> = (0..i64::try_from(MAX_FILTER_ARRAY_LEN).unwrap()).collect();
    let filter_str = Some(serde_json::json!({ "id": at_limit }).to_string());
    let parsed = parse_filter_json(filter_str).expect("array at the cap is accepted");
    assert_eq!(parsed.len(), 1);

    let over_limit: Vec<i64> = (0..=i64::try_from(MAX_FILTER_ARRAY_LEN).unwrap()).collect();
    let filter_str = Some(serde_json::json!({ "id": over_limit }).to_string());
    let err =
        parse_filter_json(filter_str).expect_err("array one element over the cap must be rejected");
    assert!(
        matches!(err, crate::errors::ApiError::BadRequest { .. }),
        "expected BadRequest, got {err:?}"
    );
}

/// Test comparison operators with edge cases
#[test]
fn test_comparison_operator_edge_cases() {
    // Field name that ends with operator-like suffix but isn't one
    assert_eq!(parse_comparison_operator("created_at"), None);
    assert_eq!(parse_comparison_operator("_gte"), Some(("", ">=")));

    // Multiple suffixes (should match the longest/last one)
    assert_eq!(
        parse_comparison_operator("field_gte_lte"),
        Some(("field_gte", "<="))
    );
}

/// Test field name validation edge cases
#[test]
fn test_field_name_validation_edge_cases() {
    // Boundary cases
    assert!(is_valid_field_name("a")); // Single char
    assert!(is_valid_field_name("a".repeat(100).as_str())); // Exactly 100
    assert!(!is_valid_field_name("a".repeat(101).as_str())); // 101

    // Special chars that should be allowed
    assert!(is_valid_field_name("field_123"));
    assert!(is_valid_field_name("Field123"));

    // Special chars that should be rejected
    assert!(!is_valid_field_name("field..name"));
    assert!(!is_valid_field_name(".."));
    assert!(!is_valid_field_name("_private"));
}

/// The typed builder accepts the numeric Rust types the number-filter path feeds
/// it (i64, f64, and u64 above `i64::MAX`), binding each without a lossy cast.
#[test]
fn test_apply_typed_comparison_various_types() {
    let i64_sql = cmp_sql(apply_typed_comparison(
        cmp_entity::Column::Id,
        ">=",
        100_i64,
    ));
    assert!(i64_sql.contains(r#""id" >= 100"#), "{i64_sql}");

    let f64_sql = cmp_sql(apply_typed_comparison(
        cmp_entity::Column::Id,
        "<=",
        99.99_f64,
    ));
    assert!(
        f64_sql.contains(r#""id" <= "#) && f64_sql.contains("99.99"),
        "{f64_sql}"
    );

    // A u64 above i64::MAX must bind exactly, not fall through to a lossy f64.
    let big: u64 = (i64::MAX as u64) + 3;
    let u64_sql = cmp_sql(apply_typed_comparison(cmp_entity::Column::Id, ">", big));
    assert!(u64_sql.contains("9223372036854775810"), "{u64_sql}");
}

/// `typed_value_for_column` parses a valid string for every supported column
/// type; text-like types return `None` so the caller keeps the string path.
#[test]
fn typed_value_for_column_parses_supported_types() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let ok = [
        (ColumnType::Date, "2026-01-15"),
        (ColumnType::Time, "13:45:30"),
        (ColumnType::DateTime, "2026-01-15T13:45:30"),
        (ColumnType::Timestamp, "2026-01-15 13:45:30"),
        (
            ColumnType::TimestampWithTimeZone,
            "2026-01-15T13:45:30+02:00",
        ),
        (ColumnType::Decimal(None), "10.50"),
        (ColumnType::Money(None), "10.50"),
        (ColumnType::Uuid, uuid),
        (ColumnType::Float, "1.5"),
        (ColumnType::Double, "1.5"),
        (ColumnType::TinyInteger, "1"),
        (ColumnType::SmallInteger, "-7"),
        (ColumnType::Integer, "42"),
        (ColumnType::BigInteger, "9000000000"),
        (ColumnType::TinyUnsigned, "1"),
        (ColumnType::SmallUnsigned, "7"),
        (ColumnType::Unsigned, "42"),
        (ColumnType::BigUnsigned, "9000000000"),
        (ColumnType::Boolean, "true"),
    ];
    for (col_type, raw) in ok {
        assert!(
            typed_value_for_column(&col_type, raw).is_some(),
            "{col_type:?} should parse {raw:?} to a typed value"
        );
    }
    // Text-like columns are not typed-bound; the caller keeps the string path.
    assert!(typed_value_for_column(&ColumnType::Text, "anything").is_none());
    assert!(typed_value_for_column(&ColumnType::Char(None), "x").is_none());
}

/// An unparseable value for a typed column yields `None`, so the caller drops
/// that clause (fail-closed) rather than emitting a comparison the backend rejects.
#[test]
fn typed_value_for_column_rejects_unparseable_values() {
    let bad = [
        (ColumnType::Date, "not-a-date"),
        (ColumnType::Time, "25:99:99"),
        (ColumnType::DateTime, "nope"),
        (ColumnType::TimestampWithTimeZone, "2026-01-15"), // missing time/offset
        (ColumnType::Decimal(None), "abc"),
        (ColumnType::Uuid, "not-a-uuid"),
        (ColumnType::Float, "one"),
        (ColumnType::Double, "1.2.3"),
        (ColumnType::Integer, "3.5"), // not an integer
        (ColumnType::Unsigned, "-1"), // negative on an unsigned column
        (ColumnType::Boolean, "1"),   // only literal true/false parse
    ];
    for (col_type, raw) in bad {
        assert!(
            typed_value_for_column(&col_type, raw).is_none(),
            "{col_type:?} should reject {raw:?}"
        );
    }
}

/// `parse_naive_datetime` accepts the `T`-separated and space-separated forms and
/// a full RFC 3339 timestamp (normalised to naive UTC), and rejects junk.
#[test]
fn parse_naive_datetime_accepts_supported_formats() {
    assert!(parse_naive_datetime("2026-01-15T13:45:30").is_some());
    assert!(parse_naive_datetime("2026-01-15 13:45:30").is_some());
    assert!(parse_naive_datetime("2026-01-15T13:45:30+02:00").is_some());
    assert!(parse_naive_datetime("garbage").is_none());
}

/// `binds_typed_value` selects the typed path for non-text columns and leaves
/// text/char columns on the case-insensitive string path.
#[test]
fn binds_typed_value_covers_non_text_columns() {
    for col_type in [
        ColumnType::Date,
        ColumnType::TimestampWithTimeZone,
        ColumnType::Decimal(None),
        ColumnType::Uuid,
        ColumnType::Double,
        ColumnType::BigInteger,
        ColumnType::Boolean,
    ] {
        assert!(
            binds_typed_value(&col_type),
            "{col_type:?} should bind typed"
        );
    }
    assert!(!binds_typed_value(&ColumnType::Text));
    assert!(!binds_typed_value(&ColumnType::Char(None)));
}

/// Number comparisons bind the real column with a typed value for each JSON
/// numeric kind (i64, u64 above `i64::MAX`, f64).
#[test]
fn number_comparisons_bind_typed_values() {
    use crate::filtering::joined::FilterOperator;

    let gte = ordered_comparison(cmp_entity::Column::Id, FilterOperator::Gte, 42_i64).unwrap();
    assert!(cmp_sql(gte).contains(r#""id" >= 42"#));

    // A u64 above i64::MAX must bind exactly, not fall through to a lossy f64.
    let big = (i64::MAX as u64) + 3;
    let lte = ordered_comparison(cmp_entity::Column::Id, FilterOperator::Lte, big).unwrap();
    assert!(cmp_sql(lte).contains("9223372036854775810"));

    let lt = ordered_comparison(cmp_entity::Column::Id, FilterOperator::Lt, 1.5_f64).unwrap();
    let sql = cmp_sql(lt);
    assert!(sql.contains(r#""id" < "#) && sql.contains("1.5"), "{sql}");

    let eq = ordered_comparison(cmp_entity::Column::Id, FilterOperator::Eq, 42_i64).unwrap();
    assert!(cmp_sql(eq).contains(r#""id" = 42"#));

    assert!(ordered_comparison(cmp_entity::Column::Id, FilterOperator::Like, 42_i64).is_none());
}

// ========================================================================
// PAGINATION TESTS - Range parsing and default pagination
// ========================================================================

/// Test `parse_range` with valid JSON array
#[test]
fn test_parse_range_valid() {
    let (start, end) = parse_range(Some("[0,9]".to_string()));
    assert_eq!(start, 0);
    assert_eq!(end, 9);

    let (start, end) = parse_range(Some("[10,19]".to_string()));
    assert_eq!(start, 10);
    assert_eq!(end, 19);

    let (start, end) = parse_range(Some("[50,74]".to_string()));
    assert_eq!(start, 50);
    assert_eq!(end, 74);
}

/// Test `parse_range` with invalid JSON returns default
#[test]
fn test_parse_range_invalid_json() {
    let (start, end) = parse_range(Some("invalid".to_string()));
    assert_eq!(start, 0);
    assert_eq!(end, 9);

    let (start, end) = parse_range(Some("[0]".to_string())); // Not enough elements
    assert_eq!(start, 0);
    assert_eq!(end, 9);

    let (start, end) = parse_range(Some("[]".to_string())); // Empty array
    assert_eq!(start, 0);
    assert_eq!(end, 9);
}

/// Test `parse_range` with None returns default
#[test]
fn test_parse_range_none() {
    let (start, end) = parse_range(None);
    assert_eq!(start, 0);
    assert_eq!(end, 9);
}

/// Test default pagination when no params provided
#[test]
fn test_pagination_default_values() {
    let params = crate::models::FilterOptions::default();
    let (offset, limit) = parse_pagination(&params);

    assert_eq!(offset, 0, "Default offset should be 0");
    assert_eq!(limit, 10, "Default limit should be 10");
}

/// Test pagination with range format calculates limit correctly
#[test]
fn test_pagination_range_calculates_limit() {
    let params = crate::models::FilterOptions {
        range: Some("[0,4]".to_string()),
        ..Default::default()
    };
    let (offset, limit) = parse_pagination(&params);

    assert_eq!(offset, 0, "Offset should be 0");
    assert_eq!(limit, 5, "Limit should be 5 for range [0,4]");

    // Test second page
    let params = crate::models::FilterOptions {
        range: Some("[5,9]".to_string()),
        ..Default::default()
    };
    let (offset, limit) = parse_pagination(&params);

    assert_eq!(offset, 5, "Offset should be 5");
    assert_eq!(limit, 5, "Limit should be 5 for range [5,9]");
}

/// Test `page/per_page` takes priority over range
#[test]
fn test_pagination_page_priority_over_range() {
    let params = crate::models::FilterOptions {
        page: Some(2),
        per_page: Some(15),
        range: Some("[0,4]".to_string()), // Should be ignored
        ..Default::default()
    };
    let (offset, limit) = parse_pagination(&params);

    assert_eq!(offset, 15, "Offset should be 15 (page 2 * 15 per_page)");
    assert_eq!(limit, 15, "Limit should be 15");
}

/// Test range pagination enforces max limits
#[test]
fn test_pagination_range_enforces_max_limits() {
    // Test max page size enforcement
    let params = crate::models::FilterOptions {
        range: Some("[0,9999]".to_string()), // Requesting 10000 items
        ..Default::default()
    };
    let (_offset, limit) = parse_pagination(&params);
    assert!(
        limit <= MAX_PAGE_SIZE,
        "Range limit should be capped at {MAX_PAGE_SIZE}"
    );

    // Test max offset enforcement
    let params = crate::models::FilterOptions {
        range: Some("[9999999,10000000]".to_string()), // Very large offset
        ..Default::default()
    };
    let (offset, _limit) = parse_pagination(&params);
    assert!(
        offset <= MAX_OFFSET,
        "Range offset should be capped at {MAX_OFFSET}"
    );
}

/// A3 regression: a huge `end` in the range branch must not overflow the `+ 1`
/// (which panics under overflow-checks). Should cap cleanly instead.
#[test]
fn test_pagination_range_huge_end_does_not_overflow() {
    let params = crate::models::FilterOptions {
        range: Some(format!("[0,{}]", u64::MAX)),
        ..Default::default()
    };
    let (offset, limit) = parse_pagination(&params);
    assert!(limit <= MAX_PAGE_SIZE, "limit must be capped, got {limit}");
    assert!(offset <= MAX_OFFSET, "offset must be capped, got {offset}");

    // Reversed range (end < start) must not panic either.
    let params = crate::models::FilterOptions {
        range: Some(format!("[{},{}]", u64::MAX, u64::MAX)),
        ..Default::default()
    };
    let (_offset, limit) = parse_pagination(&params);
    assert!(limit <= MAX_PAGE_SIZE);
}

/// Scenario: a client sends `{"name": []}`.
/// Expected behaviour: no row matches. Dropping the clause would return the whole
/// table, which is the opposite of the empty set the client asked for.
#[test]
fn test_empty_array_matches_nothing() {
    let expr = cmp_expr(
        cmp_entity::Column::Name,
        FilterOperator::In,
        &serde_json::json!([]),
    )
    .expect("empty array builds a condition");
    assert_eq!(cmp_bound_values(expr), vec![1i32.into(), 2i32.into()]);
}

/// The same guarantee on the main-entity path, which reaches `process_array_filter`.
#[test]
fn test_empty_array_matches_nothing_on_main_filter_path() {
    let expr = process_array_filter(
        &[],
        "name",
        cmp_entity::Column::Name,
        false,
        DatabaseBackend::Sqlite,
    )
    .expect("empty array is accepted")
    .expect("empty array builds a condition");
    assert_eq!(cmp_bound_values(expr), vec![1i32.into(), 2i32.into()]);
}

/// A bare number cannot be compared against a timestamp or uuid column on any
/// backend, so the clause is dropped rather than bound and rejected at 500.
#[test]
fn test_number_against_timestamp_column_is_skipped() {
    for op in [FilterOperator::Eq, FilterOperator::Gte, FilterOperator::Lt] {
        assert!(
            cmp_expr(cmp_entity::Column::ServiceDate, op, &serde_json::json!(5)).is_none(),
            "a number against a timestamptz column must not build a comparison"
        );
    }
}

/// Numeric columns keep their JSON binding: a fractional bound against an integer
/// column is well-defined SQL, and a `u64` above `i64::MAX` still binds exactly.
#[test]
fn test_number_against_numeric_column_keeps_json_binding() {
    let frac = cmp_expr(
        cmp_entity::Column::Id,
        FilterOperator::Gte,
        &serde_json::json!(3.5),
    )
    .expect("fractional bound on an integer column builds a comparison");
    assert!(cmp_sql(frac).contains("3.5"));

    let cost = cmp_expr(
        cmp_entity::Column::Cost,
        FilterOperator::Lte,
        &serde_json::json!(10.5),
    )
    .expect("fractional bound on a decimal column builds a comparison");
    assert!(cmp_sql(cost).contains("10.5"));
}
