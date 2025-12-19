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

#[macro_use]
extern crate afl;

use access::evaluate;

fn main() {
    fuzz!(|data: &[u8]| {
        if let Some(split) = data.iter().position(|c| *c == 0xFF)
            && let Ok(expression_str) = std::str::from_utf8(&data[0..split])
            && let Ok(token_str) = std::str::from_utf8(&data[(split + 1)..data.len()])
            && let Ok(expression) = access::expression(expression_str)
            && let Ok(tokens) = access::tokens(token_str)
        {
            let _ = evaluate(&expression, &tokens);
        }
    });
}
