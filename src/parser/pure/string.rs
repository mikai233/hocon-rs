use crate::parser::pure::{
    parser::Parser,
    read::{DecoderError, Read},
};

const FORBIDDEN_CHARACTERS: [char; 19] = [
    '$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\',
];

enum QuotedStringState {
    Begin,
    Parsing,
    NeedMore { need: usize, known: Option<usize> },
    End,
}

impl QuotedStringState {
    fn need_more(&self, need: usize) -> QuotedStringState {
        if let QuotedStringState::NeedMore { known, .. } = self {
            QuotedStringState::NeedMore {
                need,
                known: *known,
            }
        } else {
            QuotedStringState::NeedMore { need, known: None }
        }
    }
}

impl<R: Read> Parser<R> {
    pub(crate) fn parse_quoted_string(&mut self) -> Result<String, DecoderError> {
        let mut string = String::new();
        let mut state = QuotedStringState::Begin;
        let mut consume_len_utf8 = 0;
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    QuotedStringState::Begin => {
                        if s.starts_with('"') {
                            state = QuotedStringState::Parsing;
                            consume_len_utf8 += 1;
                        } else {
                            return Err(DecoderError::UnexpectedToken);
                        }
                    }
                    QuotedStringState::Parsing => {
                        let mut iter = s.chars();
                        while let Some(ch) = iter.next() {
                            match ch {
                                '\\' => {
                                    if let Some((c, l)) =
                                        self.parse_escaped_char(&mut iter, &mut state)?
                                    {
                                        string.push(c);
                                        consume_len_utf8 += l;
                                    }
                                }
                                '"' => {
                                    consume_len_utf8 += 1;
                                    state = QuotedStringState::End;
                                    break;
                                }
                                _ => {
                                    consume_len_utf8 += ch.len_utf8();
                                    string.push(ch);
                                }
                            }
                        }
                    }
                    QuotedStringState::NeedMore { need, known } => {
                        let current_available_before = self.reader.available_chars();
                        match self.reader.fill_buf() {
                            Ok(_) => {
                                let current_available_after = self.reader.available_chars();
                                if Some(current_available_after) > known
                                    && current_available_after - current_available_before < need
                                {
                                    state = QuotedStringState::NeedMore {
                                        need,
                                        known: Some(current_available_before),
                                    };
                                } else if current_available_after - current_available_before < need
                                {
                                    return Err(DecoderError::UnexpectedEof);
                                } else {
                                    state = QuotedStringState::Parsing;
                                }
                            }
                            Err(err) => return Err(err),
                        }
                    }
                    QuotedStringState::End => {
                        break;
                    }
                },
                None => match self.reader.fill_buf() {
                    Ok(_) => {}
                    Err(DecoderError::Eof) => {
                        if matches!(state, QuotedStringState::End) {
                            break;
                        } else {
                            return Err(DecoderError::UnexpectedEof);
                        }
                    }
                    Err(err) => {
                        return Err(err);
                    }
                },
            }
            self.reader.consume(consume_len_utf8);
            consume_len_utf8 = 0;
        }
        Ok(string)
    }

    fn parse_escaped_char(
        &self,
        iter: &mut std::str::Chars,
        state: &mut QuotedStringState,
    ) -> Result<Option<(char, usize)>, DecoderError> {
        let ch = match iter.next() {
            Some(esc) => match esc {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\x08',
                'f' => '\x0C',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => {
                    return self.parse_escaped_unicode(iter, state);
                }
                _ => return Err(DecoderError::InvalidEscape),
            },
            None => {
                *state = state.need_more(1);
                return Ok(None);
            }
        };
        Ok(Some((ch, 2)))
    }

    fn parse_escaped_unicode(
        &self,
        iter: &mut std::str::Chars,
        state: &mut QuotedStringState,
    ) -> Result<Option<(char, usize)>, DecoderError> {
        let mut high_code = String::new();
        for i in 0..4 {
            match iter.next() {
                Some(digit) if digit.is_ascii_hexdigit() => {
                    high_code.push(digit);
                }
                Some(_) => {
                    return Err(DecoderError::InvalidEscape);
                }
                None => {
                    *state = state.need_more(4 - i);
                    return Ok(None);
                }
            }
        }
        let high = u32::from_str_radix(&high_code, 16).map_err(|_| DecoderError::InvalidEscape)?;
        if (0xD800..=0xDBFF).contains(&high) {
            // 需要再读一个 \uXXXX
            match (iter.next(), iter.next()) {
                (Some('\\'), Some('u')) => {
                    let mut low_code = String::new();
                    for i in 0..4 {
                        match iter.next() {
                            Some(digit) if digit.is_ascii_hexdigit() => {
                                low_code.push(digit);
                            }
                            Some(_) => {
                                return Err(DecoderError::InvalidEscape);
                            }
                            None => {
                                *state = state.need_more(4 - i);
                                return Ok(None);
                            }
                        }
                    }
                    let low = u32::from_str_radix(&low_code, 16)
                        .map_err(|_| DecoderError::InvalidEscape)?;

                    if (0xDC00..=0xDFFF).contains(&low) {
                        // 合并 surrogate pair
                        let codepoint = 0x10000 + (((high - 0xD800) << 10) | (low - 0xDC00));
                        let ch = char::from_u32(codepoint).ok_or(DecoderError::InvalidEscape)?;
                        Ok(Some((ch, 12)))
                    } else {
                        Err(DecoderError::InvalidEscape)
                    }
                }
                _ => {
                    *state = state.need_more(6);
                    Ok(None)
                }
            }
        } else {
            let ch = char::from_u32(high).ok_or(DecoderError::InvalidEscape)?;
            Ok(Some((ch, 6)))
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::pure::{
        parser::Parser,
        read::{DecoderError, Read, SliceRead, TestRead},
    };

    #[rstest]
    #[case("\"hello\"", "hello", None)]
    #[case("\"hello\\nworld\"", "hello\nworld", None)]
    #[case(
        r#""line1\nline2\tindent\\slash\"quote""#,
        "line1\nline2\tindent\\slash\"quote",
        None
    )]
    #[case(r#""\u4F60\u597D""#, "你好", None)]
    #[case(r#""\uD83D\uDE00""#, "😀", None)]
    #[case(r#""Hello \u4F60\u597D \n 😀!""#, "Hello 你好 \n 😀!", None)]
    fn test_valid_quoted_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] remians: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_quoted_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.peek_chunk(), remians);
        Ok(())
    }

    #[rstest]
    #[case(r#""Hello \"#)]
    #[case(r#""\uD83D\u0041""#)]
    #[case("")]
    #[case("\"")]
    #[case("\"\\u")]
    #[case("\"\\uD83")]
    #[case(r#""\uD83D\u004`""#)]
    #[case(r#""\uD83D\u004""#)]
    fn test_invalid_quoted_string(#[case] input: &str) {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let result = parser.parse_quoted_string();
        assert!(result.is_err());
    }

    #[rstest]
    #[case(vec!["\"Hello", "World\""], "HelloWorld")]
    #[case(vec!["\"Hello", "World", "!\""], "HelloWorld!")]
    #[case(vec!["\"Hello", "World", "!", "How", "are", "you\""], "HelloWorld!Howareyou")]
    #[case(vec!["\"", "\\uD8", "3", "D", "\\", "u","DE00","\""],"😀")]
    fn test_quoted_string_increment_parse(
        #[case] input: Vec<&str>,
        #[case] expected: &str,
    ) -> Result<(), DecoderError> {
        let mut input = input
            .into_iter()
            .map(|s| s.as_bytes().to_vec())
            .collect::<Vec<_>>();
        let read = TestRead::new(vec![], move || {
            if input.is_empty() {
                vec![]
            } else {
                input.remove(0)
            }
        });
        let mut parser = Parser::new(read);
        let s = parser.parse_quoted_string()?;
        assert_eq!(s, expected);
        Ok(())
    }
}
