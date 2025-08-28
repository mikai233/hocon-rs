use crate::parser::string::parse_path_expression;
use crate::parser::{R, hocon_horizontal_space0};
use crate::raw::substitution::Substitution;
use nom::Parser;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{map, opt};
use nom::sequence::delimited;

pub(crate) fn parse_substitution(input: &str) -> R<'_, Substitution> {
    let (remainder, (mut substition, space)) = (
        delimited(
            tag("${"),
            map(
                (
                    opt(char('?')),
                    hocon_horizontal_space0,
                    parse_path_expression,
                    hocon_horizontal_space0,
                ),
                |(optional, _, path, _)| Substitution::new(path, optional.is_some(), None),
            ),
            tag("}"),
        ),
        opt(hocon_horizontal_space0),
    )
        .parse_complete(input)?;
    substition.space = space.map(|s| s.to_string());
    Ok((remainder, substition))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::substitution::parse_substitution;

    #[rstest]
    #[case("${? \"a .\".b. c}", "${?\"a .\".b. c}", "")]
    #[case("${? a.b.c}", "${?a.b.c}", "")]
    #[case("${ \"a .\".b. c / }", "${\"a .\".b. c /}", "")]
    #[case("${foo}", "${foo}", "")]
    #[case("${\"\".foo}abc", "${\"\".foo}", "abc")]
    #[case("${\"\"\"a\"\"\".\" b.\". c }", "${\"\"\"a\"\"\".\" b.\". c}", "")]
    #[case("${foo.bar} hello", "${foo.bar}", "hello")]
    #[case("${foo }  ", "${foo}", "")]
    fn test_valide_substitution(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse_substitution(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed.to_string(), expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case("${}")]
    #[case("$ {a.b}")]
    #[case("${\na}")]
    #[case("$\n{foo.bar}")]
    #[case("${foo.\nbar}")]
    #[case("${foo\r.bar}")]
    fn test_invalid_substitution(#[case] input: &str) {
        let result = parse_substitution(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }
}
