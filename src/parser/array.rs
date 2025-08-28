use crate::parser::{hocon_multi_space0, next_element_whitespace, parse_value, R};
use crate::raw::raw_array::RawArray;
use crate::raw::raw_value::RawValue;
use nom::character::complete::char;
use nom::combinator::map;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::Parser;

pub(crate) fn parse_array(input: &str) -> R<'_, RawArray> {
    delimited(
        char('['),
        map(many0(array_element), RawArray::new),
        char(']'),
    )
    .parse(input)
}

fn array_element(input: &str) -> R<'_, RawValue> {
    let (input, (_, value, _)) =
        (hocon_multi_space0, parse_value, next_element_whitespace).parse(input)?;
    Ok((input, value))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_array() {

    }
}
