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

use chumsky::error::RichPattern;
use chumsky::prelude::*;

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

#[cfg(test)]
mod tests {
    use chumsky::util::Maybe;

    use crate::{AccessExpression, ExpressionParseProblem, parser::token_parser};

    use super::*;

    fn token(s: &str) -> AccessExpression {
        AccessExpression::Token(s.to_string())
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
}
