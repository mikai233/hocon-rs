mod array;
mod comment;
mod frame;
mod include;
pub(crate) mod loader;
mod object;
pub mod read;
mod string;
mod substitution;

use std::rc::Rc;

use derive_more::Constructor;

const DEFAULT_ARRAY_CAPACITY: usize = 16;
const DEFAULT_OBJECT_CAPACITY: usize = 16;

use crate::config_options::ConfigOptions;
use crate::error::{Error, Parse};
use crate::parser::frame::{Entry, Frame, Separator, Value};
use crate::parser::include::INCLUDE;
use crate::parser::read::Read;
use crate::parser::string::TRIPLE_DOUBLE_QUOTE;
use crate::raw::add_assign::AddAssign;
use crate::raw::field::ObjectField;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::{Result, try_peek};

#[derive(Constructor, Default, Debug, Clone)]
pub(crate) struct Context {
    pub(crate) include_chain: Vec<Rc<String>>,
}

#[derive(Debug)]
pub struct HoconParser<R> {
    pub(crate) reader: R,
    pub(crate) scratch: Vec<u8>,
    pub(crate) options: ConfigOptions,
    pub(crate) ctx: Context,
    pub(crate) stack: Vec<Frame>,
}

impl<'de, R: Read<'de>> HoconParser<R> {
    pub fn new(reader: R) -> Self {
        HoconParser {
            reader,
            scratch: Default::default(),
            options: Default::default(),
            ctx: Default::default(),
            stack: Default::default(),
        }
    }

    pub fn with_options(reader: R, options: ConfigOptions) -> Self {
        HoconParser {
            reader,
            scratch: Default::default(),
            options,
            ctx: Default::default(),
            stack: Default::default(),
        }
    }

    pub(crate) fn with_options_and_ctx(reader: R, options: ConfigOptions, ctx: Context) -> Self {
        HoconParser {
            reader,
            scratch: Default::default(),
            options,
            ctx,
            stack: Default::default(),
        }
    }

    pub(crate) fn parse_horizontal_whitespace(reader: &mut R, scratch: &mut Vec<u8>) -> Result<()> {
        loop {
            match reader.peek_horizontal_whitespace() {
                Ok(Some(n)) => {
                    for _ in 0..n {
                        let byte = reader.next()?;
                        scratch.push(byte);
                    }
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_horizontal_whitespace(reader: &mut R) -> Result<()> {
        loop {
            match reader.peek_horizontal_whitespace() {
                Ok(Some(n)) => {
                    reader.discard(n)?;
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace(reader: &mut R) -> Result<()> {
        loop {
            match reader.peek_whitespace() {
                Ok(Some(n)) => {
                    reader.discard(n)?;
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub fn parse(&mut self) -> Result<RawObject> {
        Self::drop_whitespace_and_comments(&mut self.reader)?;
        match self.reader.peek() {
            Ok(ch) => {
                if ch != b'{' {
                    let root = Frame::Object {
                        entries: Default::default(),
                        next_entry: Default::default(),
                    };
                    self.stack.push(root);
                }
                self.parse_iteration()?;
            }
            Err(Error::Eof) => {
                return Ok(RawObject::default());
            }
            Err(err) => {
                return Err(err);
            }
        };
        Self::drop_whitespace_and_comments(&mut self.reader)?;
        match self.reader.peek() {
            Ok(_) => {
                return Err(self.reader.peek_error(Parse::Expected("end of file")));
            }
            Err(Error::Eof) => {}
            Err(err) => {
                return Err(err);
            }
        }
        debug_assert!(self.stack.len() == 1);
        let frame = self.stack.pop().unwrap();
        let raw_obj = match frame {
            Frame::Object { entries, .. } => RawObject::new(entries),
            _ => unreachable!("Unexpected frame type, expect Object"),
        };
        Ok(raw_obj)
    }
    fn resolve_value(
        Value {
            mut values, spaces, ..
        }: Value,
    ) -> Result<RawValue> {
        let value = if values.len() == 1 {
            let v = values.remove(0);
            if let RawValue::String(s) = v {
                Self::resolve_unquoted_string(s)
            } else {
                v
            }
        } else {
            debug_assert_eq!(values.len(), spaces.len() + 1);
            RawValue::concat(values, spaces)?
        };
        Ok(value)
    }

    pub(crate) fn end_value(&mut self) -> Result<()> {
        match Self::last_frame(&mut self.stack) {
            Frame::Object {
                entries,
                next_entry,
            } => {
                if let Some(Entry {
                    key,
                    separator,
                    value,
                }) = next_entry.take()
                {
                    let key = match key {
                        Some(key) => key,
                        None => {
                            return Err(self.reader.error(Parse::Expected("key")));
                        }
                    };
                    let separator = match separator {
                        Some(separator) => separator,
                        None => {
                            return Err(self.reader.error(Parse::Expected("= or : or +=")));
                        }
                    };
                    let value = match value {
                        Some(value) if !value.values.is_empty() => value,
                        _ => {
                            return Err(self.reader.error(Parse::Expected("value")));
                        }
                    };
                    let mut value = Self::resolve_value(value)?;
                    if separator == Separator::AddAssign {
                        value = RawValue::AddAssign(AddAssign::new(value.into()));
                    }
                    let field = ObjectField::key_value(key, value);
                    entries.push(field);
                }
            }
            Frame::Array {
                elements,
                next_element,
            } => {
                if let Some(element) = next_element.take() {
                    let value = Self::resolve_value(element)?;
                    elements.push(value);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn start_array(stack: &mut Vec<Frame>) {
        let array = Frame::Array {
            elements: Vec::with_capacity(DEFAULT_ARRAY_CAPACITY),
            next_element: Default::default(),
        };
        stack.push(array);
    }

    pub(crate) fn end_array(&mut self) -> Result<()> {
        self.end_value()?;
        if self.stack.len() == 1 {
            return Ok(());
        }
        match self.stack.pop().expect("frame is empty") {
            Frame::Array { elements, .. } => {
                let array = RawValue::Array(RawArray::new(elements));
                let parent = Self::last_frame(&mut self.stack);
                Self::append_to_frame(parent, array)?;
            }
            other => panic!("Unexpected frame type: {}", other.ty()),
        }
        Ok(())
    }

    pub(crate) fn start_object(stack: &mut Vec<Frame>) {
        let object = Frame::Object {
            entries: Vec::with_capacity(DEFAULT_OBJECT_CAPACITY),
            next_entry: Default::default(),
        };
        stack.push(object);
    }

    pub(crate) fn end_object(&mut self) -> Result<()> {
        self.end_value()?;
        if self.stack.len() == 1 {
            return Ok(());
        }
        match self.stack.pop().expect("stack is empty") {
            Frame::Object { entries, .. } => {
                let object = RawValue::Object(RawObject(entries));
                let parent = Self::last_frame(&mut self.stack);
                Self::append_to_frame(parent, object)?;
            }
            _ => panic!("unexpected frame type"),
        }
        Ok(())
    }

    pub(crate) fn append_to_frame(frame: &mut Frame, raw: RawValue) -> Result<()> {
        match frame {
            Frame::Array { next_element, .. } => {
                let element = next_element.get_or_insert_default();
                element.push_value(raw);
            }
            Frame::Object { next_entry, .. } => {
                assert!(next_entry.is_some());
                let Entry {
                    key,
                    separator,
                    value,
                } = next_entry.as_mut().unwrap();
                assert!(key.is_some());
                assert!(separator.is_some());
                let value = value.get_or_insert_default();
                value.push_value(raw);
            }
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn last_frame(stack: &mut [Frame]) -> &mut Frame {
        stack.last_mut().expect("stack is empty")
    }

    pub(crate) fn push_value(frame: &mut Frame, raw_value: RawValue) -> Result<()> {
        match frame {
            Frame::Array { next_element, .. } => {
                let element = next_element.get_or_insert_default();
                element.push_value(raw_value);
            }
            Frame::Object { next_entry, .. } => {
                let entry = next_entry.as_mut().unwrap();
                let value = entry.value.get_or_insert_default();
                value.push_value(raw_value);
            }
        }
        Ok(())
    }

    pub(crate) fn parse_iteration(&mut self) -> Result<()> {
        Self::drop_horizontal_whitespace(&mut self.reader)?;
        loop {
            self.check_depth_limit()?;
            let ch = try_peek!(self.reader);
            match ch {
                b':' | b'=' => {
                    match Self::last_frame(&mut self.stack) {
                        Frame::Object { next_entry, .. } => {
                            let entry = match next_entry {
                                Some(entry) => entry,
                                None => {
                                    return Err(self.reader.error(Parse::Expected("key")));
                                }
                            };
                            if entry.separator.is_some() {
                                return Err(self.reader.error(Parse::Expected("value")));
                            }
                            entry.separator = Some(Separator::Assign);
                        }
                        other => unreachable!("unexpected frame: {}", other.ty()),
                    }
                    self.reader.discard(1)?;
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                }
                b'+' => {
                    match self.reader.peek2() {
                        Ok((_, b'=')) => {}
                        _ => {
                            return Err(self.reader.peek_error(Parse::Expected("+=")));
                        }
                    }
                    self.reader.discard(2)?;
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                    match self.stack.last_mut().unwrap() {
                        Frame::Object { next_entry, .. } => {
                            let entry = next_entry.as_mut().unwrap();
                            entry.separator = Some(Separator::AddAssign);
                        }
                        _ => panic!("Unexpected frame type"),
                    }
                }
                b'[' => {
                    // Parse array
                    self.reader.discard(1)?;
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                    Self::start_array(&mut self.stack);
                }
                b'{' => {
                    // Parse object
                    self.reader.discard(1)?;
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                    // If the key value is like `a {}`, the separator will not be set, it's need to be set here manually.
                    if let Some(frame) = self.stack.last_mut()
                        && let Frame::Object { next_entry, .. } = frame
                        && let Some(entry) = next_entry
                        && entry.separator.is_none()
                    {
                        entry.separator = Some(Separator::Assign);
                    }
                    Self::start_object(&mut self.stack);
                }
                b'"' if Self::last_frame(&mut self.stack).expect_value() => {
                    // Parse quoted string or multi-line string
                    self.scratch.clear();
                    let v = if let Ok(chars) = self.reader.peek_n(3)
                        && chars == TRIPLE_DOUBLE_QUOTE
                    {
                        let multiline = Self::parse_multiline_string(
                            &mut self.reader,
                            &mut self.scratch,
                            false,
                        )?;
                        RawValue::String(RawString::MultilineString(multiline))
                    } else {
                        let quoted =
                            Self::parse_quoted_string(&mut self.reader, &mut self.scratch, false)?;
                        RawValue::String(RawString::QuotedString(quoted))
                    };
                    Self::push_value(self.stack.last_mut().unwrap(), v)?;
                }
                b'$' if Self::last_frame(&mut self.stack).expect_value() => {
                    let substitution = self.parse_substitution()?;
                    let v = RawValue::Substitution(substitution);
                    Self::push_value(self.stack.last_mut().unwrap(), v)?;
                }
                b']' => {
                    self.reader.discard(1)?;
                    self.end_array()?;
                }
                b'}' => {
                    self.reader.discard(1)?;
                    self.end_object()?;
                }
                b',' | b'#' | b'\n' => {
                    self.end_value()?;
                    if ch == b',' {
                        self.reader.discard(1)?;
                    }
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                }
                b'/' if self.reader.peek2().is_ok_and(|(_, ch2)| ch2 == b'/') => {
                    // TODO parse comment
                    self.end_value()?;
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                }
                b'\r' => {
                    if let Ok((_, ch2)) = self.reader.peek2()
                        && ch2 == b'\n'
                    {
                        self.end_value()?;
                    }
                    Self::drop_whitespace_and_comments(&mut self.reader)?;
                }
                b'i' if self.reader.peek_n(7).is_ok_and(|chars| chars == INCLUDE) => {
                    let mut inclusion = self.parse_include()?;
                    self.parse_inclusion(&mut inclusion)?;
                    match self.stack.last_mut().unwrap() {
                        Frame::Object {
                            entries,
                            next_entry,
                        } => {
                            assert!(next_entry.is_none());
                            let field = ObjectField::inclusion(inclusion);
                            entries.push(field);
                        }
                        _ => panic!("unexpected frame type"),
                    }
                }
                _ => match Self::last_frame(&mut self.stack) {
                    Frame::Object { next_entry, .. } => match next_entry {
                        Some(entry) => {
                            let entry_value = entry.value.get_or_insert_default();
                            self.scratch.clear();
                            if self.reader.starts_with_horizontal_whitespace()? {
                                Self::parse_horizontal_whitespace(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let space = unsafe { str::from_utf8_unchecked(&self.scratch) };
                                if space.is_empty() {
                                    entry_value.pre_space = None;
                                } else {
                                    entry_value.pre_space = Some(space.to_string());
                                }
                            } else {
                                let unquoted = Self::parse_unquoted_string(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let raw_value =
                                    RawValue::String(RawString::UnquotedString(unquoted));
                                entry_value.push_value(raw_value);
                            }
                        }
                        None => {
                            Self::drop_whitespace_and_comments(&mut self.reader)?;
                            let ch = self.reader.peek()?;
                            if ch != b'}' {
                                let key = Self::parse_key(&mut self.reader, &mut self.scratch)?;
                                *next_entry = Some(Entry {
                                    key: Some(key),
                                    separator: None,
                                    value: None,
                                });
                            }
                        }
                    },
                    Frame::Array { next_element, .. } => {
                        self.scratch.clear();
                        let element = next_element.get_or_insert_default();
                        if self.reader.starts_with_horizontal_whitespace()? {
                            Self::parse_horizontal_whitespace(&mut self.reader, &mut self.scratch)?;
                            let space = unsafe { str::from_utf8_unchecked(&self.scratch) };
                            if space.is_empty() {
                                element.pre_space = None;
                            } else {
                                element.pre_space = Some(space.to_string());
                            }
                        } else {
                            Self::drop_whitespace_and_comments(&mut self.reader)?;
                            let ch = self.reader.peek()?;
                            if ch != b']' {
                                let unquoted = Self::parse_unquoted_string(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let v = RawValue::String(RawString::UnquotedString(unquoted));
                                element.push_value(v);
                            }
                        }
                    }
                },
            };
        }
        self.end_value()?;
        Ok(())
    }

    fn check_depth_limit(&mut self) -> Result<()> {
        if self.stack.len() > self.options.max_include_depth {
            Err(Error::RecursionDepthExceeded {
                max_depth: self.options.max_include_depth,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use crate::Result;
    use crate::config_options::ConfigOptions;
    use crate::error::Error;
    use crate::parser::HoconParser;
    use crate::parser::read::StreamRead;
    use rstest::rstest;

    #[rstest]
    #[case("test_conf/base.conf")]
    #[case("test_conf/concat.conf")]
    #[case("test_conf/concat2.conf")]
    #[case("test_conf/concat3.conf")]
    #[case("test_conf/demo.conf")]
    #[case("test_conf/deserialize.conf")]
    #[case("test_conf/empty.conf")]
    #[cfg_attr(feature = "urls_includes", case("test_conf/included.conf"))]
    #[cfg_attr(feature = "urls_includes", case("test_conf/main.conf"))]
    fn test_parse(#[case] path: impl AsRef<std::path::Path>) -> Result<()> {
        let file = std::fs::File::open(&path)?;
        let read = StreamRead::new(BufReader::new(file));
        let options = ConfigOptions::new(false, vec!["test_conf".to_string()]);
        let mut parser = HoconParser::with_options(read, options);
        parser.parse()?;
        Ok(())
    }

    #[rstest]
    #[case("test_conf/error/missing_key.conf")]
    #[case("test_conf/error/missing_value.conf")]
    #[case("test_conf/error/error_separator.conf")]
    #[case("test_conf/error/error_separator2.conf")]
    #[case("test_conf/error/error_separator3.conf")]
    fn test_error_conf(#[case] path: impl AsRef<std::path::Path>) -> Result<()> {
        let file = std::fs::File::open(&path)?;
        let read = StreamRead::new(BufReader::new(file));
        let options = ConfigOptions::new(false, vec!["test_conf".to_string()]);
        let mut parser = HoconParser::with_options(read, options);
        match parser.parse() {
            Err(Error::Parse { .. }) => {}
            _ => panic!("should be a parse error"),
        }
        Ok(())
    }
}
