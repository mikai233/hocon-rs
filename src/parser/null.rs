use crate::parser::R;
use nom::Parser;
use nom::bytes::complete::tag;
use nom::error::context;

pub(crate) fn parse_null(input: &str) -> R<'_, ()> {
    let (input, _) = context("null", tag("null")).parse(input)?;
    Ok((input, ()))
}
