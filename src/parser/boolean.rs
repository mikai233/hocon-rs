use crate::parser::{R, hocon_horizontal_space0};
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::line_ending;
use nom::combinator::{eof, peek, value};
use nom::error::context;
use nom::sequence::{pair, terminated};

pub(crate) fn parse_boolean(input: &str) -> R<'_, bool> {
    context(
        "boolean literal (expected 'true' or 'false')",
        terminated(
            alt((value(true, tag("true")), value(false, tag("false")))),
            pair(
                hocon_horizontal_space0,
                alt((peek(tag(",")), peek(tag("}")), peek(line_ending), peek(eof))),
            ),
        ),
    )
    .parse(input)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::boolean::parse_boolean;

    #[rstest]
    #[case("true", true, "")]
    #[case("false", false, "")]
    #[case("true \t", true, "")]
    #[case("true ,", true, ",")]
    #[case("true }", true, "}")]
    fn test_valid_boolean(
        #[case] input: &str,
        #[case] expected_result: bool,
        #[case] expected_rest: &str,
    ) {
        let result = parse_boolean(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed, expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case("True")]
    #[case("TRUE")]
    #[case("FALSE")]
    #[case("true1")]
    #[case("true 1")]
    #[case("False")]
    #[case("falseX")]
    fn test_invalid_boolean(#[case] input: &str) {
        let result = parse_boolean(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }
}
