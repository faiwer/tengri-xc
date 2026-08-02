//! Helpers for building `LIKE` / `ILIKE` search patterns from user input.

/// Build a case-insensitive "contains" pattern for a `… ILIKE $ ESCAPE '\'`
/// clause: wraps `needle` in `%…%` and escapes the `LIKE` metacharacters
/// (`\`, `%`, `_`) so user input matches literally instead of acting as
/// wildcards (a query of `foo%` finds the literal `foo%`, not `foo<anything>`).
///
/// The consuming clause MUST spell out `ESCAPE '\'` (backslash) — that's the
/// escape character used here.
///
/// ```ignore
/// let pat = like_contains(needle);
/// q.and_where("name ILIKE $ ESCAPE '\\'", (pat,));
/// ```
pub fn like_contains(needle: &str) -> String {
    // `\` must be escaped first, or it would consume the backslashes we add
    // for `%` / `_`.
    let escaped = needle
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_plain_input() {
        assert_eq!(like_contains("foo"), "%foo%");
    }

    #[test]
    fn escapes_wildcards() {
        assert_eq!(like_contains("50%_off"), "%50\\%\\_off%");
    }

    #[test]
    fn escapes_backslash_before_wildcards() {
        // A literal backslash becomes `\\`; the `%` after it is escaped
        // independently, not consumed by the backslash's escape.
        assert_eq!(like_contains("a\\%"), "%a\\\\\\%%");
    }
}
