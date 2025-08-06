use crate::parser::R;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::value;
use nom::error::context;
use nom::Parser;

pub(crate) fn boolean(input: &str) -> R<bool> {
    let parse_true = value(true, context("true", tag("true")));
    let parse_false = value(false, context("false", tag("false")));
    alt((parse_true, parse_false)).parse(input)
}