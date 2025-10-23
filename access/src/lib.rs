/*
  Copyright 2025 Will Murnane

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
*/

use std::fmt::Display;

use chumsky::Parser;

use crate::{expression::access_expression, parser::unquoted_token_parser, tokens::access_tokens};

mod expression;
mod parser;
mod tokens;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AccessToken {
    Unquoted(String),
    Quoted(String),
}

impl AccessToken {
    fn new(value: &str) -> AccessToken {
        if unquoted_token_parser().parse(value).has_output() {
            AccessToken::Unquoted(value.to_string())
        } else {
            AccessToken::Quoted(value.to_string())
        }
    }
    fn emit(&self) -> String {
        match self {
            AccessToken::Quoted(s) => {
                let mut result = String::new();
                result.push('"');
                result.push_str(&s);
                result.push('"');
                result
            }
            AccessToken::Unquoted(s) => s.to_string(),
        }
    }
}

impl Display for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.emit())
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AccessExpression {
    Empty,
    Token(AccessToken),
    And(Vec<AccessExpression>),
    Or(Vec<AccessExpression>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokens {
    pub tokens: Vec<AccessToken>,
}

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum ExpressionParseProblem {
    InvalidTokenStart(char),
    MissingJunction,
    TrailingQuotes,
    CharactersOutsideQuotes,
    CannotMixAndWithOr,
    EmptyTokensNotAllowedInJunctions,
    TrailingJunction,
}

impl Display for ExpressionParseProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTokenStart(c) => write!(f, "Invalid token start '{}'", c),
            Self::MissingJunction => write!(f, "Missing junction"),
            Self::TrailingQuotes => write!(f, "Trailing quotes"),
            Self::CharactersOutsideQuotes => write!(f, "Characters after close quotes not allowed"),
            Self::CannotMixAndWithOr => {
                write!(f, "Cannot mix & with |, use parens to disambiguate")
            }
            Self::EmptyTokensNotAllowedInJunctions => {
                write!(f, "Empty tokens not allowed in junctions")
            }
            Self::TrailingJunction => write!(f, "Trailing junction not allowed"),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum TokenParseProblem {
    TrailingComma,
    UnclosedQuotedToken,
    CharactersOutsideQuotes,
    TrailingBackslashInQuotes,
    InvalidTokenStart(char),
}

impl Display for TokenParseProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrailingComma => write!(f, "Trailing comma"),
            Self::UnclosedQuotedToken => write!(f, "Unclosed quoted token"),
            Self::CharactersOutsideQuotes => write!(f, "Characters after quotes"),
            Self::TrailingBackslashInQuotes => write!(f, "Trailing backslash while inside quotes"),
            Self::InvalidTokenStart(char) => write!(f, "Invalid token start '{}'", char),
        }
    }
}

pub fn expression(input: &str) -> Result<AccessExpression, ExpressionParseProblem> {
    access_expression(input)
}
pub fn tokens(input: &str) -> Result<AccessTokens, TokenParseProblem> {
    access_tokens(input)
}

pub fn evaluate(expression: &AccessExpression, tokens: &AccessTokens) -> bool {
    match (expression, tokens) {
        (AccessExpression::Empty, _) => true,
        (AccessExpression::Token(t), tokens) => tokens.tokens.contains(t),
        (AccessExpression::And(clauses), tokens) => clauses.iter().all(|c| evaluate(c, tokens)),
        (AccessExpression::Or(clauses), tokens) => clauses.iter().any(|c| evaluate(c, tokens)),
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn parse_expression() {
        assert_eq!(
            Ok(AccessExpression::Token(AccessToken::Unquoted(
                "a".to_string()
            ))),
            expression("a")
        );
    }
    #[test]
    fn parse_tokens() {
        assert_eq!(
            Ok(AccessTokens::new(&[AccessToken::Unquoted("a".to_string())])),
            tokens("a")
        );
    }
    #[test]
    fn evaluate_empty_empty() {
        assert!(evaluate(&AccessExpression::Empty, &AccessTokens::empty()))
    }
    #[test]
    fn evaluate_token_match() {
        assert!(evaluate(
            &AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
            &AccessTokens::new(&[AccessToken::Unquoted("a".to_string())])
        ))
    }
    #[test]
    fn evaluate_token_not_match() {
        assert!(!evaluate(
            &AccessExpression::Token(AccessToken::Unquoted("b".to_string())),
            &AccessTokens::new(&[AccessToken::Unquoted("a".to_string())])
        ))
    }
    #[test]
    fn evaluate_and_match() {
        assert!(evaluate(
            &AccessExpression::And(vec![
                AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
            ]),
            &AccessTokens::new(&[
                AccessToken::Unquoted("a".to_string()),
                AccessToken::Unquoted("b".to_string())
            ])
        ))
    }
    #[test]
    fn evaluate_and_not_match() {
        assert!(!evaluate(
            &AccessExpression::And(vec![
                AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
            ]),
            &AccessTokens::new(&[
                AccessToken::Unquoted("a".to_string()),
                AccessToken::Unquoted("c".to_string())
            ])
        ))
    }
    #[test]
    fn evaluate_or_match_first() {
        assert!(evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
            ]),
            &AccessTokens::new(&[AccessToken::Unquoted("a".to_string())])
        ))
    }
    #[test]
    fn evaluate_or_match_second() {
        assert!(evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
            ]),
            &AccessTokens::new(&[AccessToken::Unquoted("b".to_string())])
        ))
    }
    #[test]
    fn evaluate_or_not_match() {
        assert!(!evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
            ]),
            &AccessTokens::new(&[AccessToken::Unquoted("c".to_string())])
        ))
    }
    #[test]
    fn display_unquoted_token() {
        assert_eq!(
            format!(
                "{}",
                AccessExpression::Token(AccessToken::Unquoted("a".to_string()))
            ),
            "a"
        );
    }
    #[test]
    fn display_quoted_token() {
        assert_eq!(
            format!(
                "{}",
                AccessExpression::Token(AccessToken::Quoted("?".to_string()))
            ),
            "\"?\""
        );
    }

    #[test]
    fn display_and() {
        assert_eq!(
            format!(
                "{}",
                &AccessExpression::And(vec![
                    AccessExpression::Token(AccessToken::Unquoted("a".to_string())),
                    AccessExpression::Token(AccessToken::Unquoted("b".to_string()))
                ])
            ),
            "a&b"
        );
    }
}
