use crate::parser::{horizontal_ending, R};
use nom::branch::alt;
use nom::character::complete::{char, digit1};
use nom::combinator::{opt, recognize};
use nom::sequence::{pair, terminated};
use nom::Parser;
use serde_json::Number;
use std::str::FromStr;

/// Parses a numeric literal in HOCON, including integers, decimals,
/// and numbers in scientific notation.
///
/// The parser does not convert the value to a numeric type; instead it returns
/// the original string slice that matches the number.
///
/// It supports:
/// - An optional leading minus sign (`-`)
/// - Digits before and/or after a decimal point (`.`)
/// - Numbers starting with a decimal point (e.g., `.123`)
/// - An optional scientific exponent part (`e` or `E`), with optional sign
///
/// # Parameters
///
/// * `input` - The input string slice to be parsed
///
/// # Returns
///
/// An [`IResult`] with:
/// - `Ok((remaining, number_str))` if a number was successfully parsed
///   where `number_str` is the matched substring
/// - `Err(HoconParseError)` if the input is not a valid number
///
/// # Examples
///
/// ```rust
/// use your_crate::number_str; // adjust path
///
/// assert_eq!(number_str("123"), Ok(("", "123")));
/// assert_eq!(number_str("-45.67e+2"), Ok(("", "-45.67e+2")));
/// assert_eq!(number_str(".5 rest"), Ok((" rest", ".5")));
/// assert!(number_str("abc").is_err());
/// ```
///
/// [`IResult`]: nom::IResult
#[inline]
fn number_str(input: &str) -> R<'_, &str> {
    recognize((
        // Optional minus sign.
        opt(char('-')),
        // Either a standard decimal or a number starting with a decimal point.
        alt((
            // Case 1: Standard decimal like `123` or `123.45`.
            recognize((digit1, opt(pair(char('.'), digit1)))),
            // Case 2: Starting with `.`, like `.123`.
            recognize(pair(char('.'), digit1)),
        )),
        // Optional exponent part.
        opt((
            alt((char('e'), char('E'))),
            opt(alt((char('+'), char('-')))),
            digit1,
        )),
    ))
    .parse_complete(input)
}

#[inline]
pub(crate) fn parse_number(input: &str) -> R<'_, Number> {
    let (remainder, num_str) = terminated(
        number_str,
        horizontal_ending,
    )
    .parse_complete(input)?;
    match Number::from_str(num_str) {
        Ok(number) => Ok((remainder, number)),
        Err(error) => {
            let error = crate::error::Error::SerdeJsonError(error);
            Err(nom::Err::Error(crate::parser::HoconParseError::Other(
                error,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64;

    use rstest::rstest;

    use crate::parser::number::parse_number;

    #[rstest]
    #[case("1.0", serde_json::Number::from_f64(1.0), "")]
    #[case("-999", serde_json::Number::from_i128(-999), "")]
    #[case("233", serde_json::Number::from_i128(233), "")]
    #[case("-233.233", serde_json::Number::from_f64(-233.233), "")]
    #[case("1.7976931348623157e+308", serde_json::Number::from_f64(f64::MAX), "")]
    #[case("-1.7976931348623157e+308", serde_json::Number::from_f64(f64::MIN), "")]
    #[case("-1E-1", serde_json::Number::from_f64(-0.1), "")]
    #[case("-1E-1,", serde_json::Number::from_f64(-0.1), ",")]
    #[case("-1E-1,\r\n", serde_json::Number::from_f64(-0.1), ",\r\n")]
    #[case("-1E-1 \n", serde_json::Number::from_f64(-0.1), "\n")]
    #[case("1.0 \n", serde_json::Number::from_f64(1.0), "\n")]
    #[case("1.0 }\n", serde_json::Number::from_f64(1.0), "}\n")]
    #[case("1.0 ]", serde_json::Number::from_f64(1.0), "]")]
    fn test_valid_number(
        #[case] input: &str,
        #[case] expected_result: Option<serde_json::Number>,
        #[case] expected_rest: &str,
    ) {
        let result = parse_number(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(Some(parsed), expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case("-1e1q")]
    #[case("foo12")]
    #[case("12 hello")]
    fn test_invalid_number(#[case] input: &str) {
        let result = parse_number(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }
}
