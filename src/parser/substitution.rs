use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::substitution::Substitution;

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_substitution(&mut self) -> Result<Substitution> {
        let (ch1, ch2) = self.reader.peek2()?;
        if ch1 != b'$' {
            return Err(Error::UnexpectedToken {
                expected: "$",
                found_beginning: ch1,
            });
        }
        self.reader.discard(1)?;
        if ch2 != b'{' {
            return Err(Error::UnexpectedToken {
                expected: "{",
                found_beginning: ch2,
            });
        }
        self.reader.discard(1)?;
        let ch = self.reader.peek()?;
        let optional = if ch == b'?' {
            self.reader.discard(1)?;
            true
        } else {
            false
        };
        self.drop_horizontal_whitespace()?;
        let path_expression = self.parse_path_expression()?;
        let ch = self.reader.peek()?;
        if ch != b'}' {
            return Err(Error::UnexpectedToken {
                expected: "}",
                found_beginning: ch,
            });
        }
        self.reader.discard(1)?;
        let substitution = Substitution::new(path_expression, optional);
        Ok(substitution)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Result, parser::read::StreamRead};
    use std::io::BufReader;

    use crate::parser::HoconParser;
    use rstest::rstest;

    #[rstest]
    #[case("${a}", "${a}")]
    #[case("${foo .bar }", "${foo .bar}")]
    #[case(r#"${a. b."c"}"#, "${a. b.c}")]
    #[case(r#"${? a. b."c"}"#, "${?a. b.c}")]
    #[case(r#"${? """a""". b."c"}"#, "${?a. b.c}")]
    fn test_valid_path_expression(#[case] input: &str, #[case] expected: &str) -> Result<()> {
        let read = StreamRead::new(BufReader::new(input.as_bytes()));
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
        let read = StreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = HoconParser::new(read);
        let result = parser.parse_substitution();
        assert!(result.is_err());
    }
}
