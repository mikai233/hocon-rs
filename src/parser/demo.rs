use std::collections::HashMap;

use nom::{
    branch::alt, bytes::complete::{is_not, tag, take_while1},
    character::complete::{char, multispace0},
    combinator::value,
    multi::separated_list0,
    sequence::{delimited, preceded, separated_pair},
    IResult,
    Parser,
};

use crate::parser::{arena_input::ArenaInput, error::ParseError};

#[derive(Debug, PartialEq, Clone)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

type R<'a, T> = IResult<ArenaInput<'a>, T, ParseError<'a>>;

/// Parse `null`
fn parse_null(input: ArenaInput) -> R<JsonValue> {
    value(JsonValue::Null, tag("null")).parse_complete(input)
}

/// Parse `true` / `false`
fn parse_bool(input: ArenaInput) -> R<JsonValue> {
    alt((
        value(JsonValue::Bool(true), tag("true")),
        value(JsonValue::Bool(false), tag("false")),
    ))
    .parse_complete(input)
}

/// Parse number (very simplified: only digits and optional dot)
fn parse_number(input: ArenaInput) -> R<JsonValue> {
    let (input, num_str) = take_while1(|c: char| c.is_digit(10) || c == '.')(input)?;
    let num = num_str.parse::<f64>().unwrap();
    Ok((input, JsonValue::Number(num)))
}

/// Parse string (no escape support, just raw until `"`)
fn parse_string(input: ArenaInput) -> R<JsonValue> {
    let (input, content) = delimited(char('"'), is_not("\""), char('"')).parse_complete(input)?;
    Ok((input, JsonValue::String(content.to_string())))
}

/// Parse array
fn parse_array(input: ArenaInput) -> R<JsonValue> {
    let (input, values) = delimited(
        preceded(multispace0, char('[')),
        separated_list0(
            preceded(multispace0, char(',')),
            preceded(multispace0, parse_value),
        ),
        preceded(multispace0, char(']')),
    )
    .parse_complete(input)?;
    Ok((input, JsonValue::Array(values)))
}

/// Parse object
fn parse_object(input: ArenaInput) -> R<JsonValue> {
    let (input, pairs) = delimited(
        preceded(multispace0, char('{')),
        separated_list0(
            preceded(multispace0, char(',')),
            separated_pair(
                preceded(multispace0, parse_string),
                preceded(multispace0, char(':')),
                preceded(multispace0, parse_value),
            ),
        ),
        preceded(multispace0, char('}')),
    )
    .parse_complete(input)?;

    let mut map = HashMap::new();
    for (k, v) in pairs {
        if let JsonValue::String(s) = k {
            map.insert(s, v);
        }
    }
    Ok((input, JsonValue::Object(map)))
}

/// Parse any JSON value
fn parse_value(input: ArenaInput) -> R<JsonValue> {
    let obj = "{\"hello\" = \"world\"}".to_string();
    let obj = input.arean.alloc(obj);
    let input2 = input.copy_from(obj);
    parse_object(input2)?;
    preceded(
        multispace0,
        alt((
            parse_null,
            parse_bool,
            parse_number,
            parse_string,
            parse_array,
            parse_object,
        )),
    )
    .parse_complete(input)
}
