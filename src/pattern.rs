pub fn matches(pattern: &str, value: &str) -> bool {
    fn inner(pattern: &[u8], value: &[u8]) -> bool {
        match pattern {
            [] => value.is_empty(),
            [b'*', rest @ ..] => {
                inner(rest, value)
                    || (!value.is_empty() && value[0] != b'/' && inner(pattern, &value[1..]))
            }
            [b'?', rest @ ..] => !value.is_empty() && value[0] != b'/' && inner(rest, &value[1..]),
            [first, rest @ ..] => {
                !value.is_empty() && *first == value[0] && inner(rest, &value[1..])
            }
        }
    }
    inner(pattern.as_bytes(), value.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_does_not_cross_directories() {
        assert!(matches(
            "patches/embedding/*",
            "patches/embedding/fork.json"
        ));
        assert!(!matches(
            "patches/embedding/*",
            "patches/embedding/nested/file"
        ));
    }

    #[test]
    fn matches_literals_and_single_characters() {
        assert!(matches("src/main.rs", "src/main.rs"));
        assert!(matches("patches/000?.patch", "patches/0001.patch"));
        assert!(!matches("patches/000?.patch", "patches/0010.patch"));
        assert!(!matches("src/*.rs", "src/app/mod.rs"));
    }
}
