use crate::{
    parser::pure::{
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::comment::CommentType,
};

impl<R: Read> Parser<R> {
    pub(crate) fn parse_comment(&mut self) -> Result<(CommentType, String), DecoderError> {
        let mut scratch = vec![];
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
            return Err(DecoderError::UnexpectedToken {
                expected: "# or //",
                found_beginning: ch,
            });
        };
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
                            Err(DecoderError::Eof) => {
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
                Err(DecoderError::Eof) => {
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

    pub(crate) fn drop_comments(&mut self) -> Result<(), DecoderError> {
        loop {
            self.drop_whitespace()?;
            match self.parse_comment() {
                Ok(_) => {}
                Err(DecoderError::Eof) | Err(DecoderError::UnexpectedToken { .. }) => {
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

    use crate::{
        parser::pure::{
            parser::Parser,
            read::{DecoderError, Read, TestRead},
        },
        raw::comment::CommentType,
    };

    #[rstest]
    #[case(vec!["#","你","好","👌","\r","\r","\n"],(CommentType::Hash,"你好👌\r"),Some("\r\n"))]
    #[case(vec!["#","你","好","👌","\r","\n"],(CommentType::Hash,"你好👌"),Some("\r\n"))]
    #[case(vec!["#","Hello","Wo\nrld","👌","\r","\n"],(CommentType::Hash,"HelloWo"),Some("\nrld"))]
    #[case(vec!["//","Hello","//World\n"],(CommentType::DoubleSlash,"Hello//World"),Some("\n"))]
    #[case(vec!["//","\r\n"],(CommentType::DoubleSlash,""),Some("\r\n"))]
    #[case(vec!["#","\n"],(CommentType::Hash,""),Some("\n"))]
    #[case(vec!["//","Hello","//World"],(CommentType::DoubleSlash,"Hello//World"),None)]
    fn test_valid_comment(
        #[case] input: Vec<&str>,
        #[case] expected: (CommentType, &str),
        #[case] remains: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = TestRead::from_input(input);
        let mut parser = Parser::new(read);
        let (t, c) = parser.parse_comment()?;
        assert_eq!(t, expected.0);
        assert_eq!(c, expected.1);
        assert_eq!(parser.reader.peek_chunk(), remains);
        Ok(())
    }
}
