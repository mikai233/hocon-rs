use crate::parser::{R, hocon_horizontal_multi_space0};
use nom::Parser;
use nom::branch::alt;
use nom::bytes::tag;
use nom::character::complete::{char, digit1, line_ending};
use nom::combinator::{eof, opt, peek, recognize};
use nom::error::ParseError;
use nom::sequence::{pair, terminated};
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

pub(crate) fn parse_number(input: &str) -> R<'_, Number> {
    let (remainder, num_str) = terminated(
        number_str,
        pair(
            hocon_horizontal_multi_space0,
            alt((peek(tag(",")), peek(line_ending), peek(eof))),
        ),
    )
    .parse_complete(input)?;
    match Number::from_str(num_str) {
        Ok(number) => Ok((remainder, number)),
        Err(_) => {
            //TODO Error
            let err = nom_language::error::VerboseError::from_error_kind(
                input,
                nom::error::ErrorKind::Digit,
            );
            Err(nom::Err::Error(crate::parser::HoconParseError::Nom(err)))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::number::parse_number;

    #[test]
    fn test_parse_number() {
        let (_, num) = parse_number("1.0").unwrap();
        println!("{}", num);
        assert!(parse_number("1.0.").is_err());
        assert!(parse_number("1.0 hello").is_err());
        assert!(parse_number("1e1 hello").is_err());
        let (_, num) = parse_number("1234567,").unwrap();
        println!("{}", num);
        let (_, num) = parse_number("0.1  \r\n").unwrap();
        println!("{}", num);
        let (_, num) = parse_number("-0.3e100").unwrap();
        println!("{}", num);
        let (_, num) = parse_number("3E+100").unwrap();
        println!("{}", num);
    }
}
