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

use chumsky::{error::RichPattern, prelude::*, util::Maybe};
use serde::{Deserialize, Serialize};

use crate::{AccessToken, AccessTokens, TokenParseProblem, parser::token_parser};

pub fn access_tokens(
    input: &str,
) -> Result<AccessTokens, (std::ops::Range<usize>, TokenParseProblem)> {
    let parsed = choice((
        token_parser()
            .separated_by(just(','))
            .collect::<Vec<AccessToken>>(),
        empty().to(vec![]),
    ))
    .parse(input);
    if let Some(err) = parsed.errors().next() {
        let expected: Vec<&RichPattern<'_, char>> = err.expected().collect::<Vec<_>>();
        return Err((
            err.span().into_range(),
            match expected[..] {
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
                    &RichPattern::Token(_) | RichPattern::EndOfInput,
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
            },
        ));
    }
    Ok(AccessTokens::new(&parsed.into_output().expect("msg")))
}

impl AccessTokens {
    #[must_use]
    pub fn empty() -> Self {
        Self::new(&[])
    }
    #[must_use]
    pub fn new(items: &[AccessToken]) -> Self {
        let mut tokens = items.to_vec();
        tokens.sort();
        Self { tokens }
    }
    fn fmt(&self, buffer: &mut String) {
        let mut first = true;
        for t in &self.tokens {
            if !first {
                buffer.push(',');
            }
            buffer.push_str(&t.to_string());
            first = false;
        }
    }
}

impl Display for AccessTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        self.fmt(&mut s);
        write!(f, "{s}")
    }
}

impl Serialize for AccessTokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}
impl<'de> Deserialize<'de> for AccessTokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).and_then(|buf| {
            access_tokens(&buf).map_err(|(_location, err)| serde::de::Error::custom(err))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn empty() {
        assert_eq!(access_tokens(""), Ok(AccessTokens::empty()));
    }
    #[test]
    fn single_invalid_char() {
        assert_eq!(
            access_tokens("\t"),
            Err((0..1, TokenParseProblem::InvalidTokenStart('\t')))
        )
    }
    #[test]
    fn second_invalid_char() {
        assert_eq!(
            access_tokens("a\t"),
            Err((1..2, TokenParseProblem::InvalidTokenStart('\t')))
        )
    }
    #[test]
    fn trailing_comma() {
        assert_eq!(
            access_tokens("a,"),
            Err((2..2, TokenParseProblem::TrailingComma))
        )
    }
    #[test]
    fn trailing_quote() {
        assert_eq!(
            access_tokens("a,\""),
            Err((3..3, TokenParseProblem::UnclosedQuotedToken))
        )
    }
    #[test]
    fn trailing_partial_token() {
        assert_eq!(
            access_tokens("a,\"b"),
            Err((4..4, TokenParseProblem::UnclosedQuotedToken))
        )
    }
    #[test]
    fn invalid_second_token() {
        assert_eq!(
            access_tokens("a,#"),
            Err((2..3, TokenParseProblem::InvalidTokenStart('#')))
        )
    }
    #[test]
    fn chars_after_quotes() {
        assert_eq!(
            access_tokens("\"a\"b"),
            Err((3..4, TokenParseProblem::CharactersOutsideQuotes))
        )
    }
    #[test]
    fn trailing_backslash() {
        assert_eq!(
            access_tokens("\"a\\"),
            Err((3..3, TokenParseProblem::TrailingBackslashInQuotes))
        )
    }
    #[test]
    fn simple() {
        assert_eq!(
            access_tokens("a"),
            Ok(AccessTokens {
                tokens: vec![AccessToken::Unquoted("a".to_owned())]
            })
        )
    }
    #[test]
    fn quoted() {
        assert_eq!(
            access_tokens("\"a\""),
            Ok(AccessTokens {
                tokens: vec![AccessToken::Unquoted("a".to_owned())]
            })
        )
    }
    #[test]
    fn quoted_with_escape1() {
        assert_eq!(
            access_tokens("\"\\\"\""),
            Ok(AccessTokens {
                tokens: vec![AccessToken::Quoted("\"".to_owned())]
            })
        )
    }
    #[test]
    fn quoted_with_escape2() {
        assert_eq!(
            access_tokens("\"\\\\\""),
            Ok(AccessTokens {
                tokens: vec![AccessToken::Quoted("\\".to_owned())]
            })
        )
    }
    #[test]
    fn roundtrip_serde() {
        // Note: these must be instances which are sorted correctly to start with, so that they wind up serializing
        // exactly the same.
        for instance in ["a", "a,b", "a,banana", "\"a?b\"", "a,\"a?b!\""] {
            let mut f: HashMap<String, AccessTokens> = HashMap::new();
            f.insert("hello".to_string(), access_tokens(instance).unwrap());
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
    fn display_tpps() {
        assert_eq!(
            "Characters after quotes",
            format!("{}", TokenParseProblem::CharactersOutsideQuotes)
        );
        assert_eq!(
            "Trailing backslash while inside quotes",
            format!("{}", TokenParseProblem::TrailingBackslashInQuotes)
        );
        assert_eq!(
            "Trailing comma",
            format!("{}", TokenParseProblem::TrailingComma)
        );
        assert_eq!(
            "Unclosed quoted token",
            format!("{}", TokenParseProblem::UnclosedQuotedToken)
        );
        assert_eq!(
            "Invalid token start '?'",
            format!("{}", TokenParseProblem::InvalidTokenStart('?'))
        );
    }
}
