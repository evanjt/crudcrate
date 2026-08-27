use super::*;
use crate::filtering::joined::FilterOperator;
use proptest::prelude::*;

mod pe {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "pe_things")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

const OPS: [FilterOperator; 9] = [
    FilterOperator::Eq,
    FilterOperator::Neq,
    FilterOperator::Gt,
    FilterOperator::Gte,
    FilterOperator::Lt,
    FilterOperator::Lte,
    FilterOperator::Like,
    FilterOperator::In,
    FilterOperator::IsNull,
];

fn json_value() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        any::<i64>().prop_map(|n| serde_json::json!(n)),
        // u64 covers values above i64::MAX, which must bind without a lossy f64 cast.
        any::<u64>().prop_map(|n| serde_json::json!(n)),
        any::<f64>().prop_map(|f| serde_json::json!(f)),
        any::<bool>().prop_map(|b| serde_json::json!(b)),
        "[a-zA-Z0-9 %_!.-]{0,24}".prop_map(|s| serde_json::json!(s)),
        proptest::collection::vec(any::<i64>(), 0..6).prop_map(|v| serde_json::json!(v)),
        proptest::collection::vec("[a-z]{0,6}", 0..6).prop_map(|v| serde_json::json!(v)),
        proptest::collection::vec(any::<bool>(), 0..6).prop_map(|v| serde_json::json!(v)),
        Just(serde_json::Value::Null),
    ]
}

proptest! {
    /// `build_comparison_expr` never panics for any operator/value combination on
    /// either an integer or a string column, and is deterministic (the same input
    /// yields the same Some/None outcome). This is the joined-filter path, fed by
    /// attacker-controlled `filter={...}` JSON.
    #[test]
    fn build_comparison_expr_never_panics(value in json_value()) {
        for op in OPS {
            let a = build_comparison_expr(pe::Column::Id, op, &value).is_some();
            let b = build_comparison_expr(pe::Column::Id, op, &value).is_some();
            prop_assert_eq!(a, b);
            let c = build_comparison_expr(pe::Column::Name, op, &value).is_some();
            let d = build_comparison_expr(pe::Column::Name, op, &value).is_some();
            prop_assert_eq!(c, d);
        }
    }

    /// Attacker-controlled string filter values are always bound as parameters,
    /// never spliced into the SQL text. Proven by rendering the parameterised form
    /// and checking the value rides a placeholder.
    #[test]
    fn build_comparison_expr_binds_string_values(s in "[a-z][a-zA-Z0-9 ';-]{0,23}") {
        use sea_orm::sea_query::{Query, SqliteQueryBuilder};
        let expr = build_comparison_expr(pe::Column::Name, FilterOperator::Eq, &serde_json::json!(s));
        prop_assert!(expr.is_some());
        let (sql, values) = Query::select()
            .column(pe::Column::Id)
            .from(pe::Entity)
            .and_where(expr.unwrap())
            .build(SqliteQueryBuilder);
        prop_assert!(sql.contains('?'), "value must ride a bound placeholder: {sql}");
        prop_assert_eq!(values.0.len(), 1);
    }

    /// REST page/per_page pagination always stays within the configured caps and
    /// never panics, even for `u64::MAX` inputs (overflow-checks are on in tests).
    #[test]
    fn parse_pagination_page_respects_caps(page in any::<u64>(), per_page in any::<u64>()) {
        let params = crate::models::FilterOptions {
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);
        prop_assert!(limit <= MAX_PAGE_SIZE);
        prop_assert!(offset <= MAX_OFFSET);
    }

    /// React-Admin `range=[start,end]` pagination stays within caps and never
    /// panics, including reversed ranges and `u64::MAX` bounds.
    #[test]
    fn parse_pagination_range_respects_caps(start in any::<u64>(), end in any::<u64>()) {
        let params = crate::models::FilterOptions {
            range: Some(format!("[{start},{end}]")),
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);
        prop_assert!(limit <= MAX_PAGE_SIZE);
        prop_assert!(offset <= MAX_OFFSET);
    }
}
