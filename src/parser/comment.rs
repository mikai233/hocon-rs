use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::{Read, Reference};
use crate::raw::comment::CommentType;

impl<'de, R: Read<'de>> HoconParser<R> {
    fn parse_comment_inner<'s>(&'s mut self) -> Result<(CommentType, Reference<'de, 's, str>)> {
        let ty = self.parse_comment_token()?;
        self.scratch.clear();
        let content =
            self.reader
                .parse_str(true, &mut self.scratch, |reader| match reader.peek() {
                    Ok(ch) => match ch {
                        b'\r' => match reader.peek2() {
                            Ok((_, ch2)) => {
                                if ch2 == b'\n' {
                                    Ok(true)
                                } else {
                                    Ok(false)
                                }
                            }
                            Err(Error::Eof) => Ok(false),
                            Err(err) => Err(err),
                        },
                        b'\n' => Ok(true),
                        _ => Ok(false),
                    },
                    Err(Error::Eof) => Ok(true),
                    Err(err) => Err(err),
                })?;
        Ok((ty, content))
    }

    pub(crate) fn parse_comment(&mut self) -> Result<(CommentType, String)> {
        self.parse_comment_inner().map(|(t, c)| (t, c.to_string()))
    }

    fn parse_comment_token(&mut self) -> Result<CommentType> {
        let ch = self.reader.peek()?;
        let ty = if ch == b'#' {
            self.reader.next()?;
            CommentType::Hash
        } else if let Ok((ch1, ch2)) = self.reader.peek2()
            && ch1 == b'/'
            && ch2 == b'/'
        {
            self.reader.next()?;
            self.reader.next()?;
            CommentType::DoubleSlash
        } else {
            return Err(Error::UnexpectedToken {
                expected: "# or //",
                found_beginning: ch,
            });
        };
        Ok(ty)
    }

    pub(crate) fn drop_whitespace_and_comments(&mut self) -> Result<()> {
        loop {
            self.drop_whitespace()?;
            match self.parse_comment_inner() {
                Ok(_) => {}
                Err(Error::Eof) | Err(Error::UnexpectedToken { .. }) => {
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
        let (t, c) = parser.parse_comment()?;
        assert_eq!(t, expected.0);
        assert_eq!(c, expected.1);
        assert_eq!(parser.reader.rest()?, rest);
        Ok(())
    }
}
