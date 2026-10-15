//! Name rules and the block list. The list is data, re-read on every call.

use std::path::Path;

/// `^[a-z0-9_]{2,30}$`. Underscore is the only symbol.
pub fn valid(name: &str) -> bool {
    (2..=30).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// True when the name is a line of the block list. A missing file blocks nothing.
pub fn blocked(path: &Path, name: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .any(|line| line == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        assert!(valid("ab"));
        assert!(valid("a_b_9"));
        assert!(valid("_x"));
        assert!(!valid("a"));
        assert!(!valid(&"a".repeat(31)));
        assert!(!valid("Ab"));
        assert!(!valid("a-b"));
        assert!(!valid("a.b"));
    }
}
