use crate::parser::{next_element_whitespace, parse_value, hocon_multi_space0, R};
use crate::raw::raw_array::RawArray;
use crate::raw::raw_value::RawValue;
use nom::character::complete::char;
use nom::combinator::map;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::Parser;

pub(crate) fn array(input: &str) -> R<RawArray> {
    delimited(
        char('['),
        map(many0(array_element), RawArray::new),
        char(']'),
    ).parse(input)
}

fn array_element(input: &str) -> R<RawValue> {
    let (input, (_, value, _)) = (hocon_multi_space0, parse_value, next_element_whitespace).parse(input)?;
    println!("{input}");
    Ok((input, value))
}

#[cfg(test)]
mod tests {
    use crate::parser::array::array;
    use crate::raw::extension::{FloatRawValueExt, IntRawValueExt};
    use crate::raw::raw_array::RawArray;

    #[test]
    fn test_array() {
        let data = "[1,2, 3,4  5.0]";
        let (r, o) = array(data).unwrap();
        assert_eq!(r, "");
        assert_eq!(o, RawArray::new(vec![1.r(), 2.r(), 3.r(), 4.r(), 5.0.r()]));
    }
}