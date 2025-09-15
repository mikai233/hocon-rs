use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::raw_string::RawString;

const FORBIDDEN_CHARACTERS: [u8; 19] = [
    b'$', b'"', b'{', b'}', b'[', b']', b':', b'=', b',', b'+', b'#', b'`', b'^', b'?', b'!', b'@',
    b'*', b'&', b'\\',
];

pub(crate) const TRIPLE_DOUBLE_QUOTE: [u8; 3] = [b'"', b'"', b'"'];

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_quoted_string(&mut self, check: bool) -> Result<String> {
        if check {
            let ch = self.reader.peek()?;
            if ch != b'"' {
                return Err(Error::UnexpectedToken {
                    expected: "\"",
                    found_beginning: ch,
                });
            }
        }
        self.reader.next()?;
        self.scratch.clear();
        let content = self
            .reader
            .parse_str(true, &mut self.scratch, |reader| Ok(reader.peek()? == b'"'))?
            .to_string();
        let ch = self.reader.peek()?;
        if ch != b'"' {
            return Err(Error::UnexpectedToken {
                expected: "\"",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        Ok(content)
    }

    pub(crate) fn parse_unquoted_string(&mut self) -> Result<String> {
        self.parse_unquoted(true)
    }

    pub(crate) fn parse_unquoted_path(&mut self) -> Result<String> {
        self.parse_unquoted(false)
    }

    fn parse_unquoted(&mut self, allow_dot: bool) -> Result<String> {
        self.scratch.clear();
        let content = self.reader.parse_str(true, &mut self.scratch, |reader| {
            let mut end = false;
            match reader.peek() {
                Ok(ch) => match ch {
                    b'/' => match reader.peek2() {
                        Ok((_, ch2)) => {
                            if ch2 == b'/' {
                                end = true;
                            }
                        }
                        Err(Error::Eof) => {}
                        Err(err) => return Err(err),
                    },
                    b'.' => {
                        if !allow_dot {
                            end = true;
                        }
                    }
                    ch => {
                        if FORBIDDEN_CHARACTERS.contains(&ch) || reader.starts_with_whitespace()? {
                            end = true;
                        }
                    }
                },
                Err(Error::Eof) => {
                    end = true;
                }
                Err(err) => return Err(err),
            }
            Ok(end)
        })?;
        if content.is_empty() {
            Err(Error::UnexpectedToken {
                expected: "a valid unquoted string",
                found_beginning: b'\0',
            })
        } else {
            Ok(content.to_string())
        }
    }

    pub(crate) fn parse_multiline_string(&mut self, verify_delimiter: bool) -> Result<String> {
        if verify_delimiter {
            let bytes = self.reader.peek_n::<3>()?;
            if bytes != TRIPLE_DOUBLE_QUOTE {
                let (_, ch) = bytes
                    .iter()
                    .enumerate()
                    .find(|(index, ch)| &&TRIPLE_DOUBLE_QUOTE[*index] != ch)
                    .unwrap();
                return Err(Error::UnexpectedToken {
                    expected: "\"\"\"",
                    found_beginning: *ch,
                });
            }
        }
        for _ in 0..3 {
            self.reader.next()?;
        }
        self.scratch.clear();
        let content = self
            .reader
            .parse_str(false, &mut self.scratch, |reader| {
                Ok(reader.peek_n::<3>()? == TRIPLE_DOUBLE_QUOTE)
            })?
            .to_string();
        for _ in 0..3 {
            self.reader.next()?;
        }
        Ok(content)
    }

    pub(crate) fn parse_path_expression(&mut self) -> Result<RawString> {
        let mut paths = vec![];
        let mut scratch = vec![];
        if self.reader.starts_with_horizontal_whitespace()? {
            return Err(Error::UnexpectedToken {
                expected: "a valid path expression",
                found_beginning: b' ',
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
                            found_beginning: b'\0',
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
                b'"' => {
                    // quoted string or multiline string
                    if let Ok(bytes) = self.reader.peek_n::<3>()
                        && bytes == TRIPLE_DOUBLE_QUOTE
                    {
                        self.parse_multiline_string(false)?
                    } else {
                        self.parse_quoted_string(false)?
                    }
                }
                _ => self.parse_unquoted_path()?,
            };
            scratch.extend_from_slice(path.as_bytes());
            let mut path = unsafe { str::from_utf8_unchecked(&scratch) }.to_string();
            // We always need to parse the ending whitespace after a path, because we don't
            // know if there are any valid path expressions after it.
            scratch.clear();
            self.parse_horizontal_whitespace(&mut scratch)?;
            let ending_space = unsafe { str::from_utf8_unchecked(&scratch) };
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
            const ENDING: [u8; 5] = [b':', b'{', b'=', b'}', b'+'];
            if ENDING.contains(&ch) {
                paths.push(path);
                break;
            } else if ch == b'.' {
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
    use crate::parser::read::StrRead;
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
        let s = parser.parse_quoted_string(true)?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.rest()?, rest);
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
        let result = parser.parse_quoted_string(true);
        assert!(result.is_err());
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
        assert_eq!(parser.reader.rest()?, rest);
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
        let s = parser.parse_multiline_string(true)?;
        assert_eq!(s, expected);
        assert_eq!(parser.reader.rest()?, rest);
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
        let result = parser.parse_multiline_string(true);
        assert!(result.is_err());
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
        assert_eq!(parser.reader.rest()?, rest);
        Ok(())
    }
}
