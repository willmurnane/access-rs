## What is this

This project implements the [accumulo-access specification](https://github.com/apache/accumulo-access/blob/main/SPECIFICATION.md) in Rust.

## What can I do with this

This is intended to act as a component in a larger system, which performs filtering of data, allowing users to see only the data they are authorized to see.

### Accumulo access recap

Access tokens are strings which represent some particular aspect of access. The meaning of these tokens is up to you. For example:

- in a system where users from different locations have different levels of access, you might assign tokens `Antigua` and `Barbados` to users from those locations.
- in a system where different departments are not permitted to share certain kinds of data, you might assign one token per department
- in a system with customer support data, you might have different access tokens to represent data available to tier 1 support, tier 2 support, supervisors, etc.

Tokens composed only of the characters `[a-zA-Z0-9_\-\.:/]` (latin alphabet, numbers, or the characters `_-.:/`) may be expressed without quoting. In addition, any valid sequence of non-surrogate UTF-8 code points may be used as a token, as long as the following algorithm is followed to construct a quoted token:

- append a double quote `"` to the quoted token
- for each code point which is to be emitted:
  - if the code point is a double quote `\u0022`, append a backslash and a double-quote `\"`
  - else if the code point is a backslash `\u005c`, append two backslashes `\\`
  - else append the code point
- finally, append a double quote.

Quoted tokens which may be written as unquoted tokens are equivalent to the unquoted form: parsing `access::tokens("\"a\"") == access::tokens("a")`.

Note that tokens are case-sensitive, and do not obey [Unicode equivalence](https://en.wikipedia.org/wiki/Unicode_equivalence) rules: `U&'\006E\0303'` and `U&'\00F1'` are different sequences of code points, and thus are unrelated tokens as far as accumulo access is concerned, despite representing the same character: ñ ñ. Apply appropriate normalization (NFC and/or case folding) externally if necessary.

Access expressions are boolean expressions comprised of tokens. When evaluated with respect to a set of tokens, each token in the expression will be treated as `true` if the token is present or `false` if it is absent.

- Individual tokens (quoted or unquoted, as defined above) are valid sub-expressions.
- Sub-expressions may be surrounded with parentheses to disambiguate meaning: `(A)` or even `((((((a))))))` are valid groupings.
- Sub-expressions may be combined with junctions: `&` meaning "and", or `|` meaning "or". Mixing junctions is not permitted: `A&B|C&D` is not permitted, because it could be interpreted as `A&(B|C)&D` or `(A&B)|(C&D)`.

For example, `":)"&Z&("…"|"A")` is a valid access expression.

### Accumulo access as applied

Out of these concepts, two types and associated parsers are provided, along with a function to evaluate whether a set of tokens are sufficient to access data protected by an expression.

- `AccessExpression`: A boolean expression required to view a piece of data. Conversion to and from string are provided, and the clauses and sub-clauses are put into a canonical order when read. For example, `SELECT '(b&D)|Z|(a|c)'::accessexpression;` returns `Z|a|c|(D&b)`. This has no impact on the meaning of the expression.

- `AccessTokens`: A set of string labels a user possesses. Conversions to and from string are provided, and the tokens will be put into a canonical order when read. For example: `SELECT '":)",A,"…",Z'::accesstokens;` returns `A,Z,":)","…"`, because the value has been parsed into the canonical form, then converted back to a string. This has no impact on how the tokens are evaluated with respect to an expression.

- `access_evaluate(expression, tokens)`: A function that checks if a set of tokens are sufficient for an expression. For example:
  - `access_evaluate('A&(b|c)'::accessexpression, 'A,c'::accesstokens)` returns true, because `A` is sufficient to fulfill the first clause, and `c` is sufficient for the second.
  - `access_evaluate('A&(b|c)'::accessexpression, 'b,c'::accesstokens)` returns false, because although the second clause is fulfilled
