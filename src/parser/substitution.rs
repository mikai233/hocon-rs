use crate::error::Error;
use crate::parser::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::substitution::Substitution;
use crate::Result;

impl<R: Read> HoconParser<R> {
    pub(crate) fn parse_substitution(&mut self) -> Result<Substitution> {
        let (ch1, ch2) = self.reader.peek2()?;
        if ch1 != '$' {
            return Err(Error::UnexpectedToken {
                expected: "$",
                found_beginning: ch1,
            });
        }
        self.reader.next()?;
        if ch2 != '{' {
            return Err(Error::UnexpectedToken {
                expected: "{",
                found_beginning: ch2,
            });
        }
        self.reader.next()?;
        let ch = self.reader.peek()?;
        let optional = if ch == '?' {
            self.reader.next()?;
            true
        } else {
            false
        };
        self.drop_horizontal_whitespace()?;
        let path_expression = self.parse_path_expression()?;
        let ch = self.reader.peek()?;
        if ch != '}' {
            return Err(Error::UnexpectedToken {
                expected: "}",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let substitution = Substitution::new(path_expression, optional);
        Ok(substitution)
    }
}

#[cfg(test)]
mod tests {
    use crate::Result;
    use std::io::BufReader;

    use crate::parser::parser::HoconParser;
    use crate::parser::read::TestStreamRead;
    use rstest::rstest;

    #[rstest]
    #[case("${a}", "${a}")]
    #[case("${foo .bar }", "${foo .bar}")]
    #[case(r#"${a. b."c"}"#, "${a. b.c}")]
    #[case(r#"${? a. b."c"}"#, "${?a. b.c}")]
    #[case(r#"${? """a""". b."c"}"#, "${?a. b.c}")]
    fn test_valid_path_expression(#[case] input: &str, #[case] expected: &str) -> Result<()> {
        let read = TestStreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = HoconParser::new(read);
        let substitution = parser.parse_substitution()?;
        assert_eq!(substitution.to_string(), expected);
        Ok(())
    }

    #[rstest]
    #[case("${foo .bar")]
    #[case("${ ?foo.bar}")]
    #[case("${?foo.bar.}")]
    #[case("${?foo.bar")]
    fn test_invalid_path_expression(#[case] input: &str) {
        let read = TestStreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = HoconParser::new(read);
        let result = parser.parse_substitution();
        assert!(result.is_err());
    }
}
