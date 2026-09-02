use super::*;
use proptest::prelude::*;

/// Reverse the `!`-escaping that `escape_like_wildcards` applies, per SQL
/// `LIKE ... ESCAPE '!'` semantics: a `!` consumes and literalises the next
/// character. Used to prove escaping is lossless.
fn unescape_like(escaped: &str) -> String {
    let mut out = String::new();
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        if c == '!' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

proptest! {
    /// Escaping any string is lossless (de-escaping recovers the original) and
    /// leaves no live `%`/`_` wildcard: every one is preceded by the `!` escape.
    #[test]
    fn escape_like_wildcards_is_lossless_and_neutralises_wildcards(s in ".*") {
        let escaped = escape_like_wildcards(&s);
        prop_assert_eq!(unescape_like(&escaped), s.clone());

        let chars: Vec<char> = escaped.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c == '%' || c == '_' {
                prop_assert!(
                    i > 0 && chars[i - 1] == '!',
                    "wildcard {:?} at {} not escaped in {:?}",
                    c, i, escaped
                );
            }
        }
    }

    /// Truncation never panics on a multi-byte boundary, returns a prefix of the
    /// input, and respects the byte budget.
    #[test]
    fn truncate_to_char_boundary_never_panics(s in ".*", n in 0usize..64) {
        let t = truncate_to_char_boundary(&s, n);
        prop_assert!(t.len() <= n);
        prop_assert!(s.starts_with(t));
        // The result is always valid UTF-8 (it is a &str slice), the implicit
        // guarantee that the raw `&s[..n]` slice would violate mid-codepoint.
    }
}
