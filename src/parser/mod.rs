mod string;
mod include;
mod object;
mod array;
mod boolean;
mod null;
mod comment;
mod substitution;
mod config_parse_options;
mod number;

use crate::parser::array::parse_array;
use crate::parser::boolean::parse_boolean;
use crate::parser::config_parse_options::ConfigParseOptions;
use crate::parser::null::parse_null;
use crate::parser::number::parse_number;
use crate::parser::object::{parse_object, parse_root_object};
use crate::parser::string::parse_string;
use crate::parser::substitution::parse_substitution;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::bytes::complete::{take_while, take_while1};
use nom::character::complete::char;
use nom::combinator::{all_consuming, map, value};
use nom::error::context;
use nom::multi::{many1, many_m_n};
use nom::sequence::preceded;
use nom::{IResult, Parser};
use nom_language::error::VerboseError;
use std::cell::RefCell;

type R<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

thread_local! {
    pub(crate) static CONFIG: RefCell<ConfigParseOptions> = RefCell::new(ConfigParseOptions::default());
}

pub fn parse(input: &str) -> R<'_, RawObject> {
    all_consuming(
        preceded(
            hocon_multi_space0,
            alt(
                (
                    parse_object,
                    parse_root_object,
                )
            ),
        )
    )
        .parse_complete(input)
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

fn hocon_multi_space0(input: &str) -> R<'_, &str> {
    take_while(is_hocon_whitespace).parse_complete(input)
}

fn hocon_multi_space1(input: &str) -> R<'_, &str> {
    take_while1(is_hocon_whitespace).parse_complete(input)
}

fn hocon_horizontal_multi_space0(input: &str) -> R<'_, &str> {
    take_while(is_hocon_horizontal_whitespace).parse_complete(input)
}

fn hocon_horizontal_multi_space1(input: &str) -> R<'_, &str> {
    take_while1(is_hocon_horizontal_whitespace).parse_complete(input)
}

fn parse_value(input: &str) -> R<'_, RawValue> {
    let (remainder, (value, )) = (
        map(
            many1(
                (
                    hocon_horizontal_multi_space0,
                    alt(
                        (
                            context("parse_boolean", parse_boolean.map(RawValue::boolean)),
                            context("parse_null", parse_null.map(|_| RawValue::null())),
                            context("parse_number", parse_number.map(RawValue::number)),
                            context("parse_substitution", parse_substitution.map(RawValue::substitution)),
                            context("parse_string", parse_string.map(RawValue::String)),
                            context("parse_array", parse_array.map(RawValue::Array)),
                            context("parse_object", parse_object.map(RawValue::Object)),
                        ),
                    ),
                    hocon_horizontal_multi_space0,
                )
            ),
            |mut values| {
                if values.len() == 1 {
                    values.remove(0).1
                } else {
                    RawValue::concat(values.into_iter().map(|v| v.1))
                }
            },
        ),
    ).parse_complete(input)?;
    Ok((remainder, value))
}

fn next_element_whitespace(input: &str) -> R<'_, ()> {
    value((), (hocon_multi_space0, many_m_n(0, 1, char(',')))).parse_complete(input)
}

pub(crate) fn load_conf(name: impl AsRef<str>) -> crate::Result<String> {
    let conf = std::fs::read_to_string(format!("resources/{}.conf", name.as_ref()))?;
    Ok(conf)
}

#[cfg(test)]
mod tests {
    use crate::parser::string::parse_key;
    use crate::parser::{next_element_whitespace, parse_value};
    use crate::raw::raw_string::RawString;
    use crate::raw::raw_value::RawValue;
    use nom::Err;
    use nom_language::error::convert_error;

    #[test]
    fn test_next_element_whitespace() {
        let (r, _) = next_element_whitespace("  , hello = world").unwrap();
        assert_eq!(r, " hello = world");
        let (r, _) = next_element_whitespace("  ,, hello = world").unwrap();
        assert_eq!(r, ", hello = world");
    }

    #[test]
    fn test1() -> crate::Result<()> {
        match parse_value("").err() {
            None => {}
            Some(e) => {
                match e {
                    Err::Incomplete(_) => {}
                    Err::Error(e) => {
                        println!("e:{}", convert_error("world", e));
                    }
                    Err::Failure(_) => {}
                }
            }
        }
        // let demo = std::fs::read_to_string("resources/demo.conf")?;
        // match parse_object(&demo) {
        //     Ok(_) => {}
        //     Err(e) => {
        //         println!("{e}");
        //     }
        // }
        // let obj = parse_object(&demo)?;
        Ok(())
    }

    #[test]
    fn test_parse_value() {
        let (remainder, result) = parse_value("\"world\"}").unwrap();
        assert_eq!(result, RawValue::String(RawString::QuotedString("world".to_string())));
        assert_eq!(remainder, "}");
        let (r, o) = parse_value("true false ${?a}").unwrap();
        println!("{}={}", r, o);
        let (r, o) = parse_key("a = true false ${}").unwrap();
        println!("{}={}", r, o);
    }
}