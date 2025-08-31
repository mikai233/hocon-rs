use crate::{
    parser::pure::{
        find_line_break,
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::comment::CommentType,
};

enum CommentState {
    Start,
    InProgress,
    NeedsMore,
    End,
}

impl<R: Read> Parser<R> {
    pub(crate) fn parse_comment(&mut self) -> Result<(CommentType, String), DecoderError> {
        let mut comment = String::new();
        let mut ty = CommentType::DoubleSlash;
        let mut state = CommentState::Start;
        let mut consume_len_utf8 = 0;
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    CommentState::Start => {
                        if s.starts_with("//") {
                            state = CommentState::InProgress;
                            consume_len_utf8 += "//".len();
                        } else if s.starts_with('#') {
                            state = CommentState::InProgress;
                            ty = CommentType::Hash;
                            consume_len_utf8 += '#'.len_utf8();
                        } else {
                            return Err(DecoderError::unexpected_token("// or #", s));
                        }
                    }
                    CommentState::InProgress => match find_line_break(s.as_bytes()) {
                        Some((pos, _)) => {
                            comment.push_str(&s[..pos]);
                            consume_len_utf8 += pos;
                            state = CommentState::End;
                        }
                        None => {
                            let is_carriage_return =
                                s.as_bytes().last().map_or(false, |&b| b == b'\r');
                            let s = if is_carriage_return {
                                state = CommentState::NeedsMore;
                                &s[..s.len() - 1]
                            } else {
                                s
                            };
                            comment.push_str(s);
                            consume_len_utf8 += s.len();
                        }
                    },
                    CommentState::NeedsMore => match self.reader.fill_buf() {
                        Ok(_) => {
                            state = CommentState::InProgress;
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    },
                    CommentState::End => {
                        break;
                    }
                },
                None => match self.reader.fill_buf() {
                    Ok(_) => {}
                    Err(DecoderError::Eof) => {
                        state = CommentState::End;
                        break;
                    }
                    Err(err) => return Err(err),
                },
            }
            self.reader.consume(consume_len_utf8);
            consume_len_utf8 = 0;
        }
        if !matches!(state, CommentState::End) {
            return Err(DecoderError::UnexpectedEof);
        }
        Ok((ty, comment))
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
