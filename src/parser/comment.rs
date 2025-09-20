use crate::error::{Error, Parse};
use crate::parser::HoconParser;
use crate::parser::read::{Read, Reference};
use crate::raw::comment::CommentType;
use crate::{Result, ref_to_string};

impl<'de, R: Read<'de>> HoconParser<R> {
    fn parse_comment_inner<'s>(
        reader: &'s mut R,
        scratch: &'s mut Vec<u8>,
    ) -> Result<(CommentType, Reference<'de, 's, str>)> {
        let ty = Self::parse_comment_token(reader)?;
        scratch.clear();
        let s = reader.parse_to_line_ending(scratch)?;
        Ok((ty, s))
    }

    pub(crate) fn parse_comment(reader: &mut R) -> Result<(CommentType, String)> {
        let mut scratch = vec![];
        let (ty, s) = Self::parse_comment_inner(reader, &mut scratch)?;
        let s = ref_to_string!(s, &mut scratch);
        Ok((ty, s))
    }

    fn parse_comment_token(reader: &mut R) -> Result<CommentType> {
        let byte = reader
            .peek()
            .map_err(|_| reader.peek_error(Parse::Expected("# or //")))?;
        let ty = if byte == b'#' {
            reader.discard(1)?;
            CommentType::Hash
        } else if let Ok(bytes) = reader.peek_n(2)
            && bytes == b"//"
        {
            reader.discard(2)?;
            CommentType::DoubleSlash
        } else {
            return Err(reader.peek_error(Parse::Expected("# or //")));
        };
        Ok(ty)
    }

    pub(crate) fn drop_whitespace_and_comments(reader: &mut R) -> Result<()> {
        let mut scratch = vec![];
        loop {
            Self::drop_whitespace(reader)?;
            match Self::parse_comment_inner(reader, &mut scratch) {
                Ok(_) => {}
                Err(Error::Eof) | Err(Error::Parse { .. }) => {
                    break Ok(());
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::Result;
    use crate::parser::HoconParser;
    use crate::parser::read::StrRead;
    use crate::raw::comment::CommentType;

    #[rstest]
    #[case("#你好👌\r\r\n", (CommentType::Hash, "你好👌\r"), "\r\n")]
    #[case("#你好👌\r\n", (CommentType::Hash, "你好👌"), "\r\n")]
    #[case("#HelloWo\nrld👌\r\n", (CommentType::Hash, "HelloWo"), "\nrld👌\r\n")]
    #[case("//Hello//World\n", (CommentType::DoubleSlash, "Hello//World"), "\n")]
    #[case("//\r\n", (CommentType::DoubleSlash, ""), "\r\n")]
    #[case("#\n", (CommentType::Hash, ""), "\n")]
    #[case("//Hello//World", (CommentType::DoubleSlash, "Hello//World"), "")]
    fn test_valid_comment(
        #[case] input: &str,
        #[case] expected: (CommentType, &str),
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let (t, c) = HoconParser::parse_comment(&mut parser.reader)?;
        assert_eq!(t, expected.0);
        assert_eq!(c, expected.1);
        assert_eq!(parser.reader.rest(), rest);
        Ok(())
    }
}
