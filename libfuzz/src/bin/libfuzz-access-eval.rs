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

#![no_main]

use access::evaluate;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(split) = data.iter().position(|c| *c == 0xFF) {
        if let (Ok(expression_str), Ok(token_str)) = (
            std::str::from_utf8(&data[0..split]),
            std::str::from_utf8(&data[(split + 1)..data.len()]),
        ) {
            if let (Ok(expression), Ok(tokens)) = (
                access::expression(expression_str),
                access::tokens(token_str),
            ) {
                evaluate(&expression, &tokens);
            }
        }
    }
});
