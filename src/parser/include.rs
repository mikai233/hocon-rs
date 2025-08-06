use crate::parser::string::quoted_string;
use crate::parser::{whitespace, R};
use crate::raw::include::{Inclusion, Location};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{map, opt};
use nom::error::context;
use nom::Parser;
use nom::sequence::{delimited, preceded};

pub(crate) fn include(input: &str) -> R<Inclusion> {
    // optional 'required(...)' wrapper
    let (remainder, is_required) = opt(delimited(
        context("required", tag("required")),
        delimited(char('('), opt(whitespace), char(')')),
        whitespace,
    )).parse(input)?;

    let is_required = is_required.is_some();
    fn inclusion(path: &str, location: Option<Location>, required: bool) -> Inclusion {
        Inclusion {
            depth: 0,
            path: path.to_string(),
            required,
            location,
            val: None,
        }
    }

    // include types
    let (remainder, value) = alt((
        preceded(context("url", tag("url")), delimited(char('('), map(quoted_string, |s| inclusion(s, Some(Location::Url), is_required)), char(')'))),
        preceded(context("file", tag("file")), delimited(char('('), map(quoted_string, |s| inclusion(s, Some(Location::File), is_required)), char(')'))),
        preceded(context("classpath", tag("classpath")), delimited(char('('), map(quoted_string, |s| inclusion(s, Some(Location::Classpath), is_required)), char(')'))),
    )).parse(remainder)?;

    Ok((remainder, value))
}