//! Join filtering and sorting support structures.
//!
//! This module provides types for filtering and sorting on related entity columns
//! via dot-notation syntax (e.g., `vehicles.year`, `vehicles.make`).

use sea_orm::Condition;

/// Describes a filterable or sortable column on a joined/related entity.
///
/// This is used by the generated `CRUDResource` implementations to expose
/// which related columns are available for filtering/sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedColumnDef {
    /// The join field name on the parent entity (e.g., "vehicles")
    pub join_field: &'static str,
    /// The column name on the related entity (e.g., "year", "make")
    pub column_name: &'static str,
    /// Full dot-notation path (e.g., "vehicles.year")
    pub full_path: &'static str,
}

/// A filter condition on a joined entity column.
#[derive(Debug, Clone)]
pub struct JoinedFilter {
    /// The join field name (e.g., "vehicles")
    pub join_field: String,
    /// The column name on the related entity (e.g., "make")
    pub column: String,
    /// The comparison operator
    pub operator: FilterOperator,
    /// The filter value
    pub value: serde_json::Value,
}

/// Comparison operators for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    /// Equality (=)
    Eq,
    /// Not equal (!=)
    Neq,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Gte,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Lte,
    /// LIKE pattern matching
    Like,
    /// IN (array of values)
    In,
    /// IS NULL
    IsNull,
}

impl FilterOperator {
    /// Parse operator from field name suffix (e.g., "_gte", "_lte")
    pub fn from_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "_gte" => Some(Self::Gte),
            "_lte" => Some(Self::Lte),
            "_gt" => Some(Self::Gt),
            "_lt" => Some(Self::Lt),
            "_neq" => Some(Self::Neq),
            "_like" => Some(Self::Like),
            _ => None,
        }
    }

    /// Get the suffix for this operator
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::Eq => "",
            Self::Neq => "_neq",
            Self::Gt => "_gt",
            Self::Gte => "_gte",
            Self::Lt => "_lt",
            Self::Lte => "_lte",
            Self::Like => "_like",
            Self::In => "",
            Self::IsNull => "",
        }
    }
}

/// Result of parsing filters - contains both main entity conditions and joined filters.
#[derive(Debug)]
pub struct ParsedFilters {
    /// Condition for the main entity (non-join filters)
    pub main_condition: Condition,
    /// Filters on joined entity columns, grouped by join field
    pub joined_filters: Vec<JoinedFilter>,
    /// Whether any joined filters were found
    pub has_joined_filters: bool,
}

impl Default for ParsedFilters {
    fn default() -> Self {
        Self {
            main_condition: Condition::all(),
            joined_filters: Vec::new(),
            has_joined_filters: false,
        }
    }
}

/// Sort configuration that may reference a joined column.
#[derive(Debug, Clone)]
pub enum SortConfig<C> {
    /// Sort by a column on the main entity
    Column {
        column: C,
        direction: sea_orm::Order,
    },
    /// Sort by a column on a joined entity
    Joined {
        /// Join field name (e.g., "vehicles")
        join_field: String,
        /// Column name on the joined entity (e.g., "year")
        column: String,
        /// Sort direction
        direction: sea_orm::Order,
    },
}

impl<C> SortConfig<C> {
    /// Check if this sort is on a joined column
    pub fn is_joined(&self) -> bool {
        matches!(self, Self::Joined { .. })
    }

    /// Get the sort direction
    pub fn direction(&self) -> sea_orm::Order {
        match self {
            Self::Column { direction, .. } | Self::Joined { direction, .. } => direction.clone(),
        }
    }
}

/// Parse a dot-notation field path into (join_field, column, operator).
///
/// Examples:
/// - "vehicles.make" -> Some(("vehicles", "make", Eq))
/// - "vehicles.year_gte" -> Some(("vehicles", "year", Gte))
/// - "name" -> None (not a join field)
pub fn parse_dot_notation(field: &str) -> Option<(String, String, FilterOperator)> {
    let dot_pos = field.find('.')?;
    let join_field = &field[..dot_pos];
    let rest = &field[dot_pos + 1..];

    // Check for operator suffix
    for suffix in ["_gte", "_lte", "_gt", "_lt", "_neq", "_like"] {
        if let Some(column) = rest.strip_suffix(suffix) {
            let op = FilterOperator::from_suffix(suffix).unwrap_or(FilterOperator::Eq);
            return Some((join_field.to_string(), column.to_string(), op));
        }
    }

    // No suffix - equals operator
    Some((join_field.to_string(), rest.to_string(), FilterOperator::Eq))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_notation_simple() {
        let result = parse_dot_notation("vehicles.make");
        assert_eq!(result, Some(("vehicles".to_string(), "make".to_string(), FilterOperator::Eq)));
    }

    #[test]
    fn test_parse_dot_notation_with_operator() {
        let result = parse_dot_notation("vehicles.year_gte");
        assert_eq!(result, Some(("vehicles".to_string(), "year".to_string(), FilterOperator::Gte)));

        let result = parse_dot_notation("vehicles.year_lte");
        assert_eq!(result, Some(("vehicles".to_string(), "year".to_string(), FilterOperator::Lte)));
    }

    #[test]
    fn test_parse_dot_notation_no_dot() {
        let result = parse_dot_notation("name");
        assert_eq!(result, None);
    }

    #[test]
    fn test_joined_column_def() {
        let def = JoinedColumnDef {
            join_field: "vehicles",
            column_name: "year",
            full_path: "vehicles.year",
        };
        assert_eq!(def.join_field, "vehicles");
        assert_eq!(def.full_path, "vehicles.year");
    }
}
