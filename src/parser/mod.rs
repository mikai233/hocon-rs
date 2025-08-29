mod array;
mod boolean;
mod comment;
pub mod config_parse_options;
mod error;
mod include;
pub(crate) mod loader;
mod null;
mod number;
mod object;
mod pure;
mod string;
mod substitution;

use crate::parser::array::parse_array;
use crate::parser::boolean::parse_boolean;
use crate::parser::config_parse_options::ConfigParseOptions;
use crate::parser::error::HoconParseError;
use crate::parser::null::parse_null;
use crate::parser::number::parse_number;
use crate::parser::object::{parse_object, parse_root_object};
use crate::parser::string::parse_string;
use crate::parser::substitution::parse_substitution;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::bytes::complete::take_while;
use nom::bytes::tag;
use nom::character::complete::{char, line_ending};
use nom::combinator::{all_consuming, eof, map, peek, value};
use nom::error::context;
use nom::multi::{many_m_n, many1};
use nom::sequence::preceded;
use nom::{IResult, Parser};
use std::cell::RefCell;

type R<'a, T> = IResult<&'a str, T, HoconParseError<'a>>;

thread_local! {
    pub(crate) static CONFIG: RefCell<ConfigParseOptions> = RefCell::new(ConfigParseOptions::default());
}

pub(crate) fn parse(input: &str, parse_options: ConfigParseOptions) -> R<'_, RawObject> {
    CONFIG.set(parse_options);
    all_consuming(preceded(
        hocon_multi_space0,
        alt((parse_object, parse_root_object)),
    ))
    .parse_complete(input)
}

#[inline]
fn is_hocon_whitespace(c: char) -> bool {
    match c {
        '\u{001C}' | '\u{001D}' | '\u{001E}' | '\u{001F}' => true,
        _ => c.is_whitespace(),
    }
}

#[inline]
fn is_hocon_horizontal_whitespace(c: char) -> bool {
    is_hocon_whitespace(c) && c != '\r' && c != '\n'
}

#[inline]
fn hocon_multi_space0(input: &str) -> R<'_, &str> {
    take_while(is_hocon_whitespace).parse_complete(input)
}

#[inline]
fn hocon_horizontal_space0(input: &str) -> R<'_, &str> {
    take_while(is_hocon_horizontal_whitespace).parse_complete(input)
}

#[inline]
fn horizontal_ending(input: &str) -> R<'_, &str> {
    preceded(
        hocon_horizontal_space0,
        alt((
            peek(tag(",")),
            peek(tag("}")),
            peek(tag("]")),
            peek(tag("//")),
            peek(tag("#")),
            peek(line_ending),
            peek(eof),
        )),
    )
    .parse_complete(input)
}

fn parse_value(input: &str) -> R<'_, RawValue> {
    let (remainder, (value,)) = (map(
        many1((
            hocon_horizontal_space0,
            alt((
                context("parse_boolean", parse_boolean.map(RawValue::boolean)),
                context("parse_null", parse_null.map(|_| RawValue::null())),
                context("parse_number", parse_number.map(RawValue::number)),
                context(
                    "parse_substitution",
                    parse_substitution.map(RawValue::substitution),
                ),
                context("parse_string", parse_string.map(RawValue::String)),
                context("parse_array", parse_array.map(RawValue::Array)),
                context("parse_object", parse_object.map(RawValue::Object)),
            )),
            hocon_horizontal_space0,
        )),
        |mut values| {
            if values.len() == 1 {
                values.remove(0).1
            } else {
                RawValue::concat(values.into_iter().map(|v| v.1))
            }
        },
    ),)
        .parse_complete(input)?;
    Ok((remainder, value))
}

#[inline(always)]
fn next_element_whitespace(input: &str) -> R<'_, ()> {
    value((), (hocon_multi_space0, many_m_n(0, 1, char(',')))).parse_complete(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::string::parse_key;
    use crate::parser::{next_element_whitespace, parse_value};
    use crate::raw::raw_string::RawString;
    use crate::raw::raw_value::RawValue;

    #[test]
    fn test_next_element_whitespace() {
        let (r, _) = next_element_whitespace("  , hello = world").unwrap();
        assert_eq!(r, " hello = world");
        let (r, _) = next_element_whitespace("  ,, hello = world").unwrap();
        assert_eq!(r, ", hello = world");
    }

    #[test]
    fn test_parse_value() {
        let (remainder, result) = parse_value("\"world\"}").unwrap();
        assert_eq!(
            result,
            RawValue::String(RawString::QuotedString("world".to_string()))
        );
        assert_eq!(remainder, "}");
        let (r, o) = parse_value("true false ${?a}").unwrap();
        println!("{}={}", r, o);
        let (r, o) = parse_key("a = true false ${}").unwrap();
        println!("{}={}", r, o);
    }
}
