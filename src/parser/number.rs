use crate::parser::{hocon_horizontal_multi_space0, R};
use nom::branch::alt;
use nom::bytes::tag;
use nom::character::complete::{char, digit1, line_ending};
use nom::combinator::{eof, opt, peek, recognize};
use nom::error::ParseError;
use nom::sequence::{pair, terminated};
use nom::Parser;
use serde_json::Number;
use std::str::FromStr;

fn number_str(input: &str) -> R<'_, &str> {
    recognize(
        (
            opt(char('-')),
            alt(
                (
                    // 小数
                    recognize(
                        (
                            digit1,
                            opt(pair(char('.'), digit1))
                        ),
                    ),
                    // 只含小数点的情况：.123
                    recognize(pair(char('.'), digit1)),
                ),
            ),
            // 科学计数法部分
            opt(
                (
                    alt((char('e'), char('E'))),
                    opt(alt((char('+'), char('-')))),
                    digit1
                ),
            )
        ),
    ).parse_complete(input)
}

pub(crate) fn parse_number(input: &str) -> R<'_, Number> {
    let (remainder, num_str) = terminated(
        number_str,
        pair(
            hocon_horizontal_multi_space0,
            alt(
                (
                    peek(tag(",")),
                    peek(line_ending),
                    peek(eof),
                ),
            ),
        ),
    ).parse_complete(input)?;
    match Number::from_str(num_str) {
        Ok(number) => {
            Ok((remainder, number))
        }
        Err(_) => {
            let err = nom_language::error::VerboseError::from_error_kind(input, nom::error::ErrorKind::Digit);
            Err(nom::Err::Error(err))
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