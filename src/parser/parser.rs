use std::cell::RefCell;

use derive_more::Constructor;

use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::read::Read;
use crate::parser::{is_hocon_horizontal_whitespace, is_hocon_whitespace};
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;

#[derive(Constructor, Default)]
pub(crate) struct Context {
    pub(crate) include_chain: Vec<String>,
    pub(crate) depth: usize,
}

thread_local! {
   pub(crate) static CTX: RefCell<Context> = Context::default().into()
}

#[derive(Debug)]
pub struct HoconParser<R: Read> {
    pub(crate) reader: R,
    pub(crate) stack: Vec<Frame>,
    pub(crate) options: ConfigOptions,
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

impl<R: Read> HoconParser<R> {
    pub fn new(reader: R) -> Self {
        HoconParser {
            reader,
            stack: Vec::new(),
            options: Default::default(),
        }
    }

    pub fn with_options(reader: R, options: ConfigOptions) -> Self {
        HoconParser {
            reader,
            stack: Vec::new(),
            options,
        }
    }

    pub(crate) fn parse_horizontal_whitespace<'a>(
        &mut self,
        scratch: &'a mut Vec<u8>,
    ) -> Result<&'a str> {
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
                Err(Error::Eof) => {
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

    pub(crate) fn drop_horizontal_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_horizontal_whitespace(ch) {
                        self.reader.next()?;
                    } else {
                        break;
                    }
                }
                Err(Error::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_whitespace(ch) {
                        self.reader.next()?;
                    } else {
                        break;
                    }
                }
                Err(Error::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn drop_comma_separator(&mut self) -> Result<bool> {
        match self.reader.peek() {
            Ok(ch) => {
                if ch == ',' {
                    self.reader.next()?;
                }
            }
            Err(Error::Eof) => return Ok(true),
            Err(err) => {
                return Err(err);
            }
        }
        Ok(false)
    }

    pub fn parse(&mut self) -> Result<RawObject> {
        self.drop_whitespace()?;
        let raw_obj = match self.reader.peek() {
            Ok(ch) => {
                if ch == '{' {
                    self.parse_object()?
                } else {
                    self.parse_root_object()?
                }
            }
            Err(Error::Eof) => {
                return Ok(RawObject::default());
            }
            Err(err) => {
                return Err(err);
            }
        };
        self.drop_whitespace()?;
        match self.reader.peek() {
            Ok(ch) => {
                return Err(Error::unexpected_token("end of file", ch));
            }
            Err(Error::Eof) => {}
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

    use crate::Result;
    use crate::parser::parser::HoconParser;
    use crate::parser::read::StreamRead;
    use rstest::rstest;

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
    #[case("F:/IdeaProjects/akka/akka-actor/src/main/resources/reference.conf")]
    // #[case("resources/max_depth.conf")]
    fn test_parse(#[case] path: impl AsRef<std::path::Path>) -> Result<()> {
        use crate::parser::read::MIN_BUFFER_SIZE;
        let file = std::fs::File::open(&path)?;
        let read: StreamRead<_, MIN_BUFFER_SIZE> = StreamRead::new(BufReader::new(file));
        let mut parser = HoconParser::new(read);
        let raw = parser.parse()?;
        tracing::debug!("{}", raw);
        Ok(())
    }
}
