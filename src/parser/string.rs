use crate::parser::{is_hocon_whitespace, R};
use nom::bytes::complete::{tag, take_till1};
use nom::error::context;
use nom::sequence::delimited;
use nom::{AsChar, Input, Parser};

const FORBIDDEN_CHARACTERS: [char; 19] = ['$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\'];

pub(crate) fn unquoted_string(input: &str) -> R<&str> {
    take_till1(|c: char| {
        is_hocon_whitespace(c) || FORBIDDEN_CHARACTERS.contains(&c)
    }).parse(input)
}

pub(crate) fn quoted_string(input: &str) -> R<&str> {
    delimited(context("\"", tag("\"")), parse_str, context("\"", tag("\""))).parse(input)
}

fn parse_str(input: &str) -> R<&str> {
    input.split_at_position_complete(|c| !(c.is_alphanum() || c == '.'))
}

#[cfg(test)]
mod tests {
    use crate::parser::string::quoted_string;

    #[test]
    fn test_quoted_string() {
        let (remainder, result) = quoted_string("\"world\"").unwrap();
        assert_eq!(remainder, "");
        assert_eq!(result, "world");
        let (remainder, result) = quoted_string("\"world\"112233").unwrap();
        assert_eq!(remainder, "112233");
        assert_eq!(result, "world");
    }
}