mod include;
mod number;
mod parser;
mod read;
mod string;

#[inline]
fn is_hocon_whitespace(c: char) -> bool {
    match c {
        '\u{001C}' | '\u{001D}' | '\u{001E}' | '\u{001F}' => true,
        _ => c.is_whitespace(),
    }
}

#[inline]
fn is_hocon_horizontal_whitespace(c: char) -> bool {
    is_hocon_whitespace(c) && c != '\r' && c != '\n'
}

#[inline]
fn is_horizontal_ending(s: &str) -> bool {
    for (i, c) in s.char_indices() {
        if !is_hocon_horizontal_whitespace(c) {
            let remaining = &s[i..];
            return remaining.is_empty()
                || matches!(remaining.chars().next(), Some(',') | Some('}') | Some(']'))
                || remaining.starts_with("//")
                || remaining.starts_with('#');
        }
    }
    true
}

#[inline]
fn whitespace(s: &str) -> (&str, &str) {
    let split_index = s.find(|c: char| !is_hocon_whitespace(c)).unwrap_or(s.len());
    s.split_at(split_index)
}

#[inline]
fn horizontal_whitespace(s: &str) -> (&str, &str) {
    let split_index = s
        .find(|c: char| !is_hocon_horizontal_whitespace(c))
        .unwrap_or(s.len());
    s.split_at(split_index)
}
