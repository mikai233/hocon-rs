use crate::Result;
use crate::error::Error;
use crate::parser::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::comment::CommentType;

impl<R: Read> HoconParser<R> {
    pub(crate) fn parse_comment(&mut self) -> Result<(CommentType, String)> {
        let ty = self.parse_comment_token()?;
        let mut scratch = vec![];
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if ch == '\r' {
                        match self.reader.peek2() {
                            Ok((_, ch2)) => {
                                if ch2 != '\n' {
                                    let (_, bytes) = self.reader.next()?;
                                    scratch.extend_from_slice(bytes);
                                } else {
                                    break;
                                }
                            }
                            Err(Error::Eof) => {
                                let (_, bytes) = self.reader.next()?;
                                scratch.extend_from_slice(bytes);
                            }
                            Err(err) => {
                                return Err(err);
                            }
                        }
                    } else if ch != '\n' {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                    } else {
                        break;
                    }
                }
                Err(Error::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        let s = unsafe { str::from_utf8_unchecked(&scratch) };
        Ok((ty, s.to_string()))
    }

    fn parse_comment_token(&mut self) -> Result<CommentType> {
        let ch = self.reader.peek()?;
        let ty = if ch == '#' {
            self.reader.next()?;
            CommentType::Hash
        } else if let Ok((ch1, ch2)) = self.reader.peek2()
            && ch1 == '/'
            && ch2 == '/'
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

    pub(crate) fn drop_comments(&mut self) -> Result<()> {
        loop {
            self.drop_whitespace()?;
            match self.parse_comment() {
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
    use crate::parser::parser::HoconParser;
    use crate::parser::read::TestRead;
    use crate::raw::comment::CommentType;

    #[rstest]
    #[case(vec!["#","你","好","👌","\r","\r","\n"],(CommentType::Hash,"你好👌\r"),"\r\n")]
    #[case(vec!["#","你","好","👌","\r","\n"],(CommentType::Hash,"你好👌"),"\r\n")]
    #[case(vec!["#","Hello","Wo\nrld","👌","\r","\n"],(CommentType::Hash,"HelloWo"),"\nrld👌\r\n")]
    #[case(vec!["//","Hello","//World\n"],(CommentType::DoubleSlash,"Hello//World"),"\n")]
    #[case(vec!["//","\r\n"],(CommentType::DoubleSlash,""),"\r\n")]
    #[case(vec!["#","\n"],(CommentType::Hash,""),"\n")]
    #[case(vec!["//","Hello","//World"],(CommentType::DoubleSlash,"Hello//World"),"")]
    fn test_valid_comment(
        #[case] input: Vec<&str>,
        #[case] expected: (CommentType, &str),
        #[case] rest: &str,
    ) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let (t, c) = parser.parse_comment()?;
        assert_eq!(t, expected.0);
        assert_eq!(c, expected.1);
        assert_eq!(parser.reader.rest(), rest);
        Ok(())
    }
}
