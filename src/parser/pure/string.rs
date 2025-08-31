use memchr::memchr;

use crate::{
    parser::pure::{
        is_hocon_whitespace,
        parser::{empty_callback, Parser},
        read::{DecoderError, Read},
    },
    raw::raw_string::RawString,
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
    NeedsMore { is_start: bool },
    Finished,
}

enum PathExpressionState {
    FirstPath,
    Path,
    Dot,
    Space(Box<PathExpressionState>),
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
                            return Err(DecoderError::unexpected_token("\"", s));
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
                    QuotedStringState::NeedsMore(need) => {
                        let eof = self.fill_buf()?;
                        if eof {
                            return Err(DecoderError::UnexpectedEof);
                        }
                        if self.reader.has_at_least_n_chars(need) {
                            state = QuotedStringState::InProgress;
                        }
                    }
                    QuotedStringState::Finished => {
                        break;
                    }
                },
                None => {
                    if self.fill_buf()? {
                        break;
                    }
                }
            }
            self.reader.consume(consume_len_utf8);
            consume_len_utf8 = 0;
        }
        if !matches!(state, QuotedStringState::Finished) {
            return Err(DecoderError::UnexpectedEof);
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
                *state = QuotedStringState::NeedsMore(2);
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
        self.parse_unquoted(true)
    }

    pub(crate) fn parse_unquoted_path(&mut self) -> Result<String, DecoderError> {
        self.parse_unquoted(false)
    }

    fn parse_unquoted(&mut self, allow_dot: bool) -> Result<String, DecoderError> {
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
                                } else if !allow_dot && ch == '.' {
                                    state = UnquotedStringState::Finished;
                                    break;
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
                UnquotedStringState::NeedsMore => {
                    let eof = self.fill_buf()?;
                    if eof {
                        consume_len_utf8 += '/'.len_utf8();
                        string.push('/');
                    } else {
                        state = UnquotedStringState::InProgress;
                    }
                }
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

    pub(crate) fn parse_multiline_string(&mut self) -> Result<String, DecoderError> {
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
                            state = MultilineStringState::NeedsMore { is_start: true };
                        } else {
                            return Err(DecoderError::unexpected_token(r#"""""#, s));
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
                                let chunk = if i != s.len() { s } else { &s[..i] };
                                string.push_str(chunk);
                                consume_len_utf8 += chunk.len();
                                state = MultilineStringState::NeedsMore { is_start: false };
                            }
                        }
                        None => {
                            string.push_str(s);
                            consume_len_utf8 += s.len();
                            state = MultilineStringState::NeedsMore { is_start: false };
                        }
                    },
                    MultilineStringState::NeedsMore { is_start } => {
                        let eof = self.fill_buf()?;
                        if eof {
                            return Err(DecoderError::UnexpectedEof);
                        } else {
                            if self.reader.has_at_least_n_chars(3) {
                                if is_start {
                                    state = MultilineStringState::Start;
                                } else {
                                    state = MultilineStringState::InProgress;
                                }
                            }
                        }
                    }
                    MultilineStringState::Finished => {
                        break;
                    }
                },
                None => {
                    if self.fill_buf()? {
                        break;
                    }
                }
            }
            self.reader.consume(consume_len_utf8);
            consume_len_utf8 = 0;
        }
        if !matches!(state, MultilineStringState::Finished) {
            return Err(DecoderError::UnexpectedEof);
        }
        Ok(string)
    }

    pub(crate) fn parse_path_expression(&mut self) -> Result<RawString, DecoderError> {
        let mut state = PathExpressionState::FirstPath;
        let mut paths = vec![];
        let mut last_space = String::new();
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    PathExpressionState::FirstPath => {
                        self.parse_leading_horizontal_whitespace(empty_callback)?;
                        let path = self.parse_path()?;
                        paths.push(path);
                        state = PathExpressionState::Space(Box::new(PathExpressionState::Dot));
                    }
                    PathExpressionState::Dot => {
                        if s.starts_with('.') {
                            state = PathExpressionState::Path;
                            self.reader.consume(1);
                        } else if !paths.is_empty() {
                            state = PathExpressionState::Finished;
                        } else {
                            return Err(DecoderError::unexpected_token(".", s));
                        }
                    }
                    PathExpressionState::Space(next) => {
                        self.parse_leading_horizontal_whitespace(|s| {
                            last_space.push_str(s);
                            Ok(())
                        })?;
                        state = *next;
                    }
                    PathExpressionState::Finished => {
                        break;
                    }
                    PathExpressionState::Path => {
                        let mut leading_space = String::new();
                        self.parse_leading_horizontal_whitespace(|s| {
                            leading_space.push_str(s);
                            Ok(())
                        })?;
                        match self.parse_path() {
                            Ok(path) => {
                                paths.last_mut().unwrap().push_str(&last_space);
                                last_space.clear();
                                leading_space.push_str(&path);
                                paths.push(leading_space);
                                state =
                                    PathExpressionState::Space(Box::new(PathExpressionState::Dot));
                            }
                            Err(_) => {
                                state = PathExpressionState::Finished;
                            }
                        }
                    }
                },
                None => {
                    if self.fill_buf()? {
                        break;
                    }
                }
            }
        }
        if paths.is_empty() {
            let ch = self
                .reader
                .peek_chunk()
                .and_then(|s| s.chars().next())
                .unwrap_or_default();
            return Err(DecoderError::UnexpectedToken {
                expected: "a valid path",
                found_beginning: ch,
            });
        }
        let path = if paths.len() == 1 {
            RawString::quoted(paths.remove(0))
        } else {
            let last_index = paths.len() - 1;
            RawString::concat(paths.into_iter().enumerate().map(|(index, p)| {
                if index != last_index {
                    (RawString::quoted(p), Some("."))
                } else {
                    (RawString::quoted(p), None)
                }
            }))
        };
        Ok(path)
    }

    fn parse_path(&mut self) -> Result<String, DecoderError> {
        let path = if let Ok(string) = self.parse_unquoted_path() {
            string
        } else if let Ok(string) = self.parse_multiline_string() {
            string
        } else if let Ok(string) = self.parse_quoted_string() {
            string
        } else {
            let ch = self
                .reader
                .peek_chunk()
                .and_then(|c| c.chars().next())
                .unwrap_or_default();
            return Err(DecoderError::UnexpectedToken {
                expected: "a valid path",
                found_beginning: ch,
            });
        };
        Ok(path)
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
    #[case(vec!["\"\\", "r\""], "\r")]
    fn test_quoted_string_increment_parse(
        #[case] input: Vec<&str>,
        #[case] expected: &str,
    ) -> Result<(), DecoderError> {
        let read = TestRead::from_input(input);
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
        let read = TestRead::from_input(input);
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
        #[case] remains: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.peek_chunk(), remains);
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
    #[case(vec![r#"""#,r#""""#, "Hello\"","World\"\"",r#"""#], "Hello\"World")]
    #[case(vec![r#"""#,r#""""#, "Hello\"\"","World\"\"",r#"""#], "Hello\"\"World")]
    fn test_multiline_string_increment_parse(
        #[case] input: Vec<&str>,
        #[case] expected: &str,
    ) -> Result<(), DecoderError> {
        let read = TestRead::from_input(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        Ok(())
    }

    #[rstest]
    #[case(r#"a.b.c "#, "a.b.c", None)]
    #[case(r#"a. b.c "#, "a. b.c", None)]
    #[case(r#"a. "..".c "#, "a. ...c", None)]
    #[case(r#"  a. "..".c   "#, "a. ...c", None)]
    #[case(r#"a.b.c,"#, "a.b.c", Some(","))]
    #[case(r#"a. """b""" . c }"#, "a. b . c", Some("}"))]
    fn test_valid_path_expression(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] remains: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = SliceRead::new(input);
        let mut parser = Parser::new(read);
        let s = parser.parse_path_expression()?;
        assert_eq!(s.to_string(), expected);
        assert_eq!(parser.reader.peek_chunk(), remains);
        Ok(())
    }
}
