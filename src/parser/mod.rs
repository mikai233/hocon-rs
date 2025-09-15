mod array;
mod comment;
mod include;
pub(crate) mod loader;
mod object;
pub mod read;
mod string;
mod substitution;

use std::rc::Rc;

use derive_more::Constructor;

use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::read::Read;
use crate::raw::raw_object::RawObject;

#[derive(Constructor, Default, Debug, Clone)]
pub(crate) struct Context {
    pub(crate) include_chain: Vec<Rc<String>>,
    pub(crate) depth: usize,
}

impl Context {
    pub(crate) fn increase_depth(&mut self) -> usize {
        self.depth += 1;
        self.depth
    }

    pub(crate) fn decrease_depth(&mut self) -> usize {
        self.depth -= 1;
        self.depth
    }
}

#[derive(Debug)]
pub struct HoconParser<R> {
    pub(crate) reader: R,
    pub(crate) scratch: Vec<u8>,
    pub(crate) options: ConfigOptions,
    pub(crate) ctx: Context,
}

impl<'de, R: Read<'de>> HoconParser<R> {
    pub fn new(reader: R) -> Self {
        HoconParser {
            reader,
            scratch: vec![],
            options: Default::default(),
            ctx: Default::default(),
        }
    }

    pub fn with_options(reader: R, options: ConfigOptions) -> Self {
        HoconParser {
            reader,
            scratch: vec![],
            options,
            ctx: Default::default(),
        }
    }

    pub(crate) fn with_options_and_ctx(reader: R, options: ConfigOptions, ctx: Context) -> Self {
        HoconParser {
            reader,
            scratch: vec![],
            options,
            ctx,
        }
    }

    pub(crate) fn parse_horizontal_whitespace(&mut self, scratch: &mut Vec<u8>) -> Result<()> {
        loop {
            match self.reader.peek_horizontal_whitespace() {
                Ok(Some(n)) => {
                    for _ in 0..n {
                        let byte = self.reader.next()?;
                        scratch.push(byte);
                    }
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_horizontal_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek_horizontal_whitespace() {
                Ok(Some(n)) => {
                    for _ in 0..n {
                        self.reader.next()?;
                    }
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek_whitespace() {
                Ok(Some(n)) => {
                    for _ in 0..n {
                        self.reader.next()?;
                    }
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_comma_separator(&mut self) -> Result<bool> {
        match self.reader.peek() {
            Ok(ch) => {
                if ch == b',' {
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
        self.drop_whitespace_and_comments()?;
        let raw_obj = match self.reader.peek() {
            Ok(ch) => {
                if ch == b'{' {
                    self.parse_object()?
                } else {
                    self.parse_braces_omitted_object()?
                }
            }
            Err(Error::Eof) => {
                return Ok(RawObject::default());
            }
            Err(err) => {
                return Err(err);
            }
        };
        self.drop_whitespace_and_comments()?;
        match self.reader.peek() {
            Ok(ch) => {
                return Err(Error::UnexpectedToken {
                    expected: "end of file",
                    found_beginning: ch,
                });
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
    use crate::config_options::ConfigOptions;
    use crate::parser::HoconParser;
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
    #[cfg_attr(feature = "urls_includes", case("resources/included.conf"))]
    #[cfg_attr(feature = "urls_includes", case("resources/main.conf"))]
    fn test_parse(#[case] path: impl AsRef<std::path::Path>) -> Result<()> {
        let file = std::fs::File::open(&path)?;
        let read = StreamRead::new(BufReader::new(file));
        let options = ConfigOptions::new(false, vec!["resources".to_string()]);
        let mut parser = HoconParser::with_options(read, options);
        parser.parse()?;
        Ok(())
    }
}
