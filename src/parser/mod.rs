mod string;
mod include;
mod object;
mod array;
mod boolean;
mod null;

use crate::error::Error;
use crate::parser::array::array;
use crate::parser::boolean::boolean;
use crate::parser::include::include;
use crate::parser::null::null;
use crate::parser::object::object;
use crate::parser::string::{quoted_string, unquoted_string};
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::bytes::complete::take_while;
use nom::character::complete::{char, i64};
use nom::combinator::{all_consuming, map, value};
use nom::multi::many_m_n;
use nom::number::complete::double;
use nom::{IResult, Parser};
use nom_language::error::{convert_error, VerboseError};

type R<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

pub fn parse_object(input: &str) -> crate::Result<RawObject> {
    let (_, object) = alt((empty_content, object))
        .parse(input)
        .map_err(|e| {
            match e {
                nom::Err::Incomplete(_) => unreachable!(),
                nom::Err::Error(e) => {
                    Error::ParseError(convert_error(input, e).replace("\\n", "\n"))
                }
                nom::Err::Failure(e) => {
                    Error::ParseError(convert_error(input, e).replace("\\n", "\n"))
                }
            }
        })?;
    Ok(object)
}

fn empty_content(input: &str) -> R<RawObject> {
    value(RawObject::default(), all_consuming(whitespace)).parse(input)
}

fn is_hocon_whitespace(c: char) -> bool {
    c.is_whitespace()
        || c == '\t'
        || c == '\n'
        || c == '\u{000B}'
        || c == '\u{000C}'
        || c == '\r'
        || c == '\u{001C}'
        || c == '\u{001D}'
        || c == '\u{001E}'
        || c == '\u{001F}'
}

fn whitespace(input: &str) -> R<()> {
    let (input, _) = take_while(is_hocon_whitespace)(input)?;
    Ok((input, ()))
}

fn parse_value(input: &str) -> R<RawValue> {
    alt(
        (
            map(null, |_| RawValue::Null),
            map(include, RawValue::Inclusion),
            map(boolean, RawValue::Boolean),
            map(i64, RawValue::Int),
            map(double, RawValue::Float),
            map(array, RawValue::Array),
            map(object, RawValue::Object),
            map(unquoted_string, |v| RawValue::UnquotedString(v.to_string())),
            map(quoted_string, |v| RawValue::String(v.to_string())),
        )
    ).parse(input)
}

fn next_element_whitespace(input: &str) -> R<()> {
    value((), (whitespace, many_m_n(0, 1, char(',')))).parse(input)
}

pub(crate) fn load_conf(name: impl AsRef<str>) -> crate::Result<String> {
    let conf = std::fs::read_to_string(format!("resources/{}.conf", name.as_ref()))?;
    Ok(conf)
}

#[cfg(test)]
mod tests {
    use crate::parser::{parse_object, parse_value};
    use crate::raw::raw_value::RawValue;

    #[test]
    fn test1() -> crate::Result<()> {
        let demo = std::fs::read_to_string("resources/demo.conf")?;
        match parse_object(&demo) {
            Ok(_) => {}
            Err(e) => {
                println!("{e}");
            }
        }
        // let obj = parse_object(&demo)?;
        Ok(())
    }

    #[test]
    fn test_parse_value() {
        let (remainder, result) = parse_value("\"world\"}").unwrap();
        assert_eq!(result, RawValue::String("world".to_string()));
        assert_eq!(remainder, "}");
    }
}