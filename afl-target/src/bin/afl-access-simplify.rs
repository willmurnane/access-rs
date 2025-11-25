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

use access::verify_simplification;

#[macro_use]
extern crate afl;

fn verify(data: &[u8]) {
    if let Ok(expression_str) = std::str::from_utf8(data)
        && let Ok(expression) = access::expression(expression_str)
    {
        let tokens = expression.relevant_tokens();
        if tokens.tokens.len() <= 6 {
            // Okay, we have an expression involving only a few tokens. Run an exhaustive search, evaluating all 2^n
            // combinations of tokens on both the original and simplified versions to make sure they match.
            verify_simplification(&expression);
        }
    }
}

fn main() {
    verify(b"f&b&ca&b&(a|b)\0");
    verify(b"(ag&(b))|(b&(b))\0");
    println!("Okay looks good, continuing");
    fuzz!(|data: &[u8]| {
        verify(data);
    });
}
