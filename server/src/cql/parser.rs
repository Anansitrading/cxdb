// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! CQL Parser - Recursive descent parser for CQL queries.
//!
//! Grammar:
//!   query       = expression ;
//!   expression  = or_expr ;
//!   or_expr     = and_expr { "OR" and_expr } ;
//!   and_expr    = unary_expr { "AND" unary_expr } ;
//!   unary_expr  = [ "NOT" ] primary ;
//!   primary     = comparison | "(" expression ")" ;
//!   comparison  = field operator value ;

use super::ast::{
    CqlError, CqlErrorType, CqlQuery, Expression, FieldName, Operator, Position, Value,
};

/// Token types for the lexer.
#[derive(Debug, Clone, PartialEq)]
enum TokenType {
    And,
    Or,
    Not,
    In,
    LParen,
    RParen,
    Comma,
    Eq,
    Neq,
    Starts,
    EqCi,
    StartsCi,
    Gt,
    Gte,
    Lt,
    Lte,
    String(String),
    Number(f64),
    Ident(String),
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    token_type: TokenType,
    position: Position,
}

/// Lexer for CQL queries.
struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn current_position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
            offset: self.pos,
        }
    }

    fn advance(&mut self) -> Option<char> {
        if let Some((pos, ch)) = self.chars.next() {
            self.pos = pos + ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, ch)| *ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self, quote: char) -> Result<Token, CqlError> {
        let start_pos = self.current_position();
        self.advance(); // consume opening quote
        let mut value = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(CqlError {
                        error_type: CqlErrorType::SyntaxError,
                        message: format!(
                            "Unterminated string starting at line {}, column {}",
                            start_pos.line, start_pos.column
                        ),
                        position: Some(start_pos),
                        field: None,
                    });
                }
                Some(ch) if ch == quote => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.advance() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('r') => value.push('\r'),
                        Some('\\') => value.push('\\'),
                        Some('"') => value.push('"'),
                        Some('\'') => value.push('\''),
                        Some(ch) => value.push(ch),
                        None => {
                            return Err(CqlError {
                                error_type: CqlErrorType::SyntaxError,
                                message: "Unterminated escape sequence".into(),
                                position: Some(self.current_position()),
                                field: None,
                            });
                        }
                    }
                }
                Some(ch) => {
                    value.push(ch);
                    self.advance();
                }
            }
        }

        Ok(Token {
            token_type: TokenType::String(value),
            position: start_pos,
        })
    }

    fn read_number(&mut self) -> Token {
        let start_pos = self.current_position();
        let start = self.pos;

        // Handle negative
        if self.peek() == Some('-') {
            self.advance();
        }

        // Integer part
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Decimal part
        if self.peek() == Some('.') {
            self.advance();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let num_str = &self.input[start..self.pos];
        let value = num_str.parse::<f64>().unwrap_or(0.0);

        Token {
            token_type: TokenType::Number(value),
            position: start_pos,
        }
    }

    fn read_identifier(&mut self) -> Token {
        let start_pos = self.current_position();
        let start = self.pos;

        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let value = &self.input[start..self.pos];
        let token_type = match value.to_uppercase().as_str() {
            "AND" => TokenType::And,
            "OR" => TokenType::Or,
            "NOT" => TokenType::Not,
            "IN" => TokenType::In,
            _ => TokenType::Ident(value.to_string()),
        };

        Token {
            token_type,
            position: start_pos,
        }
    }

    fn next_token(&mut self) -> Result<Token, CqlError> {
        self.skip_whitespace();

        let start_pos = self.current_position();

        match self.peek() {
            None => Ok(Token {
                token_type: TokenType::Eof,
                position: start_pos,
            }),
            Some('"') | Some('\'') => {
                let quote = self.peek().unwrap();
                self.read_string(quote)
            }
            Some(ch)
                if ch.is_ascii_digit()
                    || (ch == '-'
                        && self.input[self.pos..].len() > 1
                        && self.input[self.pos + 1..]
                            .starts_with(|c: char| c.is_ascii_digit())) =>
            {
                Ok(self.read_number())
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' => Ok(self.read_identifier()),
            Some('(') => {
                self.advance();
                Ok(Token {
                    token_type: TokenType::LParen,
                    position: start_pos,
                })
            }
            Some(')') => {
                self.advance();
                Ok(Token {
                    token_type: TokenType::RParen,
                    position: start_pos,
                })
            }
            Some(',') => {
                self.advance();
                Ok(Token {
                    token_type: TokenType::Comma,
                    position: start_pos,
                })
            }
            Some('=') => {
                self.advance();
                Ok(Token {
                    token_type: TokenType::Eq,
                    position: start_pos,
                })
            }
            Some('!') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token {
                        token_type: TokenType::Neq,
                        position: start_pos,
                    })
                } else {
                    Err(CqlError {
                        error_type: CqlErrorType::SyntaxError,
                        message: format!(
                            "Expected '=' after '!' at line {}, column {}",
                            start_pos.line, start_pos.column
                        ),
                        position: Some(start_pos),
                        field: None,
                    })
                }
            }
            Some('^') => {
                self.advance();
                if self.peek() == Some('~') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Ok(Token {
                            token_type: TokenType::StartsCi,
                            position: start_pos,
                        })
                    } else {
                        Err(CqlError {
                            error_type: CqlErrorType::SyntaxError,
                            message: format!(
                                "Expected '=' after '^~' at line {}, column {}",
                                start_pos.line, start_pos.column
                            ),
                            position: Some(start_pos),
                            field: None,
                        })
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token {
                        token_type: TokenType::Starts,
                        position: start_pos,
                    })
                } else {
                    Err(CqlError {
                        error_type: CqlErrorType::SyntaxError,
                        message: format!(
                            "Expected '=' or '~=' after '^' at line {}, column {}",
                            start_pos.line, start_pos.column
                        ),
                        position: Some(start_pos),
                        field: None,
                    })
                }
            }
            Some('~') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token {
                        token_type: TokenType::EqCi,
                        position: start_pos,
                    })
                } else {
                    Err(CqlError {
                        error_type: CqlErrorType::SyntaxError,
                        message: format!(
                            "Expected '=' after '~' at line {}, column {}",
                            start_pos.line, start_pos.column
                        ),
                        position: Some(start_pos),
                        field: None,
                    })
                }
            }
            Some('>') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token {
                        token_type: TokenType::Gte,
                        position: start_pos,
                    })
                } else {
                    Ok(Token {
                        token_type: TokenType::Gt,
                        position: start_pos,
                    })
                }
            }
            Some('<') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token {
                        token_type: TokenType::Lte,
                        position: start_pos,
                    })
                } else {
                    Ok(Token {
                        token_type: TokenType::Lt,
                        position: start_pos,
                    })
                }
            }
            Some(ch) => Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: format!(
                    "Unexpected character '{}' at line {}, column {}",
                    ch, start_pos.line, start_pos.column
                ),
                position: Some(start_pos),
                field: None,
            }),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>, CqlError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.token_type, TokenType::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

/// Parser for CQL queries.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            pos: 0,
        }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token {
            token_type: TokenType::Eof,
            position: Position {
                line: 1,
                column: 1,
                offset: 0,
            },
        })
    }

    fn advance(&mut self) -> &Token {
        let token = self.current();
        if !matches!(token.token_type, TokenType::Eof) {
            self.pos += 1;
        }
        self.tokens.get(self.pos - 1).unwrap()
    }

    fn check(&self, expected: &TokenType) -> bool {
        std::mem::discriminant(&self.current().token_type) == std::mem::discriminant(expected)
    }

    fn match_token(&mut self, expected: &TokenType) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn parse(&mut self, input: &str) -> Result<CqlQuery, CqlError> {
        let mut lexer = Lexer::new(input);
        self.tokens = lexer.tokenize()?;
        self.pos = 0;

        if matches!(self.current().token_type, TokenType::Eof) {
            return Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: "Empty query".into(),
                position: Some(self.current().position),
                field: None,
            });
        }

        let ast = self.parse_or_expr()?;

        if !matches!(self.current().token_type, TokenType::Eof) {
            return Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: format!("Unexpected token after expression"),
                position: Some(self.current().position),
                field: None,
            });
        }

        Ok(CqlQuery {
            raw: input.to_string(),
            ast,
        })
    }

    fn parse_or_expr(&mut self) -> Result<Expression, CqlError> {
        let mut left = self.parse_and_expr()?;

        while self.match_token(&TokenType::Or) {
            let right = self.parse_and_expr()?;
            left = Expression::Or {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expression, CqlError> {
        let mut left = self.parse_unary_expr()?;

        while self.match_token(&TokenType::And) {
            let right = self.parse_unary_expr()?;
            left = Expression::And {
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expression, CqlError> {
        if self.match_token(&TokenType::Not) {
            let inner = self.parse_primary()?;
            return Ok(Expression::Not {
                inner: Box::new(inner),
            });
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, CqlError> {
        if self.match_token(&TokenType::LParen) {
            let expr = self.parse_or_expr()?;
            if !self.match_token(&TokenType::RParen) {
                return Err(CqlError {
                    error_type: CqlErrorType::SyntaxError,
                    message: "Expected ')' after expression".into(),
                    position: Some(self.current().position),
                    field: None,
                });
            }
            return Ok(expr);
        }

        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expression, CqlError> {
        // Field name
        let field_token = self.current().clone();
        let field_name = match &field_token.token_type {
            TokenType::Ident(name) => name.clone(),
            _ => {
                return Err(CqlError {
                    error_type: CqlErrorType::SyntaxError,
                    message: format!("Expected field name"),
                    position: Some(field_token.position),
                    field: None,
                });
            }
        };
        self.advance();

        // Validate field name
        if FieldName::from_str(&field_name).is_none() {
            let valid_fields: Vec<_> = FieldName::all().iter().map(|f| f.as_str()).collect();
            return Err(CqlError {
                error_type: CqlErrorType::UnknownField,
                message: format!(
                    "Unknown field '{}'. Valid fields: {}",
                    field_name,
                    valid_fields.join(", ")
                ),
                position: Some(field_token.position),
                field: Some(field_name),
            });
        }

        // Operator
        let op_token = self.current().clone();
        let operator = match &op_token.token_type {
            TokenType::Eq => Operator::Eq,
            TokenType::Neq => Operator::Neq,
            TokenType::Starts => Operator::Starts,
            TokenType::EqCi => Operator::EqCi,
            TokenType::StartsCi => Operator::StartsCi,
            TokenType::Gt => Operator::Gt,
            TokenType::Gte => Operator::Gte,
            TokenType::Lt => Operator::Lt,
            TokenType::Lte => Operator::Lte,
            TokenType::In => Operator::In,
            _ => {
                return Err(CqlError {
                    error_type: CqlErrorType::SyntaxError,
                    message: "Expected operator".into(),
                    position: Some(op_token.position),
                    field: None,
                });
            }
        };
        self.advance();

        // Value
        let value = if operator == Operator::In {
            self.parse_list()?
        } else {
            self.parse_value()?
        };

        Ok(Expression::Comparison {
            field: field_name,
            operator,
            value,
        })
    }

    fn parse_value(&mut self) -> Result<Value, CqlError> {
        let token = self.current().clone();
        match &token.token_type {
            TokenType::String(s) => {
                self.advance();
                // Check if it's a relative date
                let relative_pattern = regex::Regex::new(r"^-(\d+)([hdm])$").unwrap();
                if relative_pattern.is_match(s) {
                    Ok(Value::Date {
                        value: s.clone(),
                        relative: true,
                    })
                } else {
                    Ok(Value::String { value: s.clone() })
                }
            }
            TokenType::Number(n) => {
                self.advance();
                Ok(Value::Number { value: *n })
            }
            TokenType::Ident(s) if s.to_lowercase() == "true" || s.to_lowercase() == "false" => {
                self.advance();
                Ok(Value::String {
                    value: s.to_lowercase(),
                })
            }
            _ => Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: "Expected value".into(),
                position: Some(token.position),
                field: None,
            }),
        }
    }

    fn parse_list(&mut self) -> Result<Value, CqlError> {
        if !self.match_token(&TokenType::LParen) {
            return Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: "Expected '(' after IN".into(),
                position: Some(self.current().position),
                field: None,
            });
        }

        let mut values = Vec::new();

        // First value
        values.push(self.parse_value()?);

        // Additional values
        while self.match_token(&TokenType::Comma) {
            values.push(self.parse_value()?);
        }

        if !self.match_token(&TokenType::RParen) {
            return Err(CqlError {
                error_type: CqlErrorType::SyntaxError,
                message: "Expected ')' after list values".into(),
                position: Some(self.current().position),
                field: None,
            });
        }

        Ok(Value::List { values })
    }
}

/// Parse a CQL query string into an AST.
pub fn parse(input: &str) -> Result<CqlQuery, CqlError> {
    let mut parser = Parser::new();
    parser.parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_eq() {
        let result = parse(r#"tag = "amplifier""#).unwrap();
        assert_eq!(result.raw, r#"tag = "amplifier""#);
        match result.ast {
            Expression::Comparison {
                field, operator, ..
            } => {
                assert_eq!(field, "tag");
                assert_eq!(operator, Operator::Eq);
            }
            _ => panic!("Expected comparison"),
        }
    }

    #[test]
    fn test_and_expr() {
        let result = parse(r#"tag = "amplifier" AND user = "jay""#).unwrap();
        match result.ast {
            Expression::And { .. } => {}
            _ => panic!("Expected AND expression"),
        }
    }

    #[test]
    fn test_or_expr() {
        let result = parse(r#"tag = "a" OR tag = "b""#).unwrap();
        match result.ast {
            Expression::Or { .. } => {}
            _ => panic!("Expected OR expression"),
        }
    }

    #[test]
    fn test_not_expr() {
        let result = parse(r#"NOT tag = "test""#).unwrap();
        match result.ast {
            Expression::Not { .. } => {}
            _ => panic!("Expected NOT expression"),
        }
    }

    #[test]
    fn test_in_operator() {
        let result = parse(r#"tag IN ("a", "b", "c")"#).unwrap();
        match result.ast {
            Expression::Comparison {
                operator, value, ..
            } => {
                assert_eq!(operator, Operator::In);
                match value {
                    Value::List { values } => assert_eq!(values.len(), 3),
                    _ => panic!("Expected list value"),
                }
            }
            _ => panic!("Expected comparison"),
        }
    }

    #[test]
    fn test_parentheses() {
        let result = parse(r#"(tag = "a" OR tag = "b") AND user = "jay""#).unwrap();
        match result.ast {
            Expression::And { left, .. } => match *left {
                Expression::Or { .. } => {}
                _ => panic!("Expected OR in left side"),
            },
            _ => panic!("Expected AND expression"),
        }
    }

    #[test]
    fn test_unknown_field() {
        let result = parse(r#"unknown = "value""#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.error_type, CqlErrorType::UnknownField));
    }

    #[test]
    fn test_relative_date() {
        let result = parse(r#"created > "-24h""#).unwrap();
        match result.ast {
            Expression::Comparison { value, .. } => match value {
                Value::Date { relative, .. } => assert!(relative),
                _ => panic!("Expected date value"),
            },
            _ => panic!("Expected comparison"),
        }
    }
}
