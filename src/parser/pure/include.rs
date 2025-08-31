use crate::{
    parser::pure::{
        leading_horizontal_whitespace,
        parser::{empty_callback, Parser},
        read::{DecoderError, Read},
    },
    raw::include::{Inclusion, Location},
};

enum InclusionState {
    Start,
    BeginRequired,
    BeginLocation,
    Path,
    EndLocation,
    EndRequired,
    NeedsMore(Box<InclusionState>),
    Finished,
}

impl<R: Read> Parser<R> {
    pub(crate) fn parse_include(&mut self) -> Result<Inclusion, DecoderError> {
        let mut location: Option<Location> = None;
        let mut required = false;
        let mut path = String::new();
        let mut state = InclusionState::Start;
        let mut consume_len_utf8 = 0;
        loop {
            match self.reader.peek_chunk() {
                Some(s) => match state {
                    InclusionState::Start => {
                        if s.starts_with("include") {
                            state = InclusionState::BeginRequired;
                            consume_len_utf8 += 7;
                        } else if s.starts_with('i') {
                            state = InclusionState::NeedsMore(Box::new(state));
                        } else {
                            return Err(DecoderError::unexpected_token("include", s));
                        }
                    }
                    InclusionState::BeginRequired => {
                        let (f, s) = leading_horizontal_whitespace(s);
                        if s.starts_with("required(") {
                            required = true;
                            state = InclusionState::BeginLocation;
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 9;
                        } else if s.starts_with('r') || s.is_empty() {
                            state = InclusionState::NeedsMore(Box::new(state));
                        } else {
                            state = InclusionState::BeginLocation;
                        }
                    }
                    InclusionState::BeginLocation => {
                        let (f, s) = leading_horizontal_whitespace(s);
                        if s.starts_with("file(") {
                            location = Some(Location::File);
                            state = InclusionState::Path;
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 5;
                        } else if s.starts_with("url(") {
                            location = Some(Location::Url);
                            state = InclusionState::Path;
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 4;
                        } else if s.starts_with("classpath(") {
                            location = Some(Location::Classpath);
                            state = InclusionState::Path;
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 10;
                        } else if s.is_empty()
                            || s.starts_with('f')
                            || s.starts_with('u')
                            || s.starts_with('c')
                        {
                            state = InclusionState::NeedsMore(Box::new(state));
                        } else {
                            state = InclusionState::Path;
                        }
                    }
                    InclusionState::Path => {
                        self.parse_leading_horizontal_whitespace(empty_callback)?;
                        path = self.parse_quoted_string()?;
                        if location.is_some() {
                            state = InclusionState::EndLocation;
                        } else if required {
                            state = InclusionState::EndRequired;
                        } else {
                            state = InclusionState::Finished;
                        }
                    }
                    InclusionState::EndLocation => {
                        let (f, s) = leading_horizontal_whitespace(s);
                        if s.starts_with(')') {
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 1;
                            if required {
                                state = InclusionState::EndRequired;
                            } else {
                                state = InclusionState::Finished;
                            }
                        } else if s.is_empty() {
                            state = InclusionState::NeedsMore(Box::new(state));
                        } else {
                            return Err(DecoderError::unexpected_token(")", s));
                        }
                    }
                    InclusionState::EndRequired => {
                        let (f, s) = leading_horizontal_whitespace(s);
                        if s.starts_with(')') {
                            consume_len_utf8 += f.len();
                            consume_len_utf8 += 1;
                            state = InclusionState::Finished
                        } else if s.is_empty() {
                            state = InclusionState::NeedsMore(Box::new(state));
                        } else {
                            return Err(DecoderError::unexpected_token(")", s));
                        }
                    }
                    InclusionState::NeedsMore(previous) => {
                        let eof = self.fill_buf()?;
                        if eof {
                            return Err(DecoderError::UnexpectedEof);
                        } else {
                            state = *previous;
                        }
                    }
                    InclusionState::Finished => {
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
        if !matches!(state, InclusionState::Finished) {
            return Err(DecoderError::UnexpectedEof);
        }
        let inclusion = Inclusion::new(path, required, location, None);
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
