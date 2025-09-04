use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::Read;
use crate::parser::{is_hocon_horizontal_whitespace, is_hocon_whitespace};
use crate::{raw::raw_string::RawString, try_peek};

const FORBIDDEN_CHARACTERS: [char; 19] = [
    '$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\',
];

pub(crate) const TRIPLE_DOUBLE_QUOTE: [char; 3] = ['"', '"', '"'];

impl<R: Read> HoconParser<R> {
    pub(crate) fn parse_quoted_string(&mut self) -> Result<String> {
        let mut scratch = vec![];
        let ch = self.reader.peek()?;
        if ch != '"' {
            return Err(Error::UnexpectedToken {
                expected: "\"",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        loop {
            match self.reader.next()? {
                ('\\', _) => {
                    let ch = self.parse_escaped_char()?; // TODO return [u8] directly
                    let mut b = [0; 4];
                    ch.encode_utf8(&mut b);
                    scratch.extend_from_slice(&b[..ch.len_utf8()]);
                }
                ('"', _) => {
                    break;
                }
                (_, bytes) => {
                    scratch.extend_from_slice(bytes);
                }
            }
        }
        let s = unsafe { str::from_utf8_unchecked(&scratch) };
        Ok(String::from(s))
    }

    fn parse_escaped_char(&mut self) -> Result<char> {
        let (ch, _) = self.reader.next()?;
        let ch = match ch {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\x08',
            'f' => '\x0C',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                return self.parse_escaped_unicode();
            }
            _ => return Err(Error::InvalidEscape),
        };
        Ok(ch)
    }

    fn parse_escaped_unicode(&mut self) -> Result<char> {
        fn unicode_code<R: Read>(reader: &mut R) -> Result<String> {
            let mut code = String::new();
            for _ in 0..4 {
                match reader.next()? {
                    (digit, _) if digit.is_ascii_hexdigit() => {
                        code.push(digit);
                    }
                    _ => {
                        return Err(Error::InvalidEscape);
                    }
                }
            }
            Ok(code)
        }
        let high_code = unicode_code(&mut self.reader)?;
        let high = u32::from_str_radix(&high_code, 16).map_err(|_| Error::InvalidEscape)?;
        if (0xD800..=0xDBFF).contains(&high) {
            // 需要再读一个 \uXXXX
            if self.reader.next()?.0 != '\\' {
                return Err(Error::InvalidEscape);
            }
            if self.reader.next()?.0 != 'u' {
                return Err(Error::InvalidEscape);
            }
            let low_code = unicode_code(&mut self.reader)?;
            let low = u32::from_str_radix(&low_code, 16).map_err(|_| Error::InvalidEscape)?;

            if (0xDC00..=0xDFFF).contains(&low) {
                // 合并 surrogate pair
                let codepoint = 0x10000 + (((high - 0xD800) << 10) | (low - 0xDC00));
                let ch = char::from_u32(codepoint).ok_or(Error::InvalidEscape)?;
                Ok(ch)
            } else {
                Err(Error::InvalidEscape)
            }
        } else {
            let ch = char::from_u32(high).ok_or(Error::InvalidEscape)?;
            Ok(ch)
        }
    }

    pub(crate) fn parse_unquoted_string(&mut self) -> Result<String> {
        self.parse_unquoted(true)
    }

    pub(crate) fn parse_unquoted_path(&mut self) -> Result<String> {
        self.parse_unquoted(false)
    }

    fn parse_unquoted(&mut self, allow_dot: bool) -> Result<String> {
        let mut scratch = vec![];
        loop {
            let ch = try_peek!(self.reader);
            match ch {
                '/' => match self.reader.peek2() {
                    Ok((_, ch2)) => {
                        if ch2 == '/' {
                            break;
                        } else {
                            let (_, bytes) = self.reader.next()?;
                            scratch.extend_from_slice(bytes);
                        }
                    }
                    Err(Error::Eof) => {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                        break;
                    }
                    Err(err) => {
                        return Err(err);
                    }
                },
                '.' => {
                    if allow_dot {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                    } else {
                        break;
                    }
                }
                ch => {
                    if !FORBIDDEN_CHARACTERS.contains(&ch) && !is_hocon_whitespace(ch) {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                    } else {
                        break;
                    }
                }
            }
        }
        if scratch.is_empty() {
            Err(Error::UnexpectedToken {
                expected: "a valid unquoted string",
                found_beginning: '\0',
            })
        } else {
            let s = unsafe { str::from_utf8_unchecked(&scratch) };
            Ok(s.to_string())
        }
    }

    pub(crate) fn parse_multiline_string(&mut self) -> Result<String> {
        let mut scratch = vec![];
        let chars = self.reader.peek_n::<3>()?;
        if chars != TRIPLE_DOUBLE_QUOTE {
            let (_, ch) = chars
                .iter()
                .enumerate()
                .find(|(index, ch)| &&TRIPLE_DOUBLE_QUOTE[*index] != ch)
                .unwrap();
            return Err(Error::UnexpectedToken {
                expected: "\"\"\"",
                found_beginning: *ch,
            });
        }
        for _ in 0..3 {
            self.reader.next()?;
        }
        loop {
            let chars = self.reader.peek_n::<3>()?;
            if chars == TRIPLE_DOUBLE_QUOTE {
                for _ in chars {
                    self.reader.next()?;
                }
                break;
            }
            let (_, bytes) = self.reader.next()?;
            scratch.extend_from_slice(bytes);
        }
        let s = unsafe { str::from_utf8_unchecked(&scratch) };
        Ok(String::from(s))
    }

    pub(crate) fn parse_path_expression(&mut self) -> Result<RawString> {
        let mut paths = vec![];
        let mut scratch = vec![];
        let ch = self.reader.peek()?;
        if is_hocon_horizontal_whitespace(ch) {
            return Err(Error::UnexpectedToken {
                expected: "a valid path expression",
                found_beginning: ch,
            });
        }
        loop {
            scratch.clear();
            self.parse_horizontal_whitespace(&mut scratch)?;
            let ch = match self.reader.peek() {
                Ok(ch) => ch,
                Err(Error::Eof) => {
                    if paths.is_empty() {
                        return Err(Error::UnexpectedToken {
                            expected: "a valid path expression",
                            found_beginning: '\0',
                        });
                    } else {
                        break;
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            };
            let path = match ch {
                '"' => {
                    // quoted string or multiline string
                    if let Ok(chars) = self.reader.peek_n::<3>()
                        && chars == TRIPLE_DOUBLE_QUOTE
                    {
                        self.parse_multiline_string()?
                    } else {
                        self.parse_quoted_string()?
                    }
                }
                _ => self.parse_unquoted_path()?,
            };
            scratch.extend_from_slice(path.as_bytes());
            let mut path = unsafe { str::from_utf8_unchecked(&scratch) }.to_string();
            // We always need to parse the ending whitespace after a path, because we don't
            // know if there are any valid path expressions after it.
            scratch.clear();
            let ending_space = self.parse_horizontal_whitespace(&mut scratch)?;
            let ch = match self.reader.peek() {
                Ok(ch) => ch,
                Err(Error::Eof) => {
                    paths.push(path);
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            };
            const ENDING: [char; 5] = [':', '{', '=', '}', '+'];
            if ENDING.contains(&ch) {
                paths.push(path);
                break;
            } else if ch == '.' {
                path.push_str(ending_space);
                paths.push(path);
                self.reader.next()?;
            } else {
                return Err(Error::UnexpectedToken {
                    expected: "a valid path expression",
                    found_beginning: ch,
                });
            }
        }
        // After the loop, the paths vector must not be empty.
        debug_assert!(!paths.is_empty());
        let path = if paths.len() == 1 {
            RawString::quoted(paths.remove(0))
        } else {
            RawString::path_expression(paths.into_iter().map(RawString::quoted).collect())
        };
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::parser::HoconParser;
    use crate::parser::read::{StrRead, TestRead};
    use rstest::rstest;

    #[rstest]
    #[case("\"hello\"", "hello", "")]
    #[case("\"hello\\nworld\"", "hello\nworld", "")]
    #[case(
        r#""line1\nline2\tindent\\slash\"quote""#,
        "line1\nline2\tindent\\slash\"quote",
        ""
    )]
    #[case(r#""\u4F60\u597D""#, "你好", "")]
    #[case(r#""\uD83D\uDE00""#, "😀", "")]
    #[case(r#""Hello \u4F60\u597D \n 😀!""#, "Hello 你好 \n 😀!", "")]
    fn test_valid_quoted_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_quoted_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.rest(), rest);
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
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
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
    ) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_quoted_string()?;
        assert_eq!(s, expected);
        Ok(())
    }

    #[rstest]
    #[case("a.b.c", "a.b.c", "")]
    #[case("a.b.c//", "a.b.c", "//")]
    #[case("a.b.c/b", "a.b.c/b", "")]
    #[case("hello#world", "hello", "#world")]
    #[case("你 好", "你", " 好")]
    #[case("你 \\r\n不好", "你", " \\r\n不好")]
    #[case("你 \r\n不好", "你", " \r\n不好")]
    #[case("a，\n", "a，", "\n")]
    fn test_valid_unquoted_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_unquoted_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.rest(), rest);
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
    ) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_unquoted_string()?;
        assert_eq!(s, expected);
        Ok(())
    }

    #[rstest]
    #[case(r#""""a.bbc""""#, "a.bbc", "")]
    #[case(r#""""a.bbc😀"""😀"#, "a.bbc😀", "😀")]
    #[case(r#""""a.b\r\nbc""""#, "a.b\\r\\nbc", "")]
    fn test_valid_multiline_string(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.rest(), rest);
        Ok(())
    }

    #[rstest]
    #[case(r#""#)]
    #[case(r#""""Hello"""#)]
    #[case(r#"""Hello"""#)]
    #[case(r#""Hello""""#)]
    fn test_invalid_multiline_string(#[case] input: &str) {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
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
    ) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_multiline_string()?;
        assert_eq!(s, expected);
        Ok(())
    }

    #[rstest]
    #[case(r#"a.b.c "#, "a.b.c", "")]
    #[case(r#"a. b.c "#, "a. b.c", "")]
    #[case(r#"a. "..".c "#, "a. ...c", "")]
    #[case(r#"a.b.c :"#, "a.b.c", ":")]
    #[case(r#"a.b.c ="#, "a.b.c", "=")]
    #[case(r#"a.b.c{"#, "a.b.c", "{")]
    #[case(r#"a. """b""" . c }"#, "a. b . c", "}")]
    fn test_valid_path_expression(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let s = parser.parse_path_expression()?;
        assert_eq!(s.to_string(), expected);
        assert_eq!(parser.reader.rest(), rest);
        Ok(())
    }
}
