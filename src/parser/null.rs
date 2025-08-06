use crate::parser::R;
use nom::bytes::complete::tag;
use nom::error::context;
use nom::Parser;

pub(crate) fn null(input: &str) -> R<()> {
    let (input, _) = context("null", tag("null")).parse(input)?;
    Ok((input, ()))
}