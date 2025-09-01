use crate::{
    parser::pure::{
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::substitution::Substitution,
};

impl<R: Read> Parser<R> {
    pub(crate) fn parse_substitution(&mut self) -> Result<Substitution, DecoderError> {
        let (ch1, ch2) = self.reader.peek2()?;
        if ch1 != '$' {
            return Err(DecoderError::UnexpectedToken {
                expected: "$",
                found_beginning: ch1,
            });
        }
        self.reader.next()?;
        if ch2 != '{' {
            return Err(DecoderError::UnexpectedToken {
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
            return Err(DecoderError::UnexpectedToken {
                expected: "}",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let substitution = Substitution::new(path_expression, optional, None);
        Ok(substitution)
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use rstest::rstest;

    use crate::parser::pure::{
        parser::Parser,
        read::{DecoderError, TestStreamRead},
    };

    #[rstest]
    #[case("${a}", "${a}")]
    #[case("${foo .bar }", "${foo .bar}")]
    #[case(r#"${a. b."c"}"#, "${a. b.c}")]
    #[case(r#"${? a. b."c"}"#, "${?a. b.c}")]
    #[case(r#"${? """a""". b."c"}"#, "${?a. b.c}")]
    fn test_valid_path_expression(
        #[case] input: &str,
        #[case] expected: &str,
    ) -> Result<(), DecoderError> {
        let read = TestStreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = Parser::new(read);
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
        let mut parser = Parser::new(read);
        let result = parser.parse_substitution();
        assert!(result.is_err());
    }
}
