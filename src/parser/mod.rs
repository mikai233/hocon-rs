use memchr::memchr;

mod array;
mod comment;
mod include;
pub(crate) mod loader;
mod number;
mod object;
pub mod parser;
pub mod read;
mod string;
mod substitution;

#[inline]
fn is_hocon_whitespace(c: char) -> bool {
    match c {
        '\u{001C}' | '\u{001D}' | '\u{001E}' | '\u{001F}' => true,
        _ => c.is_whitespace(),
    }
}

#[inline]
fn is_hocon_horizontal_whitespace(c: char) -> bool {
    is_hocon_whitespace(c) && c != '\n'
}

#[inline]
fn token_horizontal_ending_position(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    for (i, c) in s.char_indices() {
        if !is_hocon_horizontal_whitespace(c) {
            let remaining = &s[i..];
            let ending =
                // safe: we only check ASCII delimiters ',', '}', ']', '\n'
                matches!(remaining.as_bytes()[0], b',' | b'}' | b']' | b'\n')
                    || remaining.starts_with("//")
                    || remaining.starts_with('#')
                    || remaining.starts_with("\r\n");
            if ending {
                return Some(i);
            }
        }
    }
    None
}

#[inline]
fn leading_whitespace(s: &str) -> (&str, &str) {
    let split_index = s.find(|c: char| !is_hocon_whitespace(c)).unwrap_or(s.len());
    s.split_at(split_index)
}

#[inline]
fn leading_horizontal_whitespace(s: &str) -> (&str, &str) {
    let split_index = s
        .find(|c: char| !is_hocon_horizontal_whitespace(c))
        .unwrap_or(s.len());
    s.split_at(split_index)
}

fn find_line_break(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = memchr(b'\n', buf) {
        if pos > 0 && buf[pos - 1] == b'\r' {
            // Windows \r\n
            Some((pos - 1, 2))
        } else {
            // Unix/macOS \n
            Some((pos, 1))
        }
    } else {
        None
    }
}
