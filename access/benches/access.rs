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

use std::hint::black_box;

use access::expression;
use criterion::{Criterion, criterion_group, criterion_main};

pub fn criterion_benchmark(c: &mut Criterion) {
    for sample in &[
        "a",
        "averylongtokenwhichjustkeepsgoingandneverstopsimeancomeon",
        "a&b",
        "a&b&c",
        "a&(b&c)",
    ] {
        c.bench_function(&format!("parse {}", sample), |b| {
            b.iter(|| access_expression(black_box(sample)))
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
