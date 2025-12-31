use std::collections::HashSet;

use crate::{AccessExpression, AccessToken};
#[cfg(feature = "debug_simplify")]
use tracing::{Level, event, info, instrument};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum JunctionType {
    And,
    Or,
}
impl JunctionType {
    #[cfg(feature = "debug_simplify")]
    #[must_use]
    const fn name(self) -> &'static str {
        match self {
            Self::And => "AND",
            Self::Or => "OR",
        }
    }
    #[must_use]
    const fn wrap(self, children: Vec<AccessExpression>) -> AccessExpression {
        match self {
            Self::And => AccessExpression::And(children),
            Self::Or => AccessExpression::Or(children),
        }
    }
    #[must_use]
    const fn opposite(self) -> Self {
        match self {
            Self::And => Self::Or,
            Self::Or => Self::And,
        }
    }
}

#[cfg_attr(feature = "debug_simplify", instrument)]
#[must_use]
pub fn simplify(expression: &AccessExpression) -> AccessExpression {
    match expression {
        AccessExpression::Empty | AccessExpression::Token(_) => expression.clone(),
        AccessExpression::Or(children) | AccessExpression::And(children) => {
            let junction_type = if matches!(expression, AccessExpression::And(_)) {
                JunctionType::And
            } else {
                JunctionType::Or
            };
            simplify_inner(junction_type, children, &HashSet::new())
        }
    }
}

/// Partition the given children of a junction (either 'and' or 'or', depending on the value of `junction_type`) into two
/// categories:
/// * Tokens directly contained by this junction
/// * Junctions of the opposite type of this junction.
#[must_use]
fn flatten_and_collect<I>(
    junction_type: JunctionType,
    root: I,
) -> (Vec<AccessToken>, Vec<Vec<AccessExpression>>, bool)
where
    I: Iterator<Item = AccessExpression>,
{
    let mut tokens = vec![];
    let mut junctions = vec![];
    let mut found_empty = false;

    for ele in root {
        match (ele, junction_type) {
            (AccessExpression::Empty, _) => {
                found_empty = true;
            }
            (AccessExpression::Token(t), _) => {
                tokens.push(t);
            }
            (AccessExpression::And(gc), JunctionType::And)
            | (AccessExpression::Or(gc), JunctionType::Or) => {
                // In case we get a parse tree like 'A&(B&(C&(...)))', walk all the way down any junctions that have
                // the same type to find all the leaf tokens directly, so we don't have to distinguish between
                // 'A&(B&(C&(...' and 'A&B&C&(...)'.
                let (inner_tokens, inner_junctions, inner_empty) =
                    flatten_and_collect(junction_type, gc.into_iter());
                tokens.extend(inner_tokens);
                junctions.extend(inner_junctions);
                found_empty |= inner_empty;
            }
            (AccessExpression::And(gc), JunctionType::Or)
            | (AccessExpression::Or(gc), JunctionType::And) => {
                junctions.push(gc);
            }
        }
    }
    tokens.sort();
    tokens.dedup(); // FIXME does this change behavior?
    (tokens, junctions, found_empty)
}

#[cfg_attr(feature = "debug_simplify", instrument)]
#[must_use]
fn simplify_inner(
    junction_type: JunctionType,
    children: &[AccessExpression],
    assumed_tokens: &HashSet<AccessToken>,
) -> AccessExpression {
    // Find all the Token instances that match tokens we're already requiring the user to have. In 'A&(B|A)' the inner
    // parenthesized expression is redundant, but we can't eliminate it now or we wouldn't be able to tell the
    // difference between that and 'A&(B)', which doesn't simplify. Later we'll simplify that last clause to
    // eliminate the subclause and wind up with just 'A&B'.

    let (mut top_level_tokens, inner_junctions, _found_empty) =
        flatten_and_collect(junction_type, children.iter().cloned());

    top_level_tokens.sort();
    top_level_tokens.dedup();

    // Track tokens that are required to pass this expression, so that we can simplify any sub-expressions with respect
    // to those tokens. 'and' expressions mean we can assume all of the plain tokens mentioned in this junction are
    // held below this point; 'or' expressions don't give us any new assumptions.

    let mut new_assumed_tokens = assumed_tokens.clone();
    if junction_type == JunctionType::And {
        for t in &top_level_tokens {
            new_assumed_tokens.insert(t.clone());
        }
    }
    let squash_result = simplify_with_context(
        junction_type,
        &mut top_level_tokens,
        &new_assumed_tokens,
        &inner_junctions,
    );
    if junction_type == JunctionType::Or && squash_result.is_none() {
        #[cfg(feature = "debug_simplify")]
        info!(
            "Simplified OR {:?} to EMPTY while assuming {:?}",
            children, new_assumed_tokens
        );
        return AccessExpression::Empty;
    }
    let mut new_inner_junctions = squash_result.unwrap();
    // Now, all the sub-expressions have been simplified, and all the results distilled into two categories:
    // * individual tokens
    // * collections of expressions, which are junctioned with the opposite type of junction as the 'junction_type'
    //   variable being tracked in this function.
    for junction in &mut new_inner_junctions {
        junction.sort();
    }
    new_inner_junctions.sort();
    new_inner_junctions.dedup();
    #[cfg(feature = "debug_simplify")]
    event!(
        Level::INFO,
        "{} tokens {top_level_tokens:?}, Inner junctions: {new_inner_junctions:?} holding {new_assumed_tokens:?}",
        junction_type.name(),
    );
    let mut redundant_junctions: HashSet<usize> = HashSet::new();

    redundant_junctions.extend(identify_redundant_junctions(
        junction_type,
        &new_inner_junctions,
        &top_level_tokens,
    ));

    let mut simplified: Vec<AccessExpression> = Vec::new();
    top_level_tokens
        .into_iter()
        .filter(|t| !assumed_tokens.contains(t))
        .for_each(|c| simplified.push(AccessExpression::Token(c)));

    new_inner_junctions
        .into_iter()
        .enumerate()
        .for_each(|(index, ele)| {
            // println!("new_inner_junctions contained {:?}", ele);
            if !redundant_junctions.contains(&index) {
                // These are the opposite type as the parent
                simplified.push(junction_type.opposite().wrap(ele));
            }
        });

    #[cfg(feature = "debug_simplify")]
    verify_equivalence(
        &AccessExpression::And(
            assumed_tokens
                .clone()
                .into_iter()
                .map(AccessExpression::Token)
                .chain([junction_type.wrap(children.to_vec())])
                .collect(),
        ),
        &AccessExpression::And(
            assumed_tokens
                .clone()
                .into_iter()
                .map(AccessExpression::Token)
                .chain([junction_type.wrap(simplified.clone())])
                .collect(),
        ),
    );

    // There exist further simplifications opportunities, probably, but the kind of super-nested structure that could
    // lead to those cases seem unlikely to occur in practice.

    // Finally, if we are left with a single child, return the child rather than wrapping it...

    #[cfg(feature = "debug_simplify")]
    info!(
        "Simplified {} {:?} to {:?} while assuming {:?}",
        junction_type.name(),
        children,
        simplified,
        assumed_tokens
    );
    if simplified.is_empty() {
        AccessExpression::Empty
    } else if simplified.len() == 1 {
        simplified[0].clone()
    } else {
        // or wrap it in the correct type of junction.
        simplified.sort();
        junction_type.wrap(simplified)
    }
}

/// Assuming that a set of tokens are known to be held, reduce all children below this point as much as possible.
/// Consider the expression 'A&(B|(A&C))'.
/// 1. When the top level AND is parsed, it will be split into `top_level_tokens` = 'A', and `inner_junctions` will
///    be '[B|(A&C)]'.
///     1. When this function eventually calls itself recursively on the inner junction, `new_assumed_tokens` will be
///        set to 'A' and, the inner call will have `top_level_tokens` = 'B', and `inner_junctions` will be '[A&C]'.
///         1. Again, this function will call itself to evaluate the inner junction. `new_assumed_tokens` will still be
///            'A' (because OR junctions don't allow us to assume that any particular one of their children are held).
///            `top_level_tokens` will be 'A,C' and `inner_junctions` will be empty. In this case, the simplification
///            logic will be able to eliminate the 'A', because it is already known to be required at this point, so
///            the expression will simplify to `top_level_tokens` = 'C', and `inner_junctions` will be empty.
///     2. The innermost simplification has reduced the 'A&C' element previously in `inner_junctions` to just 'C', so
///        that is added to the `top_level_tokens` and eliminated from `inner_junctions`, so the state is now
///        `top_level_tokens` = 'B,C' and empty `inner_junctions`. Because no inner junctions remain, no additional
///        loop iteration is necessary, and the result is returned.
/// 2. The middle simplification has reduced the inner junction to `B|C`. No new top-level token was produced, so no
///    further loop iteration is necessary, and the expression is returned as 'A&(B|C)'.
#[cfg_attr(feature = "debug_simplify", instrument)]
#[must_use]
fn simplify_with_context(
    junction_type: JunctionType,
    top_level_tokens: &mut Vec<AccessToken>,
    assumed_tokens: &HashSet<AccessToken>,
    inner_junctions: &[Vec<AccessExpression>],
) -> Option<Vec<Vec<AccessExpression>>> {
    let mut new_inner_junctions = inner_junctions.to_vec();
    let mut new_assumed_tokens = assumed_tokens.clone();

    loop {
        // Now that we've collected all of the plain tokens that this expression requires, and updated assumptions
        // about what tokens the user must hold to get this far, simplify any sub-expressions with those in mind.
        // Sometimes this leads to a sub-expression collapsing to a single token, so we do this repeatedly.
        let (next_tokens, next_inner_junctions, found_empty) = flatten_and_collect(
            junction_type,
            new_inner_junctions
                .into_iter()
                .map(|j| simplify_inner(junction_type.opposite(), &j, &new_assumed_tokens)),
        );
        if junction_type == JunctionType::Or && found_empty {
            return None;
        }
        #[cfg(feature = "debug_simplify")]
        event!(
            Level::INFO,
            "Loop iteration: reduced {top_level_tokens:?} and {inner_junctions:?} to {next_tokens:?} and {next_inner_junctions:?} by holding {new_assumed_tokens:?}"
        );
        new_inner_junctions = next_inner_junctions;
        top_level_tokens.extend_from_slice(&next_tokens);
        top_level_tokens.sort();
        top_level_tokens.dedup();
        match junction_type {
            JunctionType::And => {
                // Since the simplification of some child resulted in it turning to a single token, it should be assumed
                // to be held while simplifying all its neighbors on the next pass.
                for t in &next_tokens {
                    new_assumed_tokens.insert(t.clone());
                }
            }
            JunctionType::Or => {
                // If this is an OR, and we already know that one of the tokens in a sub-expression is held, this
                // entire clause is true and can be eliminated. This is the 'A&(A|B)' case mentioned earlier.
                if top_level_tokens.iter().any(|t| assumed_tokens.contains(t)) {
                    #[cfg(feature = "debug_simplify")]
                    event!(
                        Level::INFO,
                        "Found redundant OR clause: {top_level_tokens:?} {new_inner_junctions:?}"
                    );
                    return None;
                }
            }
        }
        // If we did discovered any new top-level tokens this iteration (which could possibly simplify other inner
        // junctions) then we should keep trying. If there are no new tokens or no junctions to simplify, quit trying now.
        if next_tokens.is_empty() || new_inner_junctions.is_empty() {
            break;
        }
        #[cfg(feature = "debug_simplify")]
        info!(
            "Found new top-level tokens {next_tokens:?} in {}, remaining tokens {top_level_tokens:?} junctions {new_inner_junctions:?}",
            junction_type.name()
        );
    }

    // If a junction entirely simplified away (for example, A|B when B is known to be held) and this is an AND, the whole
    // expression simplifies to
    Some(new_inner_junctions)
}

/// Last simplification step: if there are any pairs of sub-expressions in which one expression is
/// 'absorbed' by another, the 'absorbed' expression is redundant, and can be eliminated.
/// This is done with a O(n^2) loop over the junctions which remain at this point; typically there are not
/// enough distinct clauses that this is a problem.
/// The term 'absorbed' is taken from the Absorption Laws of boolean algebra:
///   <https://en.wikipedia.org/wiki/Absorption_law#Logic>
/// In an OR context, B is redundant (A || (A && B) == A).
/// In an AND context, A is redundant (A && (A || B) == A).
///
/// Complexity: O(n^2 * m) where n is the number of junctions and m is the number of terms in each set.
/// This is acceptable because simplification usually happens on small, pre-processed sets.
///
/// The logic is extended beyond a single token, so in these equations A can be a stand-in for
/// any collection of tokens: (A1&A2&A3) & ((A1&A2&A3) | B) == (A1&A2&A3), and so on. In the case of multiple
/// tokens, the comparison can be done with "is a superset" for ANDs and "is a subset" for ORs:
/// (A1&A2&A3) | (A1&A2) == (A1&A2)    because the more restrictive set of tokens is the one that restricts more
/// (A1|A2|A3) & (A1|A2) == (A1|A2|A3) because the less restrictive set of tokens allows access more.
///
///  This function returns a set of indices in the `expressions` array, which can be safely removed entirely.
#[must_use]
fn identify_redundant_junctions(
    junction_type: JunctionType,
    junctions: &[Vec<AccessExpression>],
    top_tokens: &[AccessToken],
) -> HashSet<usize> {
    let mut redundant = HashSet::new();
    let top_exprs: Vec<AccessExpression> = top_tokens
        .iter()
        .map(|t| AccessExpression::Token(t.clone()))
        .collect();

    for (i, current_junction) in junctions.iter().enumerate() {
        // 1. Check Top-Level Tokens vs Current Junction
        // We treat the current junction as a single expression (And/Or)
        let current_expr = junction_type.opposite().wrap(current_junction.clone());

        if junction_type == JunctionType::Or {
            // Context: A | (A & B). The junction (A & B) is redundant if it satisfies A.
            // Note: top_exprs here represents a single choice in the top-level OR.
            if is_implied_by(
                &AccessExpression::Or(top_exprs.clone()),
                std::slice::from_ref(&current_expr),
            ) {
                redundant.insert(i);
                continue;
            }
        }

        // 2. Junction vs Junction
        for (j, other_junction) in junctions.iter().enumerate() {
            if i <= j || redundant.contains(&j) {
                continue;
            }

            let other_expr = junction_type.opposite().wrap(other_junction.clone());

            // Does one imply the other?
            let i_implies_j = is_implied_by(&other_expr, std::slice::from_ref(&current_expr));
            let j_implies_i = is_implied_by(&current_expr, std::slice::from_ref(&other_expr));

            match junction_type {
                JunctionType::And => {
                    // In AND context: (a|b) & (a|b|c) -> (a|b)
                    // (a|b) is stronger. (a|b|c) is redundant.
                    // j_implies_i means j is weaker.
                    if i_implies_j {
                        redundant.insert(j);
                    } else if j_implies_i {
                        redundant.insert(i);
                        break;
                    }
                }
                JunctionType::Or => {
                    // In OR context: (a&b) | (a&b&c) -> (a&b)
                    // (a&b) is weaker. (a&b&c) is redundant.
                    // i_implies_j means i is stronger.
                    if i_implies_j {
                        redundant.insert(i);
                        break;
                    } else if j_implies_i {
                        redundant.insert(j);
                    }
                }
            }
        }
    }
    redundant
}
#[must_use]
fn is_implied_by(requirement: &AccessExpression, provided: &[AccessExpression]) -> bool {
    if provided.contains(requirement) || matches!(requirement, AccessExpression::Empty) {
        return true;
    }

    // 1. If we have an 'And' provided, we have all its children.
    // 2. If we have an 'Or' provided, it only satisfies the requirement if EVERY branch satisfies it.
    for p in provided {
        match p {
            AccessExpression::And(p_children) => {
                if is_implied_by(requirement, p_children) {
                    return true;
                }
            }
            AccessExpression::Or(p_children) => {
                // This is the key for the (af|s&a&w) case.
                // An Or satisfies a requirement if all its paths satisfy the requirement.
                if p_children
                    .iter()
                    .all(|p_child| is_implied_by(requirement, std::slice::from_ref(p_child)))
                {
                    return true;
                }
            }
            _ => {}
        }
    }

    match requirement {
        AccessExpression::Or(req_children) => {
            req_children.iter().any(|c| is_implied_by(c, provided))
        }
        AccessExpression::And(req_children) => {
            req_children.iter().all(|c| is_implied_by(c, provided))
        }
        _ => false,
    }
}
#[cfg(feature = "debug_simplify")]
fn init_logging() {
    tracing_subscriber::fmt().pretty().init();
}

#[cfg(feature = "debug_simplify")]
static INIT: ::std::sync::Once = ::std::sync::Once::new();
#[cfg(feature = "debug_simplify")]
pub fn verify_simplification(expression: &AccessExpression) {
    INIT.call_once(init_logging);
    verify_equivalence(expression, &expression.simplify());
}

#[cfg(any(feature = "debug_simplify", test))]
pub fn verify_equivalence(e1: &AccessExpression, e2: &AccessExpression) {
    let tokens = e1.relevant_tokens();

    let limit = 2usize.pow(u32::try_from(tokens.tokens.len()).expect("Don't use so many tokens"));
    for condition in 0..limit {
        use crate::{AccessTokens, evaluate};

        let mut evaluate_with = vec![];
        for (index, t) in tokens.tokens.iter().enumerate() {
            if condition & (1 << index) == 0 {
                evaluate_with.push(t.clone());
            }
        }
        let t = AccessTokens {
            tokens: evaluate_with,
        };
        // If simplification is correct, this will never panic.
        assert_eq!(
            evaluate(e1, &t),
            evaluate(e2, &t),
            "Evaluation of {e1} vs {e2} failed: different results when evaluated with {t}"
        );
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {

    use crate::{
        AccessExpression, AccessToken,
        expression::access_expression,
        simplify::{is_implied_by, simplify, verify_equivalence},
    };
    #[cfg(feature = "debug_simplify")]
    use tracing::info;

    fn test_with_parsing(input: &str, output: &str) {
        #[cfg(feature = "debug_simplify")]
        crate::simplify::INIT.call_once(crate::simplify::init_logging);

        let lhs = access_expression(input).expect("Input could not be parsed");
        let rhs = access_expression(output).expect("Output could not be parsed");
        #[cfg(feature = "debug_simplify")]
        info!("Checking equivalence of provided input to provided output");
        verify_equivalence(&lhs, &rhs);
        #[cfg(feature = "debug_simplify")]
        info!("Checking that provided input simplifies to provided output");
        assert_eq!(
            lhs.simplify(),
            rhs,
            "Simplified version on left did not match expectations on right"
        );
    }

    #[test]
    fn simplify_empty() {
        assert_eq!(simplify(&AccessExpression::Empty), AccessExpression::Empty)
    }
    #[test]
    fn simplify_token() {
        let token: AccessToken = crate::AccessToken::new("a");
        assert_eq!(
            simplify(&AccessExpression::Token(token.clone())),
            AccessExpression::Token(token)
        )
    }
    #[test]
    fn simplify_and_nop() {
        let token1: AccessToken = crate::AccessToken::new("a");
        let token2: AccessToken = crate::AccessToken::new("b");
        assert_eq!(
            simplify(&AccessExpression::And(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Token(token2.clone())
            ])),
            AccessExpression::And(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Token(token2.clone())
            ])
        )
    }
    #[test]
    fn simplify_and_and() {
        let token1: AccessToken = crate::AccessToken::new("a");
        let token2: AccessToken = crate::AccessToken::new("b");
        assert_eq!(
            simplify(&AccessExpression::And(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::And(vec![
                    AccessExpression::Token(token1.clone()),
                    AccessExpression::Token(token2.clone())
                ])
            ])),
            AccessExpression::And(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Token(token2.clone())
            ])
        )
    }
    #[test]
    fn simplify_and_or() {
        let token1: AccessToken = crate::AccessToken::new("a");
        let token2: AccessToken = crate::AccessToken::new("b");
        assert_eq!(
            simplify(&AccessExpression::And(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Or(vec![
                    AccessExpression::Token(token1.clone()),
                    AccessExpression::Token(token2.clone())
                ])
            ])),
            AccessExpression::Token(token1.clone()),
        )
    }
    #[test]
    fn simplify_or_and() {
        let token1: AccessToken = crate::AccessToken::new("a");
        let token2: AccessToken = crate::AccessToken::new("b");
        assert_eq!(
            simplify(&AccessExpression::Or(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::And(vec![
                    AccessExpression::Token(token1.clone()),
                    AccessExpression::Token(token2.clone())
                ])
            ])),
            AccessExpression::Token(token1.clone())
        )
    }
    #[test]
    fn simplify_or_or() {
        let token1: AccessToken = crate::AccessToken::new("a");
        let token2: AccessToken = crate::AccessToken::new("b");
        assert_eq!(
            simplify(&AccessExpression::Or(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Or(vec![
                    AccessExpression::Token(token1.clone()),
                    AccessExpression::Token(token2.clone())
                ])
            ])),
            AccessExpression::Or(vec![
                AccessExpression::Token(token1.clone()),
                AccessExpression::Token(token2.clone())
            ])
        )
    }
    #[test]
    fn simplify_or_and2_and3() {
        #[cfg(feature = "debug_simplify")]
        {
            assert!(!is_implied_by(
                &AccessExpression::And(vec![tok("a"), tok("b"), tok("c")]),
                &[AccessExpression::And(vec![tok("a"), tok("b")])]
            ));
            assert!(is_implied_by(
                &AccessExpression::And(vec![tok("a"), tok("b")]),
                &[AccessExpression::And(vec![tok("a"), tok("b"), tok("c")])],
            ));
        }
        assert_eq!(
            simplify(&AccessExpression::Or(vec![
                AccessExpression::And(vec![tok("a"), tok("b")]),
                AccessExpression::And(vec![tok("a"), tok("b"), tok("c"),])
            ])),
            AccessExpression::And(vec![tok("a"), tok("b"),])
        )
    }
    #[test]
    fn simplify_and_or2_or3() {
        test_with_parsing("(a|b)&(a|b|c)", "a|b");
    }

    #[test]
    fn fuzz1() {
        test_with_parsing("f&b&ca&b&(a|b)", "b&ca&f")
    }
    #[test]
    fn fuzz2() {
        test_with_parsing("b&(b|(a&bb&(a|(a&b)|z))|z)", "b");
    }
    #[test]
    fn fuzz3() {
        test_with_parsing("aa&b&(a|b)&b&c", "aa&b&c");
    }
    #[test]
    fn fuzz4() {
        test_with_parsing(
            "x&(a&\")&(a|a|&(a|a&\")&(a|a|b)&c",
            "a&c&x&\")&(a|a|&(a|a&\"",
        );
    }
    #[test]
    fn fuzz5() {
        // This is tricky to get right.
        // First, some easy ones: y and c are required at the top level.
        // Then, there's a 'b|b' in there by itself, so 'b' is required.
        // Then, there's also a 'a|a', so 'a' is required.
        // Finally, 'auba' and 'bb' only appear in OR clauses with things we already require, so they're never helpful.

        test_with_parsing(
            "y&(a|b)&c&(a|a|b)&(a|a)&(a|bb|a|b)&c&(a|a|b)&(b|auba|b)&c\
                &(a|a|b)&(a|a)&(a|b)&c&(b|a)&(a|a|b|b)&c&(a|a|b)&(b|auba|b)&c&c\
                &(a|a|b)&(b|a|a|b)&(c&(a|b)&c&(b|a)&(a|b|b)&c&(a|a|b)&(a|a)&(a|bb|a|b)\
                &c&(c&(a|a|bb|a)&(a|b|b)&c&(a|a|b)&(b|b)&c&(a|a)&(a|a|b)&c&a)&(a|b)\
                &(b|auba|b)&c&(auba|b)&c&(a|a|a|b)&(a|a)&(a|bb|a|b)&c)&(a|b|b)&c&(a|b)\
                &(a|a|b)&(a|a|b)&c&c",
            "a&b&c&y",
        );
    }
    #[test]
    fn fuzz6() {
        test_with_parsing(
            "y&(a|b)&c&(000000000000|b)&c&(a|a|(b&c&(b|a)&(a)&c&(a|a|b)&c&(a|a|(b&a))&\
            c&(a|a|b)&(b|b)&(b|auba|b)&c&(a|a|b)&c&(a|a|(b&a))&c&(a|a|b)&(b|auba|a|(b&a))&c&\
            (a|a|b)&(b|b)&a))&c&(a|a|b)&(a|a)&(a|b|b|b)&c&(a|a)&(a|b)&c&(a|a|b)&(a|a|b)&c&a",
            "a&c&y&(000000000000|b)",
        );
    }
    #[test]
    #[cfg(feature = "debug_simplify")]
    fn fuzz7_simplify() {
        assert!(!is_implied_by(
            &AccessExpression::And(vec![tok("b"), tok("c")]),
            &[
                tok("bcb"),
                tok("bzfa"),
                AccessExpression::Or(vec![tok("b"), tok("c")])
            ]
        ))
    }
    #[test]
    fn fuzz7() {
        test_with_parsing("(b&c)|(bcb&bzfa&cb&(b|c))", "(b&c)|(bcb&bzfa&cb&(b|c))");
    }
    #[test]
    fn fuzz8() {
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("(af|(s&a&w)|b)&(af|(s&a)|b)&(af|(s&a&(af|w|b)&(af|(s&a)|b)&w)|b)&(af|(s&a)|b)").unwrap());
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("(af|(s&a&w)|b)&(af|(s&a)|b)&(af|(s&a&(af|w|b)&(af|(s&a)|b)&w)|b)").unwrap());
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("(af|(s&a)|b)&(af|(s&a&(af|w|b)&(af|(s&a)|b)&w)|b)").unwrap());
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("(af|(s&a)|b)&(af|(s&a&(af|w|b)&(af|(s&a)|b)&w)|b)").unwrap());
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("(af|(s&a)|b)&(af|b|(s&a&w))").unwrap());
        verify_equivalence(
            &access_expression("(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)").unwrap(),
            &access_expression("af|b|(s&a&w)").unwrap());
        test_with_parsing(
            "(af|(s&a&w)|b)&(af|(s&a&a)|b)&(af|(s&a&(af|(s&a&w)|b)&(af|(s&a&a)|b)&w)|b)&(af|(s&a&a)|b)",
            "(af|b|(a&s&w))",
        );
    }
    #[test]
    fn fuzz8a() {
        test_with_parsing("(af|(s&a&w))&(af|(s&a))", "(af|(a&s&w))");
    }
    #[test]
    fn fuzz9() {
        test_with_parsing("(\"&\")|((a|b)&mkf)|b", "\"&\"|b|((a|b)&mkf)");
    }
    #[test]
    fn fuzz10() {
        test_with_parsing("Y&bbb&(a|(Y&bbb))&(a|(Y&bb/))&(a|b)", "Y&bbb&(a|bb/)&(a|b)");
    }

    #[test]
    fn fuzz11() {
        verify_equivalence(
            &access_expression("b&((a&b)|(b&b)|(b&(c|d)))").unwrap(),
            &access_expression("b").unwrap(),
        );
        test_with_parsing("b&((a&b)|(b&b)|(b&(c|d)))", "b");
    }
    #[test]
    fn fuzz12() {
        test_with_parsing(
            "Sj&(a|(W))&(a|i|\"|\"|(j&W))",
            "Sj&(a|(W))&(a|i|\"|\"|(j&W))",
        );
    }

    #[test]
    fn hit_quadratic_step() {
        test_with_parsing("(a&b&c)|(a&b&d)|(b&c&d)|(a&b)", "(a&b)|(b&c&d)");
    }
    #[test]
    fn or_of_ands() {
        test_with_parsing("a|(a&b)|(a&b&c)|(a&b&c&d)|(a&b&c&d&e)|(a&b&c&d&e&f)", "a");
    }
    #[test]
    fn and_of_ors() {
        test_with_parsing("a&(a|b)&(a|b|c)&(a|b|c|d)&(a|b|c|d|e)&(a|b|c|d|e|f)", "a");
    }

    // Helper to create tokens quickly
    fn tok(name: &str) -> AccessExpression {
        AccessExpression::Token(AccessToken::new(name))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod satisfaction_tests {
    use super::*;
    // Helper to create tokens quickly
    fn tok(name: &str) -> AccessExpression {
        AccessExpression::Token(AccessToken::new(name))
    }

    #[test]
    fn test_implication_logic() {
        let a = tok("a");
        let b = tok("b");
        let w = tok("w");

        // (a & b & w) satisfies (a & b)
        let strong_and = AccessExpression::And(vec![a.clone(), b.clone(), w.clone()]);
        let weak_and = AccessExpression::And(vec![a.clone(), b.clone()]);
        assert!(is_implied_by(&weak_and, &[strong_and]));

        // (a | b) satisfies (a | b | w)
        let strong_or = AccessExpression::Or(vec![a.clone(), b.clone()]);
        let weak_or = AccessExpression::Or(vec![a.clone(), b.clone(), w.clone()]);
        assert!(is_implied_by(&weak_or, &[strong_or]));
    }

    #[test]
    fn test_failing_case_logic() {
        // Test: (af | (s & a & w)) & (af | (s & a))
        // Does the left satisfy the right? Yes. (If you have w, you have the base)
        // Does the right satisfy the left? No. (If you don't have w, you miss the left)
        let af = tok("af");
        let s = tok("s");
        let a = tok("a");
        let w = tok("w");

        let left = AccessExpression::Or(vec![
            af.clone(),
            AccessExpression::And(vec![s.clone(), a.clone(), w.clone()]),
        ]);
        let right = AccessExpression::Or(vec![
            af.clone(),
            AccessExpression::And(vec![s.clone(), a.clone()]),
        ]);

        assert!(
            is_implied_by(&right, &[left.clone()]),
            "Strong should satisfy weak"
        );
        assert!(
            !is_implied_by(&left, &[right.clone()]),
            "Weak should not satisfy strong"
        );
    }
}

#[cfg(test)]
#[cfg(feature = "debug_simplify")]
mod fuzz_crashes {
    use crate::{expression::access_expression, simplify::verify_simplification};

    #[cfg(feature = "debug_simplify")]
    #[test]
    fn fuzz_crashes() {
        let path = std::env::current_dir().unwrap();
        println!("The current directory is {}", path.display());

        let paths = ::std::fs::read_dir("../out/m1/crashes");
        if paths.is_err() {
            return;
        }

        for p in paths.unwrap() {
            let path = p.unwrap().path();
            if path.ends_with("README.txt") {
                continue;
            }
            let expr_str = String::from_utf8(std::fs::read(path).unwrap()).unwrap();
            println!("Parsing as expression: {expr_str}");
            let expr = access_expression(&expr_str).unwrap();
            println!("==== Verifying simplification of '{expr}' ====");
            verify_simplification(&expr);
        }
    }
}
