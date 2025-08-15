use crate::parser::{hocon_horizontal_multi_space0, R};
use crate::raw::comment::CommentType;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::line_ending;
use nom::combinator::{opt, value};
use nom::sequence::{preceded, terminated};
use nom::Parser;

fn comment_separator(input: &str) -> R<'_, CommentType> {
    alt(
        (
            value(CommentType::DoubleSlash, tag("//")),
            value(CommentType::Hash, tag("#")),
        )
    ).parse_complete(input)
}

fn comment_content(input: &str) -> R<'_, &str> {
    is_not("\n\r")(input)
}

//TODO line ending
pub(crate) fn parse_comment(input: &str) -> R<'_, (CommentType, &str)> {
    preceded(
        hocon_horizontal_multi_space0,
        terminated(
            (
                comment_separator,
                comment_content,
            ),
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
        let (r, o) = parse_comment("  //  include \"demo.conf\" // comment").unwrap();
        println!("{:?} {:?}", r, o);
    }
}