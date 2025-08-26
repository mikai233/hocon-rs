use crate::parser::{R, hocon_horizontal_space0};
use crate::raw::comment::CommentType;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while};
use nom::character::complete::line_ending;
use nom::combinator::{opt, value};
use nom::sequence::{preceded, terminated};

#[inline]
fn comment_separator(input: &str) -> R<'_, CommentType> {
    alt((
        value(CommentType::DoubleSlash, tag("//")),
        value(CommentType::Hash, tag("#")),
    ))
    .parse_complete(input)
}

#[inline]
fn comment_content(input: &str) -> R<'_, &str> {
    take_while(|c| c != '\n' && c != '\r').parse_complete(input)
}

//TODO line ending
#[inline]
pub(crate) fn parse_comment(input: &str) -> R<'_, (CommentType, &str)> {
    preceded(
        hocon_horizontal_space0,
        terminated((comment_separator, comment_content), opt(line_ending)),
    )
    .parse_complete(input)
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
