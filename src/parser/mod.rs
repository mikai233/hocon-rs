mod array;
mod comment;
mod include;
pub(crate) mod loader;
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
