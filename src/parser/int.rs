use crate::parser::{hocon_horizontal_multi_space0, R};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{i64, line_ending};
use nom::combinator::{eof, peek};
use nom::error::context;
use nom::sequence::{pair, terminated};
use nom::Parser;

pub(crate) fn parse_int(input: &str) -> R<'_, i64> {
    context(
        "i64 number expect",
        terminated(
            i64,
            pair(
                hocon_horizontal_multi_space0,
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
    use crate::parser::int::parse_int;

    #[test]
    fn test_int() {
        assert!(parse_int("1.0").is_err());
        let (r, o) = parse_int("1234").unwrap();
        assert!(r.is_empty());
        assert_eq!(o, 1234);
        let (r, o) = parse_int("1234, ").unwrap();
        assert_eq!(r, ", ");
        assert_eq!(o, 1234);
        let (r, o) = parse_int("1234 , ").unwrap();
        assert_eq!(r, ", ");
        assert_eq!(o, 1234);
        let (r, o) = parse_int("1234\n , ").unwrap();
        assert_eq!(r, "\n , ");
        assert_eq!(o, 1234);
    }
}