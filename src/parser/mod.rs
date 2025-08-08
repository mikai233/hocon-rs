mod string;
mod include;
mod object;
mod array;
mod boolean;
mod null;
mod comment;
mod int;
mod float;
mod substitution;

use crate::error::Error;
use crate::parser::array::array;
use crate::parser::boolean::parse_boolean;
use crate::parser::float::parse_float;
use crate::parser::include::parse_include;
use crate::parser::int::parse_int;
use crate::parser::null::parse_null;
use crate::parser::object::object;
use crate::parser::string::parse_hocon_string;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::bytes::complete::{take_while, take_while1};
use nom::character::complete::char;
use nom::combinator::{all_consuming, map, value};
use nom::multi::many_m_n;
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
    value(RawObject::default(), all_consuming(hocon_multi_space0)).parse(input)
}

fn is_hocon_whitespace(c: char) -> bool {
    match c {
        '\u{001C}' |
        '\u{001D}' |
        '\u{001E}' |
        '\u{001F}' => true,
        _ => c.is_whitespace()
    }
}

fn is_hocon_horizontal_whitespace(c: char) -> bool {
    is_hocon_whitespace(c) && c != '\r' && c != '\n'
}

fn hocon_multi_space0(input: &str) -> R<&str> {
    take_while(is_hocon_whitespace).parse_complete(input)
}

fn hocon_multi_space1(input: &str) -> R<&str> {
    take_while1(is_hocon_whitespace).parse_complete(input)
}

fn hocon_horizontal_multi_space0(input: &str) -> R<&str> {
    take_while(is_hocon_horizontal_whitespace).parse_complete(input)
}

fn hocon_horizontal_multi_space1(input: &str) -> R<&str> {
    take_while1(is_hocon_horizontal_whitespace).parse_complete(input)
}

fn parse_value(input: &str) -> R<RawValue> {
    alt(
        (
            map(parse_null, |_| RawValue::Null),
            map(parse_include, RawValue::Inclusion),
            map(parse_boolean, RawValue::Boolean),
            map(parse_int, RawValue::Int),
            map(parse_float, RawValue::Float),
            map(array, RawValue::Array),
            map(object, RawValue::Object),
            map(parse_hocon_string, RawValue::String),
        )
    ).parse(input)
}

// fn parse_simple_value(input: &str) -> R<RawValue> {
//     alt((
//         parse_quoted_string,
//         parse_multiline_string,
//         parse_boolean,
//         parse_null,
//         parse_int,
//         parse_float,
//         parse_unquoted_string,
//     )).parse(input)
// }

fn next_element_whitespace(input: &str) -> R<()> {
    value((), (hocon_multi_space0, many_m_n(0, 1, char(',')))).parse(input)
}

pub(crate) fn load_conf(name: impl AsRef<str>) -> crate::Result<String> {
    let conf = std::fs::read_to_string(format!("resources/{}.conf", name.as_ref()))?;
    Ok(conf)
}

#[cfg(test)]
mod tests {
    use crate::parser::{parse_object, parse_value};
    use crate::raw::raw_string::RawString;
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
        assert_eq!(result, RawValue::String(RawString::QuotedString("world".to_string())));
        assert_eq!(remainder, "}");
    }
}