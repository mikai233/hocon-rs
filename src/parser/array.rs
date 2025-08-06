use crate::parser::{next_element_whitespace, parse_value, whitespace, R};
use crate::raw::raw_array::RawArray;
use crate::raw::raw_value::RawValue;
use nom::character::complete::char;
use nom::combinator::map;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::Parser;

pub(crate) fn array(input: &str) -> R<RawArray> {
    fn array_element(input: &str) -> R<RawValue> {
        let (input, (_, value, _)) = (whitespace, parse_value, next_element_whitespace).parse(input)?;
        Ok((input, value))
    }

    delimited(char('['), map(many0(array_element), RawArray::new), char(']')).parse(input)
}