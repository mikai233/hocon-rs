use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::{
    parser::pure::{
        is_hocon_horizontal_whitespace, is_hocon_whitespace,
        read::{DecoderError, Read},
    },
    raw::raw_object::RawObject,
};

#[derive(Debug)]
pub(crate) struct Parser<R: Read> {
    pub(crate) reader: R,
    pub(crate) stack: Vec<Frame>,
}

#[derive(Debug)]
enum Frame {
    Array(Vec<RawValue>),
    Object {
        entries: Vec<ObjectField>,
        expecting_key: bool,
        current_key: Option<RawString>,
    },
}

impl<R: Read> Parser<R> {
    pub(crate) fn new(reader: R) -> Self {
        Parser { reader, stack: Vec::new() }
    }

    pub(crate) fn parse_horizontal_whitespace<'a>(
        &mut self,
        scratch: &'a mut Vec<u8>,
    ) -> Result<&'a str, DecoderError> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_horizontal_whitespace(ch) {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                    } else {
                        break;
                    }
                }
                Err(DecoderError::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        let s = unsafe { str::from_utf8_unchecked(scratch) };
        Ok(s)
    }

    pub(crate) fn drop_horizontal_whitespace(&mut self) -> Result<(), DecoderError> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_horizontal_whitespace(ch) {
                        self.reader.next()?;
                    } else {
                        break;
                    }
                }
                Err(DecoderError::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace(&mut self) -> Result<(), DecoderError> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_whitespace(ch) {
                        self.reader.next()?;
                    } else {
                        break;
                    }
                }
                Err(DecoderError::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn drop_comma_separator(&mut self) -> Result<bool, DecoderError> {
        match self.reader.peek() {
            Ok(ch) => {
                if ch == ',' {
                    self.reader.next()?;
                }
            }
            Err(DecoderError::Eof) => return Ok(true),
            Err(err) => {
                return Err(err);
            }
        }
        Ok(false)
    }

    pub(crate) fn parse(mut self) -> Result<RawObject, DecoderError> {
        self.drop_whitespace()?;
        let raw_obj = match self.reader.peek() {
            Ok(ch) => {
                if ch == '{' {
                    self.parse_object()?
                } else {
                    self.parse_root_object()?
                }
            }
            Err(DecoderError::Eof) => {
                return Ok(RawObject::default());
            }
            Err(err) => {
                return Err(err);
            }
        };
        self.drop_whitespace()?;
        match self.reader.peek() {
            Ok(ch) => {
                return Err(DecoderError::UnexpectedToken {
                    expected: "end of file",
                    found_beginning: ch,
                });
            }
            Err(DecoderError::Eof) => {}
            Err(err) => {
                return Err(err);
            }
        }
        Ok(raw_obj)
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use rstest::rstest;

    use crate::parser::pure::{
        parser::Parser,
        read::{DecoderError, StreamRead},
    };

    #[rstest]
    #[case("resources/base.conf")]
    #[case("resources/concat.conf")]
    #[case("resources/concat2.conf")]
    #[case("resources/concat3.conf")]
    #[case("resources/demo.conf")]
    #[case("resources/deserialize.conf")]
    #[case("resources/empty.conf")]
    #[case("resources/included.conf")]
    #[case("resources/main.conf")]
    // #[case("resources/max_depth.conf")]
    fn test_parse(#[case] path: impl AsRef<std::path::Path>) -> Result<(), DecoderError> {
        use crate::parser::pure::read::MIN_BUFFER_SIZE;

        let file = std::fs::File::open(path)?;
        let read: StreamRead<_, MIN_BUFFER_SIZE> = StreamRead::new(BufReader::new(file));
        let parser = Parser::new(read);
        let raw = parser.parse()?;
        tracing::debug!("{}", raw);
        Ok(())
    }
}
