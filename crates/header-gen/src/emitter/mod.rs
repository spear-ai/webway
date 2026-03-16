pub mod mapping;
pub mod proto;
pub mod rust_structs;

// ─── Shared utilities ────────────────────────────────────────────────────────

/// Convert a CamelCase or ALLCAPS identifier to snake_case.
///
/// Examples:
///   `TrackId`       → `track_id`
///   `SPSBFCommand`  → `spsbf_command`
///   `XMLParser`     → `xml_parser`
pub(crate) fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else if c.is_uppercase() {
            if i > 0 && !out.ends_with('_') {
                let prev = chars[i - 1];
                let next_is_lower = chars.get(i + 1).map(|c| c.is_lowercase()).unwrap_or(false);
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next_is_lower)
                {
                    out.push('_');
                }
            }
            out.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// Escape Rust reserved words by appending `_field`.
pub(crate) fn escape_keyword(s: &str) -> String {
    match s {
        "type" | "ref" | "fn" | "mod" | "use" | "move" | "match" | "loop" | "if" | "else"
        | "while" | "for" | "in" | "let" | "mut" | "const" | "static" | "return" | "break"
        | "continue" | "struct" | "enum" | "trait" | "impl" | "where" | "pub" | "super"
        | "self" | "Self" | "crate" | "extern" | "unsafe" | "async" | "await" | "dyn"
        | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override" | "priv"
        | "try" | "typeof" | "unsized" | "virtual" | "yield" => format!("{s}_field"),
        _ => s.to_owned(),
    }
}

/// Sanitise a raw C identifier into a valid, escaped Rust field name.
pub(crate) fn field_name(raw: &str) -> String {
    escape_keyword(&snake_case(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_simple() {
        assert_eq!(snake_case("TrackId"), "track_id");
    }

    #[test]
    fn snake_case_acronym_prefix() {
        assert_eq!(snake_case("SPSBFCommand"), "spsbf_command");
    }

    #[test]
    fn snake_case_all_lower() {
        assert_eq!(snake_case("foo"), "foo");
    }

    #[test]
    fn snake_case_preserves_existing() {
        assert_eq!(snake_case("foo_bar"), "foo_bar");
    }

    #[test]
    fn escape_type_keyword() {
        assert_eq!(escape_keyword("type"), "type_field");
    }

    #[test]
    fn escape_plain_ident() {
        assert_eq!(escape_keyword("count"), "count");
    }
}
