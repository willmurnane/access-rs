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

use crate::AccessExpression;
use crate::AccessToken;
use chumsky::IterParser;
use chumsky::prelude::*;

pub fn unquoted_token_parser<'a>()
-> impl Clone + Parser<'a, &'a str, String, extra::Err<Rich<'a, char>>> {
    // slash                   = "/"
    // access-token            = 1*( ALPHA / DIGIT / "_" / "-" / "." / ":" / slash )
    any::<'a, &'a str, extra::Err<Rich<'a, char>>>()
        .filter(|c: &char| c.is_ascii_alphanumeric() || "_-.:/".contains(*c))
        .repeated()
        .at_least(1)
        .collect::<String>()
}

pub fn token_parser<'a>()
-> impl Clone + Parser<'a, &'a str, AccessToken, extra::Err<Rich<'a, char>>> {
    // escaped                 = "\" DQUOTE / "\\"
    let escaped = just('\\').ignore_then(choice((just('"'), just('\\'))));
    // utf8-subset             = %x20-21 / %x23-5B / %x5D-7E / unicode-beyond-ascii ; utf8 minus '"' and '\'
    // unicode-beyond-ascii    = %x0080-D7FF / %xE000-10FFFF
    let utf8_subset = any::<_, extra::Err<Rich<'a, char>>>().filter(|c: &char| {
        matches!(*c, '\u{20}' |
                 '\u{21}'
                | '\u{23}'..='\u{5B}'
                | '\u{5D}'..='\u{7E}'
                | '\u{0080}'..='\u{D7FF}'
                | '\u{E000}'..='\u{10FFFF}')
    });
    // access-token            =/ DQUOTE 1*(utf8-subset / escaped) DQUOTE
    choice((
        unquoted_token_parser().map(AccessToken::Unquoted),
        just('"')
            .ignore_then(
                choice((utf8_subset, escaped))
                    .repeated()
                    .at_least(1)
                    .collect::<String>()
                    .map(|s| AccessToken::new(&s)),
            )
            .then_ignore(just('"')),
    ))
}

pub fn expression_parser<'a>()
-> impl Clone + Parser<'a, &'a str, AccessExpression, extra::Err<Rich<'a, char>>> {
    recursive(|exp| {
        let paren_or_token = choice((
            just('(').ignore_then(exp.clone()).then_ignore(just(')')),
            token_parser().map(AccessExpression::Token),
        ));

        let and_exp = paren_or_token
            .clone()
            .separated_by(just('&'))
            .at_least(1)
            .collect::<Vec<AccessExpression>>();
        let or_exp = paren_or_token
            .clone()
            .separated_by(just('|'))
            .at_least(1)
            .collect::<Vec<AccessExpression>>();

        paren_or_token
            .then(choice((just('&').then(and_exp), just('|').then(or_exp))).or_not())
            .map(|(head, others)| match others {
                Some(('&', mut tail)) => {
                    tail.insert(0, head);
                    AccessExpression::and(tail)
                }
                Some(('|', mut tail)) => {
                    tail.insert(0, head);
                    AccessExpression::or(tail)
                }
                _ => head,
            })
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use chumsky::{error::RichPattern, util::Maybe};

    use crate::parser::token_parser;

    use super::*;
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
}
