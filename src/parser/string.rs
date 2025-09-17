use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::raw_string::RawString;

// Precompute forbidden characters table
const FORBIDDEN_TABLE: [bool; 256] = {
    let mut table = [false; 256];
    table[b'$' as usize] = true;
    table[b'"' as usize] = true;
    table[b'{' as usize] = true;
    table[b'}' as usize] = true;
    table[b'[' as usize] = true;
    table[b']' as usize] = true;
    table[b':' as usize] = true;
    table[b'=' as usize] = true;
    table[b',' as usize] = true;
    table[b'+' as usize] = true;
    table[b'#' as usize] = true;
    table[b'`' as usize] = true;
    table[b'^' as usize] = true;
    table[b'?' as usize] = true;
    table[b'!' as usize] = true;
    table[b'@' as usize] = true;
    table[b'*' as usize] = true;
    table[b'&' as usize] = true;
    table[b'\\' as usize] = true;
    table
};

pub(crate) const TRIPLE_DOUBLE_QUOTE: &[u8] = b"\"\"\"";

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_quoted_string<F, T>(
        reader: &mut R,
        scratch: &mut Vec<u8>,
        check: bool,
        result: F,
    ) -> Result<T>
    where
        F: FnOnce(&str) -> Result<T>,
    {
        if check {
            match reader.peek() {
                Ok(b'"') => {}
                _ => {
                    return Err(reader.peek_error("\""));
                }
            }
        }
        reader.discard(1)?;
        scratch.clear();
        let content = reader.parse_str(true, scratch, |reader| match reader.peek() {
            Ok(b'"') => Ok(true),
            Ok(_) => Ok(false),
            _ => Err(reader.peek_error("\"")),
        })?;
        let r = result(&content)?;
        match reader.peek() {
            Ok(b'"') => {}
            _ => {
                return Err(reader.peek_error("\""));
            }
        }
        reader.discard(1)?;
        Ok(r)
    }

    pub(crate) fn parse_unquoted_string(reader: &mut R, scratch: &mut Vec<u8>) -> Result<String> {
        Self::parse_unquoted(reader, scratch, true, |s| Ok(s.to_string()))
    }

    pub(crate) fn parse_unquoted_path(reader: &mut R, scratch: &mut Vec<u8>) -> Result<Vec<u8>> {
        Self::parse_unquoted(reader, scratch, false, |s| Ok(Vec::from(s.as_bytes())))
    }

    fn parse_unquoted<F, T>(
        reader: &mut R,
        scratch: &mut Vec<u8>,
        allow_dot: bool,
        result: F,
    ) -> Result<T>
    where
        F: FnOnce(&str) -> Result<T>,
    {
        scratch.clear();
        let content = reader.parse_str(true, scratch, |reader| {
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
                        if FORBIDDEN_TABLE[ch as usize] || reader.starts_with_whitespace()? {
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
            Err(reader.peek_error("a valid unquoted string"))
        } else {
            result(&content)
        }
    }

    pub(crate) fn parse_multiline_string<F, T>(
        reader: &mut R,
        scratch: &mut Vec<u8>,
        verify_delimiter: bool,
        result: F,
    ) -> Result<T>
    where
        F: FnOnce(&str) -> Result<T>,
    {
        if verify_delimiter {
            match reader.peek_n(3) {
                Ok(bytes) if bytes == TRIPLE_DOUBLE_QUOTE => {}
                _ => {
                    return Err(reader.peek_error("\"\"\""));
                }
            }
        }
        reader.discard(3)?;
        scratch.clear();
        let content = reader.parse_str(false, scratch, |reader| match reader.peek_n(3) {
            Ok(bytes) if bytes == TRIPLE_DOUBLE_QUOTE => Ok(true),
            Ok(_) => Ok(false),
            _ => Err(reader.peek_error("\"\"\"")),
        })?;
        let r = result(&content)?;
        match reader.peek_n(3) {
            Ok(bytes) if bytes == TRIPLE_DOUBLE_QUOTE => {}
            _ => {
                return Err(reader.peek_error("\"\"\""));
            }
        }
        reader.discard(3)?;
        Ok(r)
    }

    pub(crate) fn parse_path_expression(
        reader: &mut R,
        scratch: &mut Vec<u8>,
    ) -> Result<RawString> {
        let mut paths = vec![];
        if reader.starts_with_horizontal_whitespace()? {
            return Err(reader.peek_error("a valid path expression"));
        }
        loop {
            scratch.clear();
            Self::parse_horizontal_whitespace(reader, scratch)?;
            let ch = match reader.peek() {
                Ok(ch) => ch,
                Err(Error::Eof) => {
                    if paths.is_empty() {
                        return Err(reader.peek_error("a valid path expression"));
                    } else {
                        break;
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            };
            let mut path_scratch = Vec::with_capacity(8);
            let path_bytes = match ch {
                b'"' => {
                    // quoted string or multiline string
                    if let Ok(bytes) = reader.peek_n(3)
                        && bytes == TRIPLE_DOUBLE_QUOTE
                    {
                        Self::parse_multiline_string(reader, &mut path_scratch, false, |s| {
                            Ok(Vec::from(s.as_bytes()))
                        })?
                    } else {
                        Self::parse_quoted_string(reader, &mut path_scratch, false, |s| {
                            Ok(Vec::from(s.as_bytes()))
                        })?
                    }
                }
                _ => Self::parse_unquoted_path(reader, &mut path_scratch)?,
            };
            scratch.extend(path_bytes);
            let mut path = unsafe { str::from_utf8_unchecked(scratch) }.to_string();
            // We always need to parse the ending whitespace after a path, because we don't
            // know if there are any valid path expressions after it.
            scratch.clear();
            Self::parse_horizontal_whitespace(reader, scratch)?;
            let ending_space = unsafe { str::from_utf8_unchecked(scratch) };
            let ch = match reader.peek() {
                Ok(ch) => ch,
                Err(Error::Eof) => {
                    paths.push(path);
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            };
            match ch {
                b':' | b'{' | b'=' | b'}' | b'+' => {
                    paths.push(path);
                    break;
                }
                b'.' => {
                    path.push_str(ending_space);
                    paths.push(path);
                    reader.discard(1)?;
                }
                _ => {
                    return Err(reader.peek_error("a valid path expression"));
                }
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
        parser.scratch.clear();
        let s =
            HoconParser::parse_quoted_string(&mut parser.reader, &mut parser.scratch, true, |s| {
                Ok(s.to_string())
            })?;
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
        let result =
            HoconParser::parse_quoted_string(&mut parser.reader, &mut parser.scratch, true, |s| {
                Ok(s.to_string())
            });
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
        parser.scratch.clear();
        let s = HoconParser::parse_unquoted_string(&mut parser.reader, &mut parser.scratch)?;
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
        let s = HoconParser::parse_multiline_string(
            &mut parser.reader,
            &mut parser.scratch,
            true,
            |s| Ok(s.to_string()),
        )?;
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
        let result = HoconParser::parse_multiline_string(
            &mut parser.reader,
            &mut parser.scratch,
            true,
            |s| Ok(s.to_string()),
        );
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
        parser.scratch.clear();
        let s = HoconParser::parse_path_expression(&mut parser.reader, &mut parser.scratch)?;
        assert_eq!(s.to_string(), expected);
        assert_eq!(parser.reader.rest()?, rest);
        Ok(())
    }
}
