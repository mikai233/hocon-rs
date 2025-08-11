use crate::parser::{hocon_multi_space0, R};
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::line_ending;
use nom::combinator::opt;
use nom::sequence::{preceded, terminated};
use nom::Parser;

fn comment_separator(input: &str) -> R<'_, &str> {
    alt((tag("//"), tag("#"))).parse_complete(input)
}

fn comment_content(input: &str) -> R<'_, &str> {
    is_not("\n\r")(input)
}

pub(crate) fn parse_comment(input: &str) -> R<'_, &str> {
    preceded(
        hocon_multi_space0,
        terminated(
            preceded(comment_separator, comment_content),
            opt(line_ending),
        ),
    ).parse_complete(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::comment::parse_comment;

    #[test]
    fn test_parse_comment() {
        let (r, o) = parse_comment("//foo\nbar").unwrap();
        println!("{:?} {:?}", r, o);
        let (r, o) = parse_comment("###//foo\nbar").unwrap();
        println!("{:?} {:?}", r, o);

        let (r, o) = parse_comment("# /foo\nbar").unwrap();
        println!("{:?} {:?}", r, o);
    }
}