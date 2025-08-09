use crate::parser::string::parse_quoted_string;
use crate::parser::{hocon_multi_space0, R};
use crate::raw::include::{Inclusion, Location};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{map, opt};
use nom::error::context;
use nom::sequence::{delimited, preceded};
use nom::Parser;

pub(crate) fn parse_include(input: &str) -> R<'_, Inclusion> {
    // optional 'required(...)' wrapper
    let (remainder, is_required) = opt(delimited(
        context("required", tag("required")),
        delimited(char('('), opt(hocon_multi_space0), char(')')),
        hocon_multi_space0,
    )).parse(input)?;

    let is_required = is_required.is_some();
    fn inclusion(path: impl Into<String>, location: Option<Location>, required: bool) -> Inclusion {
        Inclusion {
            depth: 0,
            path: path.into(),
            required,
            location,
            val: None,
        }
    }

    // include types
    let (remainder, value) = alt((
        preceded(context("url", tag("url")), delimited(char('('), map(parse_quoted_string, |s| inclusion(s, Some(Location::Url), is_required)), char(')'))),
        preceded(context("file", tag("file")), delimited(char('('), map(parse_quoted_string, |s| inclusion(s, Some(Location::File), is_required)), char(')'))),
        preceded(context("classpath", tag("classpath")), delimited(char('('), map(parse_quoted_string, |s| inclusion(s, Some(Location::Classpath), is_required)), char(')'))),
    )).parse(remainder)?;

    Ok((remainder, value))
}