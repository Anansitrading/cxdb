// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! CQL Query Executor - Evaluates CQL AST against secondary indexes.

use std::collections::HashSet;

use super::ast::{CqlError, CqlErrorType, Expression, FieldName, Operator, Value};
use super::indexes::SecondaryIndexes;

/// Execute a CQL expression against the secondary indexes.
pub fn execute(
    expr: &Expression,
    indexes: &SecondaryIndexes,
    live_contexts: &HashSet<u64>,
) -> Result<HashSet<u64>, CqlError> {
    match expr {
        Expression::And { left, right } => {
            let left_result = execute(left, indexes, live_contexts)?;
            let right_result = execute(right, indexes, live_contexts)?;
            Ok(left_result.intersection(&right_result).copied().collect())
        }
        Expression::Or { left, right } => {
            let left_result = execute(left, indexes, live_contexts)?;
            let right_result = execute(right, indexes, live_contexts)?;
            Ok(left_result.union(&right_result).copied().collect())
        }
        Expression::Not { inner } => {
            let inner_result = execute(inner, indexes, live_contexts)?;
            Ok(indexes
                .all_contexts()
                .difference(&inner_result)
                .copied()
                .collect())
        }
        Expression::Comparison {
            field,
            operator,
            value,
        } => execute_comparison(field, *operator, value, indexes, live_contexts),
    }
}

fn execute_comparison(
    field: &str,
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
    live_contexts: &HashSet<u64>,
) -> Result<HashSet<u64>, CqlError> {
    let field_name = FieldName::from_str(field).ok_or_else(|| CqlError {
        error_type: CqlErrorType::UnknownField,
        message: format!("Unknown field: {}", field),
        position: None,
        field: Some(field.to_string()),
    })?;

    match field_name {
        FieldName::Id => execute_id(operator, value, indexes),
        FieldName::Tag => execute_string_field(operator, value, indexes, StringField::Tag),
        FieldName::Title => execute_string_field(operator, value, indexes, StringField::Title),
        FieldName::Label => execute_label(operator, value, indexes),
        FieldName::User => execute_string_field(operator, value, indexes, StringField::User),
        FieldName::Service => execute_string_field(operator, value, indexes, StringField::Service),
        FieldName::Host => execute_string_field(operator, value, indexes, StringField::Host),
        FieldName::TraceId => execute_trace_id(operator, value, indexes),
        FieldName::Parent => execute_parent(operator, value, indexes),
        FieldName::Root => execute_root(operator, value, indexes),
        FieldName::Created => execute_created(operator, value, indexes),
        FieldName::Depth => execute_depth(operator, value, indexes),
        FieldName::IsLive => execute_is_live(operator, value, live_contexts, indexes),
    }
}

enum StringField {
    Tag,
    Title,
    User,
    Service,
    Host,
}

fn execute_string_field(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
    field: StringField,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(match field {
                StringField::Tag => indexes.lookup_tag_exact(s),
                StringField::Title => indexes.lookup_title_exact(s),
                StringField::User => indexes.lookup_user_exact(s),
                StringField::Service => indexes.lookup_service_exact(s),
                StringField::Host => indexes.lookup_host_exact(s),
            })
        }
        Operator::EqCi => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(match field {
                StringField::Tag => indexes.lookup_tag_exact_ci(s),
                StringField::Title => indexes.lookup_title_exact_ci(s),
                StringField::User => indexes.lookup_user_exact_ci(s),
                StringField::Service => indexes.lookup_service_exact_ci(s),
                StringField::Host => indexes.lookup_host_exact(s), // Host doesn't have CI index
            })
        }
        Operator::Starts => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(match field {
                StringField::Tag => indexes.lookup_tag_prefix(s),
                StringField::Title => indexes.lookup_title_prefix(s),
                StringField::User => indexes.lookup_user_prefix(s),
                StringField::Service => indexes.lookup_service_prefix(s),
                StringField::Host => indexes.lookup_host_prefix(s),
            })
        }
        Operator::StartsCi => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(match field {
                StringField::Tag => indexes.lookup_tag_prefix_ci(s),
                StringField::Title => indexes.lookup_title_prefix_ci(s),
                StringField::User => indexes.lookup_user_prefix_ci(s),
                StringField::Service => indexes.lookup_service_prefix_ci(s),
                StringField::Host => indexes.lookup_host_prefix(s), // Host doesn't have CI index
            })
        }
        Operator::Neq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            let matches = match field {
                StringField::Tag => indexes.lookup_tag_exact(s),
                StringField::Title => indexes.lookup_title_exact(s),
                StringField::User => indexes.lookup_user_exact(s),
                StringField::Service => indexes.lookup_service_exact(s),
                StringField::Host => indexes.lookup_host_exact(s),
            };
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::In => {
            let list = value.as_list().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected list value for IN operator".into(),
                position: None,
                field: None,
            })?;
            let mut result = HashSet::new();
            for v in list {
                if let Some(s) = v.as_string() {
                    let matches = match field {
                        StringField::Tag => indexes.lookup_tag_exact(s),
                        StringField::Title => indexes.lookup_title_exact(s),
                        StringField::User => indexes.lookup_user_exact(s),
                        StringField::Service => indexes.lookup_service_exact(s),
                        StringField::Host => indexes.lookup_host_exact(s),
                    };
                    result.extend(matches);
                }
            }
            Ok(result)
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for string fields", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_id(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for id".into(),
                position: None,
                field: None,
            })?;
            if indexes.all_contexts().contains(&id) {
                Ok(HashSet::from([id]))
            } else {
                Ok(HashSet::new())
            }
        }
        Operator::Neq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for id".into(),
                position: None,
                field: None,
            })?;
            let mut result = indexes.all_contexts().clone();
            result.remove(&id);
            Ok(result)
        }
        Operator::In => {
            let list = value.as_list().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected list value for IN operator".into(),
                position: None,
                field: None,
            })?;
            let mut result = HashSet::new();
            for v in list {
                if let Some(id) = v.as_u64() {
                    if indexes.all_contexts().contains(&id) {
                        result.insert(id);
                    }
                }
            }
            Ok(result)
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for id field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_label(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(indexes.lookup_label_exact(s))
        }
        Operator::Neq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            let matches = indexes.lookup_label_exact(s);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::In => {
            let list = value.as_list().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected list value for IN operator".into(),
                position: None,
                field: None,
            })?;
            let mut result = HashSet::new();
            for v in list {
                if let Some(s) = v.as_string() {
                    result.extend(indexes.lookup_label_exact(s));
                }
            }
            Ok(result)
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for label field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_trace_id(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            Ok(indexes.lookup_trace_id_exact(s))
        }
        Operator::Neq => {
            let s = value.as_string().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected string value".into(),
                position: None,
                field: None,
            })?;
            let matches = indexes.lookup_trace_id_exact(s);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for trace_id field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_parent(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for parent".into(),
                position: None,
                field: None,
            })?;
            Ok(indexes.lookup_parent_exact(id))
        }
        Operator::Neq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for parent".into(),
                position: None,
                field: None,
            })?;
            let matches = indexes.lookup_parent_exact(id);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::In => {
            let list = value.as_list().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected list value for IN operator".into(),
                position: None,
                field: None,
            })?;
            let mut result = HashSet::new();
            for v in list {
                if let Some(id) = v.as_u64() {
                    result.extend(indexes.lookup_parent_exact(id));
                }
            }
            Ok(result)
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for parent field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_root(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    match operator {
        Operator::Eq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for root".into(),
                position: None,
                field: None,
            })?;
            Ok(indexes.lookup_root_exact(id))
        }
        Operator::Neq => {
            let id = value.as_u64().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected numeric value for root".into(),
                position: None,
                field: None,
            })?;
            let matches = indexes.lookup_root_exact(id);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::In => {
            let list = value.as_list().ok_or_else(|| CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected list value for IN operator".into(),
                position: None,
                field: None,
            })?;
            let mut result = HashSet::new();
            for v in list {
                if let Some(id) = v.as_u64() {
                    result.extend(indexes.lookup_root_exact(id));
                }
            }
            Ok(result)
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for root field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_created(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    let timestamp = parse_date_value(value)?;

    match operator {
        Operator::Eq => Ok(indexes.lookup_created_eq(timestamp)),
        Operator::Neq => {
            let matches = indexes.lookup_created_eq(timestamp);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::Gt => Ok(indexes.lookup_created_gt(timestamp)),
        Operator::Gte => Ok(indexes.lookup_created_gte(timestamp)),
        Operator::Lt => Ok(indexes.lookup_created_lt(timestamp)),
        Operator::Lte => Ok(indexes.lookup_created_lte(timestamp)),
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for created field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_depth(
    operator: Operator,
    value: &Value,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    let depth = value.as_u64().ok_or_else(|| CqlError {
        error_type: CqlErrorType::InvalidValue,
        message: "Expected numeric value for depth".into(),
        position: None,
        field: None,
    })? as u32;

    match operator {
        Operator::Eq => Ok(indexes.lookup_depth_eq(depth)),
        Operator::Neq => {
            let matches = indexes.lookup_depth_eq(depth);
            Ok(indexes
                .all_contexts()
                .difference(&matches)
                .copied()
                .collect())
        }
        Operator::Gt => Ok(indexes.lookup_depth_gt(depth)),
        Operator::Gte => Ok(indexes.lookup_depth_gte(depth)),
        Operator::Lt => Ok(indexes.lookup_depth_lt(depth)),
        Operator::Lte => Ok(indexes.lookup_depth_lte(depth)),
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for depth field", operator),
            position: None,
            field: None,
        }),
    }
}

fn execute_is_live(
    operator: Operator,
    value: &Value,
    live_contexts: &HashSet<u64>,
    indexes: &SecondaryIndexes,
) -> Result<HashSet<u64>, CqlError> {
    let is_live = match value {
        Value::String { value } => value == "true",
        _ => {
            return Err(CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: "Expected boolean value for is_live".into(),
                position: None,
                field: None,
            });
        }
    };

    match operator {
        Operator::Eq => {
            if is_live {
                Ok(live_contexts.clone())
            } else {
                Ok(indexes
                    .all_contexts()
                    .difference(live_contexts)
                    .copied()
                    .collect())
            }
        }
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidOperator,
            message: format!("Operator {:?} not supported for is_live field", operator),
            position: None,
            field: None,
        }),
    }
}

/// Parse a date value (relative or absolute) into a Unix timestamp in milliseconds.
fn parse_date_value(value: &Value) -> Result<u64, CqlError> {
    match value {
        Value::Date { value, relative } => {
            if *relative {
                parse_relative_date(value)
            } else {
                parse_absolute_date(value)
            }
        }
        Value::String { value } => {
            // Try relative first, then absolute
            if let Ok(ts) = parse_relative_date(value) {
                Ok(ts)
            } else {
                parse_absolute_date(value)
            }
        }
        Value::Number { value } => Ok(*value as u64),
        _ => Err(CqlError {
            error_type: CqlErrorType::InvalidValue,
            message: "Expected date value".into(),
            position: None,
            field: None,
        }),
    }
}

fn parse_relative_date(value: &str) -> Result<u64, CqlError> {
    let re = regex::Regex::new(r"^-(\d+)([hdm])$").unwrap();
    let caps = re.captures(value).ok_or_else(|| CqlError {
        error_type: CqlErrorType::InvalidValue,
        message: format!("Invalid relative date format: {}", value),
        position: None,
        field: None,
    })?;

    let amount: u64 = caps[1].parse().map_err(|_| CqlError {
        error_type: CqlErrorType::InvalidValue,
        message: format!("Invalid number in relative date: {}", value),
        position: None,
        field: None,
    })?;

    let unit = &caps[2];
    let millis = match unit {
        "h" => amount * 60 * 60 * 1000,
        "d" => amount * 24 * 60 * 60 * 1000,
        "m" => amount * 60 * 1000,
        _ => {
            return Err(CqlError {
                error_type: CqlErrorType::InvalidValue,
                message: format!("Invalid time unit: {}", unit),
                position: None,
                field: None,
            });
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    Ok(now - millis)
}

fn parse_absolute_date(value: &str) -> Result<u64, CqlError> {
    // Try ISO-8601 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(dt.timestamp_millis() as u64);
    }

    // Try date-only format (YYYY-MM-DD)
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)
                .timestamp_millis() as u64,
        );
    }

    Err(CqlError {
        error_type: CqlErrorType::InvalidValue,
        message: format!("Invalid date format: {}", value),
        position: None,
        field: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_date() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let result = parse_relative_date("-24h").unwrap();
        let expected = now - (24 * 60 * 60 * 1000);
        // Allow 1 second tolerance
        assert!((result as i64 - expected as i64).abs() < 1000);
    }

    #[test]
    fn test_parse_absolute_date() {
        let result = parse_absolute_date("2024-01-15T00:00:00Z").unwrap();
        assert_eq!(result, 1705276800000);
    }
}
