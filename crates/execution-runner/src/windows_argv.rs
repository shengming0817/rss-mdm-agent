// ref: rust-lang/rust library/std/src/sys/args/windows.rs (CRT backslash/quote convention).
pub(crate) fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        out.push(character);
        slashes = 0;
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}
// Windows command-line encoding, not a shell language. Every argv element is separately quoted.
#[cfg(test)]
mod tests {
    #[test]
    fn literal_arguments_keep_spaces_quotes_and_trailing_backslashes() {
        assert_eq!(super::quote("plain"), "\"plain\"");
        assert_eq!(super::quote("a b"), "\"a b\"");
        assert_eq!(super::quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(super::quote("C:\\folder\\"), "\"C:\\folder\\\\\"");
        assert_eq!(
            super::quote("$(Write-Host secret); & x"),
            "\"$(Write-Host secret); & x\""
        );
    }
}
