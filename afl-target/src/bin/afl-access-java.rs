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

use std::collections::HashSet;

use access::{AccessToken, AccessTokens};
use j4rs::{ClasspathEntry, InvocationArg, Jvm, JvmBuilder};

fn instantiate_jvm() -> j4rs::errors::Result<Jvm> {
    let jar = std::env::var("JAR_LOCATION").unwrap_or(env!("JAR_LOCATION").to_string());
    println!("Using JAR_LOCATION={jar}");
    let entry = ClasspathEntry::new(jar);
    let mut builder = JvmBuilder::new();
    let mut b = builder.classpath_entry(entry);

    if let Ok(base_path) = std::env::var("J4RS_BASE_PATH") {
        println!("Using J4RS_BASE_PATH={base_path}");
        b = b.with_base_path(base_path);
    }
    let jvm = b.build()?;

    Ok(jvm)
}

fn jvm_evaluate(jvm: &Jvm, expression: &str, tokens: &AccessTokens) -> j4rs::errors::Result<bool> {
    // TODO: doesn't split tokens
    let raw_tokens = tokens
        .tokens
        .iter()
        .map(|t| match t {
            AccessToken::Quoted(s) => s,
            AccessToken::Unquoted(s) => s,
        })
        // Have to filter out duplicate elements on the Rust side, as java's Set.of throws on duplicate entries.
        .collect::<HashSet<&String>>()
        .into_iter()
        .map(|s| InvocationArg::try_from(s).unwrap())
        .collect::<Vec<InvocationArg>>();

    // Access.Builder access_builder = Access.builder();
    let access_builder = jvm
        .invoke_static(
            "org.apache.accumulo.access.Access",
            "builder",
            InvocationArg::empty(),
        )
        .unwrap();
    // builder.authorizationValidator()
    // Access access = access_builder.build()
    let access = jvm
        .invoke(&access_builder, "build", InvocationArg::empty())
        .unwrap();

    let tokens_array = jvm.create_java_array("java.lang.String", &raw_tokens)?;

    let tokens_set =
        jvm.invoke_static("java.util.Set", "of", &[InvocationArg::from(tokens_array)])?;
    // Create an AccessEvaluator object
    let evaluator = jvm.invoke(&access, "newEvaluator", &[InvocationArg::from(tokens_set)])?;

    let sample_string = jvm.create_instance(
        "java.lang.String",
        &[InvocationArg::try_from(expression).unwrap()],
    )?;
    let boolean_instance = jvm.invoke(
        &evaluator,
        "canAccess",
        &[InvocationArg::from(sample_string)],
    )?;

    jvm.to_rust(boolean_instance)
}

fn verify(jvm: &Jvm, data: &[u8]) {
    if let Some(split) = data.iter().position(|c| *c == 0)
        && let Ok(expression_str) = std::str::from_utf8(&data[0..split])
        && let Ok(token_str) = std::str::from_utf8(&data[(split + 1)..data.len()])
        && let Ok(tokens) = access::tokens(token_str)
    {
        let java_result = jvm_evaluate(jvm, expression_str, &tokens);
        let rust_result = access::expression(expression_str).map(|e| access::evaluate(&e, &tokens));
        assert_eq!(
            java_result.is_ok(),
            rust_result.is_ok(),
            "java:{:?} rust:{:?} for value {data:?}",
            java_result,
            rust_result
        );
        if let Ok(rust_answer) = rust_result {
            assert_eq!(rust_answer, java_result.unwrap());
        }
    }
}

fn main() {
    let jvm_result = instantiate_jvm();
    let jvm = match jvm_result {
        Ok(j) => j,
        Err(e) => panic!("Unable to load JVM: {e}"),
    };

    verify(&jvm, b"a&b\0b,b");
    verify(&jvm, b"a&\"b\0");
    verify(&jvm, b"\"\x01\"&c\0w");
    verify(&jvm, b"\"a\"&c\0a,c");

    println!("Successfully verified java and rust implementations behave the same");

    fuzz!(|data: &[u8]| {
        verify(&jvm, data);
    });
}
