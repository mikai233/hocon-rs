use crate::parser::string::parse_path_expression;
use crate::parser::{R, hocon_horizontal_multi_space0};
use crate::raw::substitution::Substitution;
use nom::Parser;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{map, opt};
use nom::sequence::delimited;

pub(crate) fn parse_substitution(input: &str) -> R<'_, Substitution> {
    delimited(
        tag("${"),
        map(
            (
                opt(char('?')),
                hocon_horizontal_multi_space0,
                parse_path_expression,
                hocon_horizontal_multi_space0,
            ),
            |(optional, _, path, _)| Substitution::new(path, optional.is_some()),
        ),
        tag("}"),
    )
    .parse_complete(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::substitution::parse_substitution;

    #[test]
    fn test_substitution() {
        let (r, o) = parse_substitution("${? \"a .\".b. c}").unwrap();
        println!("{}={:?}", r, o.path);
        let (r, o) = parse_substitution("${? a.b.c}").unwrap();
        println!("{}={:?}", r, o.path);
        let (r, o) = parse_substitution("${? \"a .\".b.c}").unwrap();
        println!("{}={:?}", r, o.path);
        let (r, o) = parse_substitution("${? \"a .\".b.c}").unwrap();
        println!("{}={:?}", r, o.path);
        let (r, o) = parse_substitution("${ \"a .\".b. c / }").unwrap();
        println!("{}={:?}", r, o.path);
        let (r, o) = parse_substitution("${\"\"\"a\"\"\".\" b.\". c }").unwrap();
        println!("{}={:?}", r, o.path);
    }
}
