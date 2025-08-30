use memchr::memchr;

use crate::parser::pure::{
    is_hocon_whitespace,
    parser::Parser,
    read::{DecoderError, Read},
};

const FORBIDDEN_CHARACTERS: [char; 19] = [
    '$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\',
];

enum QuotedStringState {
    Start,
    InProgress,
    NeedsMore(usize),
    Finished,
}

enum UnquotedStringState {
    InProgress,
    NeedsMore,
    Finished,
}

enum MultilineStringState {
    Start,
    InProgress,
    NeedsMore(bool),
    Finished,
}

impl<R: Read> Parser<R> {
    pub(crate) fn parse_quoted_string(&mut self) -> Result<String, DecoderError> {
        let mut string = String::new();
        let mut state = QuotedStringState::Start;
        let mut consume_len_utf8 = 0;
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    QuotedStringState::Start => {
                        if s.starts_with('"') {
                            state = QuotedStringState::InProgress;
                            consume_len_utf8 += 1;
                        } else {
                            return Err(DecoderError::UnexpectedToken);
                        }
                    }
                    QuotedStringState::InProgress => {
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
                                    state = QuotedStringState::Finished;
                                    break;
                                }
                                _ => {
                                    consume_len_utf8 += ch.len_utf8();
                                    string.push(ch);
                                }
                            }
                        }
                    }
                    QuotedStringState::NeedsMore(need) => match self.reader.fill_buf() {
                        Ok(_) => {
                            if self.reader.has_at_least_n_chars(need) {
                                state = QuotedStringState::InProgress;
                            }
                        }
                        Err(DecoderError::Eof) => {
                            return Err(DecoderError::UnexpectedEof);
                        }
                        Err(err) => return Err(err),
                    },
                    QuotedStringState::Finished => {
                        break;
                    }
                },
                None => match self.reader.fill_buf() {
                    Ok(_) => {}
                    Err(DecoderError::Eof) => {
                        if matches!(state, QuotedStringState::Finished) {
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
                *state = QuotedStringState::NeedsMore(1);
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
        for _ in 0..4 {
            match iter.next() {
                Some(digit) if digit.is_ascii_hexdigit() => {
                    high_code.push(digit);
                }
                Some(_) => {
                    return Err(DecoderError::InvalidEscape);
                }
                None => {
                    *state = QuotedStringState::NeedsMore(6);
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
                    for _ in 0..4 {
                        match iter.next() {
                            Some(digit) if digit.is_ascii_hexdigit() => {
                                low_code.push(digit);
                            }
                            Some(_) => {
                                return Err(DecoderError::InvalidEscape);
                            }
                            None => {
                                *state = QuotedStringState::NeedsMore(12);
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
                    *state = QuotedStringState::NeedsMore(12);
                    Ok(None)
                }
            }
        } else {
            let ch = char::from_u32(high).ok_or(DecoderError::InvalidEscape)?;
            Ok(Some((ch, 6)))
        }
    }

    pub(crate) fn parse_unquoted_string(&mut self) -> Result<String, DecoderError> {
        let mut string = String::new();
        let mut consume_len_utf8 = 0;
        let mut state = UnquotedStringState::InProgress;
        loop {
            match state {
                UnquotedStringState::InProgress => {
                    match self.reader.peek_chunk() {
                        Some(s) => {
                            let mut p = s.chars().peekable();
                            while let Some(ch) = p.next() {
                                if ch == '/' {
                                    // need to peek next char to see if it's a double slash comment
                                    match p.peek() {
                                        Some(next) => {
                                            if next != &'/' {
                                                consume_len_utf8 += ch.len_utf8();
                                                string.push(ch);
                                            } else {
                                                state = UnquotedStringState::Finished;
                                                break;
                                            }
                                        }
                                        None => {
                                            // need more data to determine if it's a comment or not
                                            state = UnquotedStringState::NeedsMore;
                                            break;
                                        }
                                    }
                                } else if !FORBIDDEN_CHARACTERS.contains(&ch)
                                    && !is_hocon_whitespace(ch)
                                {
                                    consume_len_utf8 += ch.len_utf8();
                                    string.push(ch);
                                } else {
                                    // current character is not a valid unquoted string character
                                    state = UnquotedStringState::Finished;
                                    break;
                                }
                            }
                        }
                        None => match self.reader.fill_buf() {
                            Ok(_) => {}
                            Err(DecoderError::Eof) => {
                                break;
                            }
                            Err(err) => {
                                return Err(err);
                            }
                        },
                    }
                }
                UnquotedStringState::NeedsMore => match self.reader.fill_buf() {
                    Ok(_) => {
                        state = UnquotedStringState::InProgress;
                    }
                    Err(DecoderError::Eof) => {
                        consume_len_utf8 += '/'.len_utf8();
                        string.push('/');
                    }
                    Err(err) => {
                        return Err(err);
                    }
                },
                UnquotedStringState::Finished => {
                    break;
                }
            }
            self.reader.consume(consume_len_utf8);
            consume_len_utf8 = 0;
        }
        if string.is_empty() {
            return Err(DecoderError::UnexpectedEof);
        }
        Ok(string)
    }

    fn parse_multiline_string(&mut self) -> Result<String, DecoderError> {
        let mut string = String::new();
        let mut state = MultilineStringState::Start;
        let mut consume_len_utf8 = 0;
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    MultilineStringState::Start => {
                        if s.starts_with("\"\"\"") {
                            state = MultilineStringState::InProgress;
                            consume_len_utf8 += 3;
                        } else if !self.reader.has_at_least_n_chars(3) {
                            state = MultilineStringState::NeedsMore(true);
                        } else {
                            return Err(DecoderError::UnexpectedToken);
                        }
                    }
                    MultilineStringState::InProgress => match memchr(b'"', s.as_bytes()) {
                        Some(i) => {
                            if s[i..].starts_with("\"\"\"") {
                                let chunk = &s[..i];
                                string.push_str(chunk);
                                consume_len_utf8 += chunk.len() + 3;
                                state = MultilineStringState::Finished;
                            } else {
                                let chunk = &s[..i];
                                string.push_str(chunk);
                                consume_len_utf8 += chunk.len();
                                state = MultilineStringState::NeedsMore(false);
                            }
                        }
                        None => {
                            string.push_str(s);
                            consume_len_utf8 += s.len();
                            state = MultilineStringState::NeedsMore(false);
                        }
                    },
                    MultilineStringState::NeedsMore(start) => match self.reader.fill_buf() {
                        Ok(_) => {
                            if self.reader.has_at_least_n_chars(3) {
                                if start {
                                    state = MultilineStringState::Start;
                                } else {
                                    state = MultilineStringState::InProgress;
                                }
                            }
                        }
                        Err(DecoderError::Eof) => {
                            return Err(DecoderError::UnexpectedEof);
                        }
                        Err(err) => return Err(err),
                    },
                    MultilineStringState::Finished => {
                        break;
                    }
                },
                None => match self.reader.fill_buf() {
                    Ok(_) => {}
                    Err(DecoderError::Eof) => {
                        if matches!(state, MultilineStringState::Finished) {
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
        if !matches!(state, MultilineStringState::Finished) {
            return Err(DecoderError::UnexpectedEof);
        }
        Ok(string)
    }

    fn parse_path_expression(&mut self) -> Result<Vec<String>, DecoderError> {
        unimplemented!()
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

    #[rstest]
    #[case("a.b.c", "a.b.c", None)]
    #[case("a.b.c//", "a.b.c", Some("//"))]
    #[case("a.b.c/b", "a.b.c/b", None)]
    #[case("hello#world", "hello", Some("#world"))]
    #[case("你 好", "你", Some(" 好"))]
    #[case("你 \\r\n不好", "你", Some(" \\r\n不好"))]
    #[case("你 \r\n不好", "你", Some(" \r\n不好"))]
    fn test_valid_unquoted_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] remians: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_unquoted_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.peek_chunk(), remians);
        Ok(())
    }

    #[rstest]
    #[case(vec!["Hello", "World"], "HelloWorld")]
    #[case(vec!["Hello", "World", "!"], "HelloWorld")]
    #[case(vec!["Hello", "World", "vs", "How", "are", "you"], "HelloWorldvsHowareyou")]
    #[case(vec!["a.", "b", ".", "你", "/", "u","DE00",""],"a.b.你/uDE00")]
    fn test_unquoted_string_increment_parse(
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
        let s = parser.parse_unquoted_string()?;
        assert_eq!(s, expected);
        Ok(())
    }

    #[rstest]
    #[case(r#""""a.bbc""""#, "a.bbc", None)]
    #[case(r#""""a.bbc😀"""😀"#, "a.bbc😀", Some("😀"))]
    #[case(r#""""a.b\r\nbc""""#, "a.b\\r\\nbc", None)]
    fn test_valid_multiline_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] remians: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.peek_chunk(), remians);
        Ok(())
    }

    #[rstest]
    #[case(r#""#)]
    #[case(r#""""Hello"""#)]
    #[case(r#"""Hello"""#)]
    #[case(r#""Hello""""#)]
    fn test_invalid_multiline_string(#[case] input: &str) {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let result = parser.parse_multiline_string();
        assert!(result.is_err());
    }

    #[rstest]
    #[case(vec![r#"""#,r#""""#, "Hello","World\"",r#""""#], "HelloWorld")]
    #[case(vec![r#"""#,r#""""#, "Hello","World\"\"",r#"""#], "HelloWorld")]
    fn test_multiline_string_increment_parse(
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
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        Ok(())
    }
}
