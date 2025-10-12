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

use crate::{expression::access_expression, tokens::access_tokens};

mod expression;
mod parser;
mod tokens;

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum AccessExpression {
    Empty,
    Token(String),
    And(Vec<AccessExpression>),
    Or(Vec<AccessExpression>),
}
impl AccessExpression {
    fn and(items: Vec<AccessExpression>) -> AccessExpression {
        let mut children = vec![];
        for item in items {
            match item {
                AccessExpression::And(mut inner) => children.append(&mut inner),
                _ => children.push(item),
            }
        }
        AccessExpression::And(children)
    }
    fn or(items: Vec<AccessExpression>) -> AccessExpression {
        let mut children = vec![];
        for item in items {
            match item {
                AccessExpression::Or(mut inner) => children.append(&mut inner),
                _ => children.push(item),
            }
        }
        AccessExpression::Or(children)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokens {
    pub tokens: Vec<String>,
}
impl AccessTokens {
    pub fn empty() -> AccessTokens {
        Self::new(&[])
    }
    pub fn new(items: &[String]) -> AccessTokens {
        AccessTokens {
            tokens: items.to_vec(),
        }
    }
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

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum TokenParseProblem {
    TrailingComma,
    UnclosedQuotedToken,
    CharactersOutsideQuotes,
    TrailingBackslashInQuotes,
    InvalidTokenStart(char),
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
            Ok(AccessExpression::Token("a".to_string())),
            expression("a")
        );
    }
    #[test]
    fn parse_tokens() {
        assert_eq!(Ok(AccessTokens::new(&["a".to_string()])), tokens("a"));
    }
    #[test]
    fn evaluate_empty_empty() {
        assert!(evaluate(&AccessExpression::Empty, &AccessTokens::empty()))
    }
    #[test]
    fn evaluate_token_match() {
        assert!(evaluate(
            &AccessExpression::Token("a".to_string()),
            &AccessTokens::new(&["a".to_string()])
        ))
    }
    #[test]
    fn evaluate_token_not_match() {
        assert!(!evaluate(
            &AccessExpression::Token("b".to_string()),
            &AccessTokens::new(&["a".to_string()])
        ))
    }
    #[test]
    fn evaluate_and_match() {
        assert!(evaluate(
            &AccessExpression::And(vec![
                AccessExpression::Token("a".to_string()),
                AccessExpression::Token("b".to_string())
            ]),
            &AccessTokens::new(&["a".to_string(), "b".to_string()])
        ))
    }
    #[test]
    fn evaluate_and_not_match() {
        assert!(!evaluate(
            &AccessExpression::And(vec![
                AccessExpression::Token("a".to_string()),
                AccessExpression::Token("b".to_string())
            ]),
            &AccessTokens::new(&["a".to_string(), "c".to_string()])
        ))
    }
    #[test]
    fn evaluate_or_match_first() {
        assert!(evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token("a".to_string()),
                AccessExpression::Token("b".to_string())
            ]),
            &AccessTokens::new(&["a".to_string()])
        ))
    }
    #[test]
    fn evaluate_or_match_second() {
        assert!(evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token("a".to_string()),
                AccessExpression::Token("b".to_string())
            ]),
            &AccessTokens::new(&["b".to_string()])
        ))
    }
    #[test]
    fn evaluate_or_not_match() {
        assert!(!evaluate(
            &AccessExpression::Or(vec![
                AccessExpression::Token("a".to_string()),
                AccessExpression::Token("b".to_string())
            ]),
            &AccessTokens::new(&["c".to_string()])
        ))
    }
}
