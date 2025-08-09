use crate::parser::R;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, space0};
use nom::combinator::{eof, peek};
use nom::error::context;
use nom::number::complete::double;
use nom::sequence::{pair, terminated};
use nom::Parser;

pub(crate) fn parse_float(input: &str) -> R<'_, f64> {
    context(
        "f64 number expect",
        terminated(
            double,
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
    use crate::parser::float::parse_float;

    #[test]
    fn test_float() {
        let (r, o) = parse_float("-1.1314").unwrap();
        assert!(r.is_empty());
        assert_eq!(o, -1.1314);
        let (r, o) = parse_float("1234").unwrap();
        assert!(r.is_empty());
        assert_eq!(o, 1234f64);
        let (r, o) = parse_float("1234, ").unwrap();
        assert_eq!(r, ", ");
        assert_eq!(o, 1234f64);
        let (r, o) = parse_float("1234 , ").unwrap();
        assert_eq!(r, ", ");
        assert_eq!(o, 1234f64);
        let (r, o) = parse_float("1234\n , ").unwrap();
        assert_eq!(r, "\n , ");
        assert_eq!(o, 1234f64);
        assert!(parse_float("4 5.0").is_err());
    }
}