// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! CQL Integration Tests
//!
//! End-to-end tests covering the CQL parser, indexes, and executor.

use cxdb_server::cql::{execute, parse, Expression, Operator, SecondaryIndexes, Value};
use cxdb_server::store::{ContextMetadata, Provenance};
use std::collections::HashSet;

// Helper to create test indexes with sample data
fn create_test_indexes() -> SecondaryIndexes {
    let mut indexes = SecondaryIndexes::new();

    // Context 1: tag=amplifier, user=jay, service=dotrunner
    let meta1 = ContextMetadata {
        client_tag: Some("amplifier".to_string()),
        title: Some("Test context 1".to_string()),
        labels: Some(vec![]),
        provenance: Some(Provenance {
            on_behalf_of: Some("jay".to_string()),
            service_name: Some("dotrunner".to_string()),
            ..Default::default()
        }),
    };
    indexes.add_context(1, Some(&meta1), 1000, 5);

    // Context 2: tag=amplifier, user=alex, service=gen
    let meta2 = ContextMetadata {
        client_tag: Some("amplifier".to_string()),
        title: Some("Test context 2".to_string()),
        labels: Some(vec![]),
        provenance: Some(Provenance {
            on_behalf_of: Some("alex".to_string()),
            service_name: Some("gen".to_string()),
            ..Default::default()
        }),
    };
    indexes.add_context(2, Some(&meta2), 2000, 3);

    // Context 3: tag=test, user=jay, service=dotrunner
    let meta3 = ContextMetadata {
        client_tag: Some("test".to_string()),
        title: Some("Test context 3".to_string()),
        labels: Some(vec![]),
        provenance: Some(Provenance {
            on_behalf_of: Some("jay".to_string()),
            service_name: Some("dotrunner".to_string()),
            ..Default::default()
        }),
    };
    indexes.add_context(3, Some(&meta3), 3000, 10);

    // Context 4: tag=core, user=sam, service=generator
    let meta4 = ContextMetadata {
        client_tag: Some("core".to_string()),
        title: Some("Test context 4".to_string()),
        labels: Some(vec![]),
        provenance: Some(Provenance {
            on_behalf_of: Some("sam".to_string()),
            service_name: Some("generator".to_string()),
            ..Default::default()
        }),
    };
    indexes.add_context(4, Some(&meta4), 4000, 2);

    // Context 5: tag=amplifier-core, user=jay, service=dot-test
    let meta5 = ContextMetadata {
        client_tag: Some("amplifier-core".to_string()),
        title: Some("Test context 5".to_string()),
        labels: Some(vec![]),
        provenance: Some(Provenance {
            on_behalf_of: Some("jay".to_string()),
            service_name: Some("dot-test".to_string()),
            ..Default::default()
        }),
    };
    indexes.add_context(5, Some(&meta5), 5000, 7);

    indexes
}

// ============================================================================
// Parser Tests
// ============================================================================

#[test]
fn test_parse_simple_equality() {
    let query = parse(r#"tag = "amplifier""#).expect("should parse");
    assert_eq!(query.raw, r#"tag = "amplifier""#);

    match &query.ast {
        Expression::Comparison {
            field,
            operator,
            value,
        } => {
            assert_eq!(field, "tag");
            assert!(matches!(operator, Operator::Eq));
            match value {
                Value::String { value: v } => assert_eq!(v, "amplifier"),
                _ => panic!("expected String value"),
            }
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn test_parse_and_expression() {
    let query = parse(r#"tag = "amplifier" AND user = "jay""#).expect("should parse");

    match &query.ast {
        Expression::And { left, right } => {
            assert!(matches!(left.as_ref(), Expression::Comparison { .. }));
            assert!(matches!(right.as_ref(), Expression::Comparison { .. }));
        }
        _ => panic!("expected And expression"),
    }
}

#[test]
fn test_parse_or_expression() {
    let query = parse(r#"service = "dotrunner" OR service = "gen""#).expect("should parse");

    match &query.ast {
        Expression::Or { left, right } => {
            assert!(matches!(left.as_ref(), Expression::Comparison { .. }));
            assert!(matches!(right.as_ref(), Expression::Comparison { .. }));
        }
        _ => panic!("expected Or expression"),
    }
}

#[test]
fn test_parse_not_expression() {
    let query = parse(r#"NOT tag = "test""#).expect("should parse");

    match &query.ast {
        Expression::Not { inner } => {
            assert!(matches!(inner.as_ref(), Expression::Comparison { .. }));
        }
        _ => panic!("expected Not expression"),
    }
}

#[test]
fn test_parse_parentheses() {
    let query = parse(r#"(service = "dotrunner" OR service = "gen") AND tag = "amplifier""#)
        .expect("should parse");

    match &query.ast {
        Expression::And { left, right } => {
            assert!(matches!(left.as_ref(), Expression::Or { .. }));
            assert!(matches!(right.as_ref(), Expression::Comparison { .. }));
        }
        _ => panic!("expected And with Or on left"),
    }
}

#[test]
fn test_parse_prefix_operator() {
    let query = parse(r#"tag ^= "amp""#).expect("should parse");

    match &query.ast {
        Expression::Comparison { operator, .. } => {
            assert!(matches!(operator, Operator::Starts));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn test_parse_case_insensitive_operator() {
    let query = parse(r#"user ~= "Jay""#).expect("should parse");

    match &query.ast {
        Expression::Comparison { operator, .. } => {
            assert!(matches!(operator, Operator::EqCi));
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn test_parse_in_operator() {
    let query = parse(r#"tag IN ("a", "b", "c")"#).expect("should parse");

    match &query.ast {
        Expression::Comparison {
            operator, value, ..
        } => {
            assert!(matches!(operator, Operator::In));
            match value {
                Value::List { values } => assert_eq!(values.len(), 3),
                _ => panic!("expected List value"),
            }
        }
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn test_parse_numeric_value() {
    let query = parse("id = 12345").expect("should parse");

    match &query.ast {
        Expression::Comparison { value, .. } => match value {
            Value::Number { value: n } => assert_eq!(*n as i64, 12345),
            _ => panic!("expected Number value"),
        },
        _ => panic!("expected Comparison"),
    }
}

#[test]
fn test_parse_error_missing_value() {
    let result = parse("tag = ");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_unknown_field() {
    let result = parse(r#"unknown_field = "value""#);
    assert!(result.is_err());
}

#[test]
fn test_parse_error_unclosed_paren() {
    let result = parse(r#"(tag = "test""#);
    assert!(result.is_err());
}

// ============================================================================
// Executor Tests
// ============================================================================

#[test]
fn test_execute_exact_match() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"tag = "amplifier""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.contains(&1));
    assert!(result.contains(&2));
}

#[test]
fn test_execute_and_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"tag = "amplifier" AND user = "jay""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert_eq!(result.len(), 1);
    assert!(result.contains(&1));
}

#[test]
fn test_execute_or_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"service = "dotrunner" OR service = "gen""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(result.contains(&3));
}

#[test]
fn test_execute_not_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    // NOT tag = "test" should return all contexts except context 3
    let query = parse(r#"NOT tag = "test""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert!(!result.contains(&3));
    // Should contain contexts 1, 2, 4, 5
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(result.contains(&4));
    assert!(result.contains(&5));
}

#[test]
fn test_execute_prefix_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"tag ^= "amp""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    // Should match "amplifier" (1, 2) and "amplifier-core" (5)
    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(result.contains(&5));
}

#[test]
fn test_execute_case_insensitive_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"user ~= "JAY""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    // Should match jay (contexts 1, 3, 5)
    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&3));
    assert!(result.contains(&5));
}

#[test]
fn test_execute_in_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"tag IN ("amplifier", "core")"#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    // Should match amplifier (1, 2) and core (4)
    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&2));
    assert!(result.contains(&4));
}

#[test]
fn test_execute_complex_query() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"(tag = "amplifier" OR tag = "core") AND user = "jay""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    // Only context 1 matches: tag=amplifier AND user=jay
    assert_eq!(result.len(), 1);
    assert!(result.contains(&1));
}

#[test]
fn test_execute_empty_result() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    let query = parse(r#"tag = "nonexistent""#).unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert!(result.is_empty());
}

#[test]
fn test_execute_is_live() {
    let indexes = create_test_indexes();
    let mut live_contexts = HashSet::new();
    live_contexts.insert(1u64);
    live_contexts.insert(3u64);

    let query = parse("is_live = true").unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.contains(&1));
    assert!(result.contains(&3));
}

#[test]
fn test_execute_depth_range() {
    let indexes = create_test_indexes();
    let live_contexts = HashSet::new();

    // Context depths: 1=5, 2=3, 3=10, 4=2, 5=7
    let query = parse("depth >= 5").unwrap();
    let result = execute(&query.ast, &indexes, &live_contexts).unwrap();

    // Should match contexts 1 (5), 3 (10), 5 (7)
    assert_eq!(result.len(), 3);
    assert!(result.contains(&1));
    assert!(result.contains(&3));
    assert!(result.contains(&5));
}

// ============================================================================
// Index Tests
// ============================================================================

#[test]
fn test_index_exact_lookup() {
    let indexes = create_test_indexes();

    let results = indexes.lookup_tag_exact("amplifier");
    assert_eq!(results.len(), 2);
    assert!(results.contains(&1));
    assert!(results.contains(&2));
}

#[test]
fn test_index_prefix_lookup() {
    let indexes = create_test_indexes();

    let results = indexes.lookup_service_prefix("dot");
    // Should match "dotrunner" (1, 3) and "dot-test" (5)
    assert_eq!(results.len(), 3);
}

#[test]
fn test_index_case_insensitive_lookup() {
    let indexes = create_test_indexes();

    let results = indexes.lookup_user_exact_ci("JAY");
    assert_eq!(results.len(), 3);
}

#[test]
fn test_index_all_context_ids() {
    let indexes = create_test_indexes();

    let all = indexes.all_contexts();
    assert_eq!(all.len(), 5);
}
