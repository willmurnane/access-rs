The AFL target validates that this codebase recognizes the language described in the accumulo-access project. Unfortunately, it appears that the Java codebase does not implement the language described in the ABNF, so the tests currently fail. To run the tests:

```
cargo afl build
cargo afl fuzz -i eval-in -o out target/debug/afl-access-eval
```
