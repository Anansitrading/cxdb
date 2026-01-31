// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! CQL Abstract Syntax Tree types.
//!
//! These types mirror the frontend TypeScript definitions for JSON serialization.

use serde::{Deserialize, Serialize};

/// A parsed CQL query with the original string and AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CqlQuery {
    pub raw: String,
    pub ast: Expression,
}

/// Expression node in the CQL AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Expression {
    And {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Or {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Not {
        inner: Box<Expression>,
    },
    Comparison {
        field: String,
        operator: Operator,
        value: Value,
    },
}

/// Comparison operators supported by CQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,       // =
    Neq,      // !=
    Starts,   // ^=
    EqCi,     // ~=
    StartsCi, // ^~=
    Gt,       // >
    Gte,      // >=
    Lt,       // <
    Lte,      // <=
    In,       // IN
}

/// Value types in CQL expressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Value {
    String { value: String },
    Number { value: f64 },
    Date { value: String, relative: bool },
    List { values: Vec<Value> },
}

impl Value {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String { value } => Some(value),
            Value::Date { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number { value } => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_number().map(|n| n as u64)
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List { values } => Some(values),
            _ => None,
        }
    }
}

/// Valid CQL field names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldName {
    Id,
    Tag,
    Title,
    Label,
    User,
    Service,
    Host,
    TraceId,
    Parent,
    Root,
    Created,
    Depth,
    IsLive,
}

impl FieldName {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "id" => Some(Self::Id),
            "tag" => Some(Self::Tag),
            "title" => Some(Self::Title),
            "label" => Some(Self::Label),
            "user" => Some(Self::User),
            "service" => Some(Self::Service),
            "host" => Some(Self::Host),
            "trace_id" => Some(Self::TraceId),
            "parent" => Some(Self::Parent),
            "root" => Some(Self::Root),
            "created" => Some(Self::Created),
            "depth" => Some(Self::Depth),
            "is_live" => Some(Self::IsLive),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Tag => "tag",
            Self::Title => "title",
            Self::Label => "label",
            Self::User => "user",
            Self::Service => "service",
            Self::Host => "host",
            Self::TraceId => "trace_id",
            Self::Parent => "parent",
            Self::Root => "root",
            Self::Created => "created",
            Self::Depth => "depth",
            Self::IsLive => "is_live",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Id,
            Self::Tag,
            Self::Title,
            Self::Label,
            Self::User,
            Self::Service,
            Self::Host,
            Self::TraceId,
            Self::Parent,
            Self::Root,
            Self::Created,
            Self::Depth,
            Self::IsLive,
        ]
    }
}

/// CQL parsing/execution error.
#[derive(Debug, Clone, Serialize)]
pub struct CqlError {
    #[serde(rename = "type")]
    pub error_type: CqlErrorType,
    pub message: String,
    pub position: Option<Position>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CqlErrorType {
    SyntaxError,
    UnknownField,
    InvalidOperator,
    InvalidValue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl std::fmt::Display for CqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(pos) = &self.position {
            write!(
                f,
                "{} (line {}, column {})",
                self.message, pos.line, pos.column
            )
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for CqlError {}
