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

use chumsky::{error::RichPattern, prelude::*, util::Maybe};

use crate::{AccessTokens, TokenParseProblem, parser::token_parser};

pub(crate) fn access_tokens(input: &str) -> Result<AccessTokens, TokenParseProblem> {
    let parsed = choice((
        token_parser()
            .separated_by(just(','))
            .collect::<Vec<String>>(),
        empty().to(vec![]),
    ))
    .parse(input);
    if let Some(err) = parsed.errors().next() {
        let expected: Vec<&RichPattern<'_, char>> = err.expected().collect::<Vec<_>>();
        return Err(match expected[..] {
            [&RichPattern::Any, &RichPattern::Token(Maybe::Val('"'))] => {
                TokenParseProblem::TrailingComma
            }
            [RichPattern::Any, &RichPattern::Token(Maybe::Val('\\'))] => {
                TokenParseProblem::UnclosedQuotedToken
            }
            [
                RichPattern::Any,
                &RichPattern::Token(Maybe::Val('\\')),
                &RichPattern::Token(Maybe::Val('"')),
            ] => TokenParseProblem::UnclosedQuotedToken,
            [RichPattern::SomethingElse, &RichPattern::Token(_)]
            | [
                RichPattern::SomethingElse,
                &RichPattern::Token(_),
                &RichPattern::Token(_),
            ]
            | [
                RichPattern::SomethingElse,
                &RichPattern::Token(_),
                RichPattern::EndOfInput,
            ] => TokenParseProblem::InvalidTokenStart(
                *err.found().expect("Should have found a character"),
            ),
            [
                &RichPattern::Token(Maybe::Val(',')),
                RichPattern::EndOfInput,
            ] => TokenParseProblem::CharactersOutsideQuotes,
            // Note: This is the only pattern I've observed for real
            // [&RichPattern::Token(Maybe::Val('"')), &RichPattern::Token(Maybe::Val('\\'))]
            // and this is just to make the match exhaustive.
            _ => TokenParseProblem::TrailingBackslashInQuotes,
        });
    }
    Ok(AccessTokens::new(&parsed.into_output().expect("msg")))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        assert_eq!(access_tokens(""), Ok(AccessTokens::empty()));
    }
    #[test]
    fn single_invalid_char() {
        assert_eq!(
            access_tokens("\t"),
            Err(TokenParseProblem::InvalidTokenStart('\t'))
        )
    }
    #[test]
    fn second_invalid_char() {
        assert_eq!(
            access_tokens("a\t"),
            Err(TokenParseProblem::InvalidTokenStart('\t'))
        )
    }
    #[test]
    fn trailing_comma() {
        assert_eq!(access_tokens("a,"), Err(TokenParseProblem::TrailingComma))
    }
    #[test]
    fn trailing_quote() {
        assert_eq!(
            access_tokens("a,\""),
            Err(TokenParseProblem::UnclosedQuotedToken)
        )
    }
    #[test]
    fn trailing_partial_token() {
        assert_eq!(
            access_tokens("a,\"b"),
            Err(TokenParseProblem::UnclosedQuotedToken)
        )
    }
    #[test]
    fn invalid_second_token() {
        assert_eq!(
            access_tokens("a,#"),
            Err(TokenParseProblem::InvalidTokenStart('#'))
        )
    }
    #[test]
    fn chars_after_quotes() {
        assert_eq!(
            access_tokens("\"a\"b"),
            Err(TokenParseProblem::CharactersOutsideQuotes)
        )
    }
    #[test]
    fn trailing_backslash() {
        assert_eq!(
            access_tokens("\"a\\"),
            Err(TokenParseProblem::TrailingBackslashInQuotes)
        )
    }
    #[test]
    fn simple() {
        assert_eq!(
            access_tokens("a"),
            Ok(AccessTokens {
                tokens: vec!["a".to_owned()]
            })
        )
    }
    #[test]
    fn quoted() {
        assert_eq!(
            access_tokens("\"a\""),
            Ok(AccessTokens {
                tokens: vec!["a".to_owned()]
            })
        )
    }
    #[test]
    fn quoted_with_escape1() {
        assert_eq!(
            access_tokens("\"\\\"\""),
            Ok(AccessTokens {
                tokens: vec!["\"".to_owned()]
            })
        )
    }
    #[test]
    fn quoted_with_escape2() {
        assert_eq!(
            access_tokens("\"\\\\\""),
            Ok(AccessTokens {
                tokens: vec!["\\".to_owned()]
            })
        )
    }
}
