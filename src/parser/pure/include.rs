use crate::{
    parser::pure::{
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::include::{Inclusion, Location},
};

pub(crate) const INCLUDE: [char; 7] = ['i', 'n', 'c', 'l', 'u', 'd', 'e'];

impl<R: Read> Parser<R> {
    pub(crate) fn parse_include(&mut self) -> Result<Inclusion, DecoderError> {
        let mut location: Option<Location> = None;
        let mut required = false;
        let ch = self.reader.peek()?;
        if ch != 'i' {
            return Err(DecoderError::UnexpectedToken {
                expected: "include",
                found_beginning: ch,
            });
        }
        // At this point, we still don't know if it's an include or something else,
        // so we need to use peek instead of consuming it
        const N: usize = 7;
        let chars = self.reader.peek_n::<N>()?;
        if chars != INCLUDE {
            let (_, ch) = chars
                .iter()
                .enumerate()
                .find(|(index, ch)| &&INCLUDE[*index] != ch)
                .unwrap();
            return Err(DecoderError::UnexpectedToken {
                expected: "include",
                found_beginning: *ch,
            });
        }
        for _ in 0..N {
            self.reader.next()?;
        }

        self.drop_horizontal_whitespace()?;
        let ch = self.reader.peek()?;
        if ch == 'r' {
            for ele in ['r', 'e', 'q', 'u', 'i', 'r', 'e', 'd', '('] {
                let (next, _) = self.reader.next()?;
                if ele != next {
                    return Err(DecoderError::UnexpectedToken {
                        expected: "required(",
                        found_beginning: next,
                    });
                }
            }
            required = true
        }
        if required {
            self.drop_horizontal_whitespace()?;
        }
        let ch = self.reader.peek()?;
        match ch {
            'f' => {
                for ele in ['f', 'i', 'l', 'e', '('] {
                    let (next, _) = self.reader.next()?;
                    if ele != next {
                        return Err(DecoderError::UnexpectedToken {
                            expected: "file(",
                            found_beginning: next,
                        });
                    }
                }
                location = Some(Location::File);
            }
            'u' => {
                for ele in ['u', 'r', 'l', '('] {
                    let (next, _) = self.reader.next()?;
                    if ele != next {
                        return Err(DecoderError::UnexpectedToken {
                            expected: "url(",
                            found_beginning: next,
                        });
                    }
                }
                location = Some(Location::Url);
            }
            'c' => {
                for ele in ['c', 'l', 'a', 's', 's', 'p', 'a', 't', 'h', '('] {
                    let (next, _) = self.reader.next()?;
                    if ele != next {
                        return Err(DecoderError::UnexpectedToken {
                            expected: "classpath(",
                            found_beginning: next,
                        });
                    }
                }
                location = Some(Location::Classpath);
            }
            '"' => {}
            ch => {
                return Err(DecoderError::UnexpectedToken {
                    expected: "file( or classpath( or url( or \"",
                    found_beginning: ch,
                });
            }
        }
        if location.is_some() {
            self.drop_horizontal_whitespace()?;
        }
        let include_path = self.parse_quoted_string()?;
        for _ in [location.is_some(), required].iter().filter(|x| **x) {
            self.drop_horizontal_whitespace()?;
            let ch = self.reader.peek()?;
            if ch != ')' {
                return Err(DecoderError::UnexpectedToken {
                    expected: ")",
                    found_beginning: ch,
                });
            } else {
                self.reader.next()?;
            }
        }
        let inclusion = Inclusion::new(include_path, required, location, None);
        Ok(inclusion)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::pure::{
        parser::Parser,
        read::{DecoderError, Read, TestRead},
    };

    #[rstest]
    #[case(vec!["i","nclude"," ", "\"demo\".conf"],"include \"demo\"", Some(".conf"))]
    #[case(vec!["i","nclude", "\"demo.conf\""],"include \"demo.conf\"", None)]
    #[case(vec!["i","nclude","   r","equired(  ", "  \"demo.conf\" ",")"],"include required(\"demo.conf\")", None
    )]
    #[case(vec!["i","nclude","   r","equired(  ", "file(  \"demo.conf\" )",")"],"include required(file(\"demo.conf\"))", None
    )]
    fn test_valid_include(
        #[case] input: Vec<&str>,
        #[case] expected: &str,
        #[case] remains: Option<&str>,
    ) -> Result<(), DecoderError> {
        let read = TestRead::from_input(input);
        let mut parser = Parser::new(read);
        let inclusion = parser.parse_include()?;
        assert_eq!(inclusion.to_string(), expected);
        assert_eq!(parser.reader.peek_chunk(), remains);
        Ok(())
    }

    #[rstest]
    #[case(vec!["include", "demo"])]
    #[case(vec!["include", "required (\"demo\")"])]
    #[case(vec!["include", "required(\"demo\",)"])]
    #[case(vec!["include", "required(\"demo\""])]
    #[case(vec!["include", "required1(\"demo\")"])]
    #[case(vec!["include", "classpat(\"demo\")"])]
    #[case(vec!["include", "classpath(file(\"demo\"))"])]
    #[case(vec!["include", "classpath(required(\"demo\"))"])]
    fn test_invalid_include(#[case] input: Vec<&str>) -> Result<(), DecoderError> {
        let read = TestRead::from_input(input);
        let mut parser = Parser::new(read);
        let result = parser.parse_include();
        assert!(result.is_err());
        Ok(())
    }
}
