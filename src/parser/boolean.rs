use crate::parser::R;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, space0};
use nom::combinator::{eof, peek, value};
use nom::error::context;
use nom::sequence::{pair, terminated};
use nom::Parser;

pub(crate) fn parse_boolean(input: &str) -> R<'_, bool> {
    context(
        "boolean literal (expected 'true' or 'false')",
        terminated(
            alt(
                (
                    value(true, tag("true")),
                    value(false, tag("false")),
                )
            ),
            pair(
                space0,
                alt(
                    (
                        peek(tag(",")),
                        peek(line_ending),
                        peek(eof)
                    )
                ),
            ),
        ),
    )
        .parse(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::boolean::parse_boolean;

    #[test]
    fn test_boolean() {
        let (r, v) = parse_boolean("true").unwrap();
        assert_eq!(r, "");
        assert_eq!(v, true);
        let (r, v) = parse_boolean("false").unwrap();
        assert_eq!(r, "");
        assert_eq!(v, false);
        let (r, v) = parse_boolean("false\n").unwrap();
        assert_eq!(r, "\n");
        assert_eq!(v, false);
        let (r, v) = parse_boolean("false \r\n").unwrap();
        assert_eq!(r, "\r\n");
        assert_eq!(v, false);
        assert!(parse_boolean("true false").is_err());
        assert!(parse_boolean("trueX").is_err());
    }
}