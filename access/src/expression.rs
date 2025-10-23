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

use chumsky::error::RichPattern;
use chumsky::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{AccessExpression, ExpressionParseProblem, parser::expression_parser};

pub fn access_expression(input: &str) -> Result<AccessExpression, ExpressionParseProblem> {
    let result = choice((expression_parser(), empty().to(AccessExpression::Empty))).parse(input);

    if let Some(err) = result.errors().next() {
        return Err(match err.found() {
            None => ExpressionParseProblem::TrailingJunction,
            Some(f) => {
                if matches!(*f, '|' | '&') {
                    if err
                        .expected()
                        .any(|ex| ex == &RichPattern::Token(chumsky::util::Maybe::Val('(')))
                    {
                        ExpressionParseProblem::InvalidTokenStart(*f)
                    } else {
                        ExpressionParseProblem::CannotMixAndWithOr
                    }
                } else {
                    match *f {
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | '/' => {
                            ExpressionParseProblem::CharactersOutsideQuotes
                        }
                        '(' => ExpressionParseProblem::MissingJunction,
                        '"' => ExpressionParseProblem::TrailingQuotes,
                        _ => ExpressionParseProblem::InvalidTokenStart(*f),
                    }
                }
            }
        });
    }
    Ok(result.into_output().expect("msg"))
}

impl Serialize for AccessExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}
impl<'de> Deserialize<'de> for AccessExpression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|buf| access_expression(&buf).map_err(serde::de::Error::custom))
    }
}

impl Display for AccessExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        self.fmt(&mut s, JunctionContext::Unknown);
        write!(f, "{}", s)
    }
}

#[derive(Eq, PartialEq)]
enum JunctionContext {
    Unknown,
    And,
    Or,
}
impl AccessExpression {
    fn fmt(&self, buf: &mut String, context: JunctionContext) {
        match self {
            AccessExpression::Empty => return,
            AccessExpression::Token(t) => {
                buf.push_str(&t.to_string());
            }
            AccessExpression::And(children) => {
                if context == JunctionContext::Or {
                    buf.push('(');
                }
                let mut first = true;
                for ele in children {
                    if !first {
                        buf.push('&');
                    }
                    ele.fmt(buf, JunctionContext::And);
                    first = false;
                }

                if context == JunctionContext::Or {
                    buf.push(')');
                }
            }
            AccessExpression::Or(children) => {
                if context == JunctionContext::And {
                    buf.push('(');
                }
                let mut first = true;
                for ele in children {
                    if !first {
                        buf.push('|');
                    }
                    ele.fmt(buf, JunctionContext::Or);
                    first = false;
                }

                if context == JunctionContext::And {
                    buf.push(')');
                }
            }
        }
    }
    pub(crate) fn and(items: Vec<AccessExpression>) -> AccessExpression {
        let mut children = vec![];
        for item in items {
            match item {
                AccessExpression::And(mut inner) => children.append(&mut inner),
                _ => children.push(item),
            }
        }
        children.sort();
        AccessExpression::And(children)
    }
    pub(crate) fn or(items: Vec<AccessExpression>) -> AccessExpression {
        let mut children = vec![];
        for item in items {
            match item {
                AccessExpression::Or(mut inner) => children.append(&mut inner),
                _ => children.push(item),
            }
        }
        children.sort();
        AccessExpression::Or(children)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chumsky::util::Maybe;

    use crate::{AccessExpression, ExpressionParseProblem, parser::token_parser};

    use super::*;

    fn token(s: &str) -> AccessExpression {
        AccessExpression::Token(crate::AccessToken::new(s))
    }

    #[test]
    fn single_token_empty() {
        assert_eq!(access_expression(""), Ok(AccessExpression::Empty));
    }
    #[test]
    fn single_token() {
        assert_eq!(access_expression("abc"), Ok(token("abc")));
    }
    #[test]
    fn invalid_tokens() {
        for example in &["\u{22}", "\u{5C}", "\u{7F}", "\u{E000}"] {
            let answer = token_parser().parse(example);
            assert_eq!(answer.output(), None);
            for ele in answer.errors() {
                assert_eq!(
                    ele.expected().collect::<Vec<&RichPattern<'_, char>>>(),
                    if example == &"\"" {
                        vec![&RichPattern::Any, &RichPattern::Token(Maybe::Val('\\'))]
                    } else {
                        vec![
                            &RichPattern::SomethingElse,
                            &RichPattern::Token(Maybe::Val('"')),
                        ]
                    }
                )
            }
        }
    }
    #[test]
    fn trailing_quote() {
        assert_eq!(
            access_expression("a\""),
            Err(ExpressionParseProblem::TrailingQuotes)
        );
    }
    #[test]
    fn weird_token() {
        assert_eq!(access_expression("\"a&b!\""), Ok(token("a&b!")))
    }
    #[test]
    fn quoted_space_token() {
        assert_eq!(access_expression("\" \""), Ok(token(" ")))
    }
    #[test]
    fn quoted_exclamation_token() {
        assert_eq!(access_expression("\"!\""), Ok(token("!")))
    }
    #[test]
    fn utf8_low_token() {
        assert_eq!(access_expression("\"\u{1234}\""), Ok(token("\u{1234}")))
    }
    #[test]
    fn utf8_high_token() {
        assert_eq!(access_expression("\"\u{FE00}\""), Ok(token("\u{FE00}")))
    }
    #[test]
    fn maximal_token() {
        let s = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ09123456789-_:./";
        assert_eq!(access_expression(s), Ok(token(s)))
    }
    #[test]
    fn parens() {
        assert_eq!(access_expression("(a)"), Ok(token("a")))
    }
    #[test]
    fn parens_quoted() {
        assert_eq!(access_expression("(\"a\")"), Ok(token("a")))
    }
    #[test]
    fn andand() {
        assert_eq!(
            access_expression("a&&"),
            Err(ExpressionParseProblem::InvalidTokenStart('&'))
        );
    }
    #[test]
    fn and2() {
        assert_eq!(
            access_expression("a&b"),
            Ok(AccessExpression::And(vec![token("a"), token("b")]))
        )
    }
    #[test]
    fn and3() {
        assert_eq!(
            access_expression("a&b&c"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }
    #[test]
    fn trailing_and() {
        assert_eq!(
            access_expression("a&"),
            Err(ExpressionParseProblem::TrailingJunction)
        );
    }
    #[test]
    fn trailing_and2() {
        assert_eq!(
            access_expression("a&b&"),
            Err(ExpressionParseProblem::TrailingJunction)
        )
    }
    #[test]
    fn and_paren1() {
        assert_eq!(
            access_expression("(a)&b"),
            Ok(AccessExpression::And(vec![token("a"), token("b")]))
        )
    }
    #[test]
    fn and_paren2() {
        assert_eq!(
            access_expression("a&(b)"),
            Ok(AccessExpression::And(vec![token("a"), token("b")]))
        )
    }
    #[test]
    fn and_paren3a() {
        assert_eq!(
            access_expression("(a)&b&c"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }
    #[test]
    fn and_paren3b() {
        assert_eq!(
            access_expression("a&(b)&c"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }
    #[test]
    fn and_paren3c() {
        assert_eq!(
            access_expression("a&b&(c)"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn and_paren_and() {
        assert_eq!(
            access_expression("a&(b&c)"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn paren_andand() {
        assert_eq!(
            access_expression("(a&)"),
            Err(ExpressionParseProblem::InvalidTokenStart(')'))
        );
    }

    #[test]
    fn paren_and_and() {
        assert_eq!(
            access_expression("(a&b)&c"),
            Ok(AccessExpression::And(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn or2() {
        assert_eq!(
            access_expression("a|b"),
            Ok(AccessExpression::Or(vec![token("a"), token("b")]))
        )
    }
    #[test]
    fn or3() {
        assert_eq!(
            access_expression("a|b|c"),
            Ok(AccessExpression::Or(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn or_paren_or() {
        assert_eq!(
            access_expression("a|(b|c)"),
            Ok(AccessExpression::Or(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn paren_or_or() {
        assert_eq!(
            access_expression("(a|b)|c"),
            Ok(AccessExpression::Or(vec![
                token("a"),
                token("b"),
                token("c")
            ]))
        )
    }

    #[test]
    fn mixed_junction_and_or() {
        assert_eq!(
            access_expression("a&b|c"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn mixed_junction_or_and() {
        assert_eq!(
            access_expression("a|b&c"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn mixed_junction_and_or_and() {
        assert_eq!(
            access_expression("a&b|c&d"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn mixed_junction_or_and_or() {
        assert_eq!(
            access_expression("a|b&c|d"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn mixed_junction_and_and_or() {
        assert_eq!(
            access_expression("a&b&c|d"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn mixed_junction_or_or_and() {
        assert_eq!(
            access_expression("a|b|c&d"),
            Err(ExpressionParseProblem::CannotMixAndWithOr)
        );
    }
    #[test]
    fn junction_after_paren_and() {
        assert_eq!(
            access_expression("a(&"),
            Err(ExpressionParseProblem::MissingJunction)
        )
    }
    #[test]
    fn junction_after_paren_or() {
        assert_eq!(
            access_expression("a(|"),
            Err(ExpressionParseProblem::MissingJunction)
        )
    }
    #[test]
    fn legal_character_after_quoted_token1() {
        assert_eq!(
            access_expression("\"a\"1"),
            Err(ExpressionParseProblem::CharactersOutsideQuotes)
        )
    }
    #[test]
    fn legal_character_after_quoted_token2() {
        assert_eq!(
            access_expression("\"a\"a"),
            Err(ExpressionParseProblem::CharactersOutsideQuotes)
        )
    }
    #[test]
    fn legal_character_after_quoted_token3() {
        assert_eq!(
            access_expression("\"a\"A"),
            Err(ExpressionParseProblem::CharactersOutsideQuotes)
        )
    }
    #[test]
    fn illegal_character_after_quoted_token() {
        assert_eq!(
            access_expression("\"#\"@"),
            Err(ExpressionParseProblem::InvalidTokenStart('@'))
        )
    }

    #[test]
    fn roundtrip() {
        // Note: these must be instances which are sorted correctly to start with, so that they wind up serializing
        // exactly the same.
        for instance in ["", "a", "a&b", "banana", "a|b", "a&(b|c)", "c|(a&b)"].into_iter() {
            assert_eq!(
                Ok(instance),
                access_expression(instance)
                    .map(|i| format!("{}", i))
                    .as_deref()
            )
        }
    }

    #[test]
    fn roundtrip_serde() {
        // Note: these must be instances which are sorted correctly to start with, so that they wind up serializing
        // exactly the same.
        for instance in ["a", "a&b", "banana", "a|b", "a&(b|c)", "c|(a&b)"] {
            let mut f: HashMap<String, AccessExpression> = HashMap::new();
            f.insert("hello".to_string(), access_expression(instance).unwrap());
            ::serde_test::assert_tokens(
                &f,
                &[
                    ::serde_test::Token::Map { len: Some(1) },
                    ::serde_test::Token::String("hello"),
                    ::serde_test::Token::String(instance),
                    ::serde_test::Token::MapEnd,
                ],
            );
        }
    }

    #[test]
    fn display_epps() {
        assert_eq!(
            "Invalid token start '?'",
            format!("{}", ExpressionParseProblem::InvalidTokenStart('?'))
        );
        assert_eq!(
            "Missing junction",
            format!("{}", ExpressionParseProblem::MissingJunction)
        );
        assert_eq!(
            "Trailing quotes",
            format!("{}", ExpressionParseProblem::TrailingQuotes)
        );
        assert_eq!(
            "Characters after close quotes not allowed",
            format!("{}", ExpressionParseProblem::CharactersOutsideQuotes)
        );
        assert_eq!(
            "Cannot mix & with |, use parens to disambiguate",
            format!("{}", ExpressionParseProblem::CannotMixAndWithOr)
        );
        assert_eq!(
            "Empty tokens not allowed in junctions",
            format!(
                "{}",
                ExpressionParseProblem::EmptyTokensNotAllowedInJunctions
            )
        );
        assert_eq!(
            "Trailing junction not allowed",
            format!("{}", ExpressionParseProblem::TrailingJunction)
        );
    }
}
