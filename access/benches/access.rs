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

use access::{evaluate, expression};
use criterion::{Criterion, criterion_group, criterion_main};

use j4rs::{ClasspathEntry, InvocationArg, Jvm, JvmBuilder};

fn instantiate_jvm() -> j4rs::errors::Result<Jvm> {
    let jar = env!("JAR_LOCATION");
    let entry = ClasspathEntry::new(jar);
    let jvm: Jvm = JvmBuilder::new().classpath_entry(entry).build()?;

    Ok(jvm)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let jvm = instantiate_jvm().expect("Failed to instantiate JVM");
    for (sample, tokens) in &[
        ("a", "a"),
        (
            "averylongtokenwhichjustkeepsgoingandneverstopsimeancomeon",
            "averylongtokenwhichjustkeepsgoingandneverstopsimeancomeon",
        ),
        ("a&b", "b,c,d"),
        ("a&b&c", "a,b,c"),
        ("a&(b&c)", "a,b,c"),
    ] {
        c.bench_function(&format!("java parse {sample}"), |b| {
            b.iter(|| {
                // Access.Builder access_builder = Access.builder();
                let access_builder = jvm
                    .invoke_static(
                        "org.apache.accumulo.access.Access",
                        "builder",
                        InvocationArg::empty(),
                    )
                    .unwrap();
                // Access access = access_builder.build()
                let access = jvm
                    .invoke(&access_builder, "build", InvocationArg::empty())
                    .unwrap();

                // Instantiate 'tokens' as array of strings
                let tokens_jvm = jvm
                    .create_java_array(
                        "java.lang.String",
                        &tokens
                            .split(',')
                            .map(|t| InvocationArg::try_from(t).unwrap())
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                // Instantiate 'tokens' as a Set<String>
                let tokens_set = jvm
                    .invoke_static("java.util.Set", "of", &[InvocationArg::from(tokens_jvm)])
                    .unwrap();
                // Create an Authorizations object
                let auths = jvm
                    .invoke(
                        &access,
                        "newAuthorizations",
                        &[InvocationArg::from(tokens_set)],
                    )
                    .unwrap();
                let evaluator = jvm
                    .invoke(&access, "newEvaluator", &[InvocationArg::from(auths)])
                    .unwrap();
                // Instantiate 'sample' as java string
                let sample_string = jvm
                    .create_instance(
                        "java.lang.String",
                        &[InvocationArg::try_from(*sample).unwrap()],
                    )
                    .expect("Convert to java instance");
                let _ = jvm
                    .invoke(
                        &evaluator,
                        "canAccess",
                        &[InvocationArg::from(sample_string)],
                    )
                    .unwrap();
            });
        });

        c.bench_function(&format!("rust parse {sample}"), |b| {
            b.iter(|| {
                let ex = expression(black_box(sample)).unwrap();
                let tok = access::tokens(black_box(tokens)).unwrap();
                let _ = evaluate(&ex, &tok);
            })
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
