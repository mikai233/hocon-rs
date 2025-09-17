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
use crate::error::Error;
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
use crate::{try_peek, Result};

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

    pub(crate) fn parse_horizontal_whitespace2(
        reader: &mut R,
        scratch: &mut Vec<u8>,
    ) -> Result<()> {
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

    pub(crate) fn drop_horizontal_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek_horizontal_whitespace() {
                Ok(Some(n)) => {
                    self.reader.discard(n)?;
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_horizontal_whitespace2(reader: &mut R) -> Result<()> {
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

    pub(crate) fn drop_whitespace(&mut self) -> Result<()> {
        loop {
            match self.reader.peek_whitespace() {
                Ok(Some(n)) => {
                    self.reader.discard(n)?;
                }
                Ok(None) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace2(reader: &mut R) -> Result<()> {
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

    pub(crate) fn drop_comma_separator(&mut self) -> Result<bool> {
        match self.reader.peek() {
            Ok(ch) => {
                if ch == b',' {
                    self.reader.discard(1)?;
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

    pub(crate) fn end_value(stack: &mut Vec<Frame>) -> Result<()> {
        match stack.last_mut().unwrap() {
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
                    assert!(key.is_some());
                    assert!(separator.is_some());
                    assert!(value.as_ref().is_some_and(|v| !v.values.is_empty()));
                    let key = key.unwrap();
                    let mut value = Self::resolve_value(value.unwrap())?;
                    if separator.unwrap() == Separator::AddAssign {
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

    pub(crate) fn end_array(stack: &mut Vec<Frame>) -> Result<()> {
        Self::end_value(stack)?;
        match stack.pop().unwrap() {
            Frame::Array { elements, .. } => {
                let array = RawValue::Array(RawArray::new(elements));
                Self::append(stack.last_mut().unwrap(), array)?;
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

    pub(crate) fn end_object(stack: &mut Vec<Frame>) -> Result<()> {
        Self::end_value(stack)?;
        if stack.len() == 1 {
            return Ok(());
        }
        match stack.pop() {
            Some(frame) => match frame {
                Frame::Object { entries, .. } => {
                    let object = RawValue::Object(RawObject(entries));
                    Self::append(stack.last_mut().unwrap(), object)?;
                }
                _ => panic!("Unexpected frame type"),
            },
            None => {}
        }
        Ok(())
    }

    pub(crate) fn append(frame: &mut Frame, raw: RawValue) -> Result<()> {
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
        Self::drop_horizontal_whitespace2(&mut self.reader)?;
        fn expect_value(stack: &Vec<Frame>) -> bool {
            match stack.last() {
                Some(frame) => match frame {
                    Frame::Object { next_entry, .. } => next_entry
                        .as_ref()
                        .is_some_and(|e| e.key.is_some() && e.separator.is_some()),
                    Frame::Array { .. } => true,
                },
                None => false,
            }
        }
        loop {
            self.check_depth_limit()?;
            let ch = try_peek!(self.reader);
            match ch {
                b':' | b'=' => {
                    self.reader.discard(1)?;
                    match self.stack.last_mut().unwrap() {
                        Frame::Object { next_entry, .. } => {
                            let entry = next_entry.as_mut().unwrap();
                            entry.separator = Some(Separator::Assign);
                        }
                        _ => panic!("Unexpected frame type"),
                    }
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
                }
                b'+' => {
                    let (_, ch2) = self.reader.peek2()?;
                    if ch2 != b'=' {
                        return Err(Error::UnexpectedToken {
                            expected: "=",
                            found_beginning: ch2,
                        });
                    }
                    self.reader.discard(2)?;
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
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
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
                    Self::start_array(&mut self.stack);
                }
                b'{' => {
                    // Parse object
                    self.reader.discard(1)?;
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
                    // If the key value is like `a {}`, the separator will not be set, it's need to be set here manually.
                    if let Some(frame) = self.stack.last_mut()
                        && let Frame::Object { next_entry, .. } = frame
                        && let Some(entry) = next_entry
                    {
                        if entry.separator.is_none() {
                            entry.separator = Some(Separator::Assign);
                        }
                    }
                    Self::start_object(&mut self.stack);
                }
                b'"' if expect_value(&self.stack) => {
                    // Parse quoted string or multi-line string
                    let v = if let Ok(chars) = self.reader.peek_n(3)
                        && chars == TRIPLE_DOUBLE_QUOTE
                    {
                        let multiline = self.parse_multiline_string(false)?;
                        RawValue::String(RawString::MultilineString(multiline))
                    } else {
                        let quoted = self.parse_quoted_string(false)?;
                        RawValue::String(RawString::QuotedString(quoted))
                    };
                    Self::push_value(self.stack.last_mut().unwrap(), v)?;
                }
                b'$' if expect_value(&self.stack) => {
                    let substitution = self.parse_substitution()?;
                    let v = RawValue::Substitution(substitution);
                    Self::push_value(self.stack.last_mut().unwrap(), v)?;
                }
                b']' => {
                    self.reader.discard(1)?;
                    Self::end_array(&mut self.stack)?;
                }
                b'}' => {
                    self.reader.discard(1)?;
                    Self::end_object(&mut self.stack)?;
                }
                b',' | b'#' | b'\n' => {
                    Self::end_value(&mut self.stack)?;
                    if ch == b',' {
                        self.reader.discard(1)?;
                    }
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
                }
                b'/' if self.reader.peek2().is_ok_and(|(_, ch2)| ch2 == b'/') => {
                    // TODO parse comment
                    Self::end_value(&mut self.stack)?;
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
                }
                b'\r' => {
                    if let Ok((_, ch2)) = self.reader.peek2()
                        && ch2 == b'\n'
                    {
                        Self::end_value(&mut self.stack)?;
                    }
                    Self::drop_whitespace_and_comments2(&mut self.reader)?;
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
                        _ => panic!("Unexpected frame type"),
                    }
                }
                _ => match self.stack.last_mut().unwrap() {
                    Frame::Object { next_entry, .. } => match next_entry {
                        Some(entry) => {
                            let entry_value = entry.value.get_or_insert_default();
                            self.scratch.clear();
                            if self.reader.starts_with_horizontal_whitespace()? {
                                Self::parse_horizontal_whitespace2(
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
                                let unquoted = Self::parse_unquoted_string2(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let v = RawValue::String(RawString::UnquotedString(unquoted));
                                entry_value.push_value(v);
                            }
                        }
                        None => {
                            Self::drop_whitespace_and_comments2(&mut self.reader)?;
                            let ch = self.reader.peek()?;
                            if ch != b'}' {
                                let key = Self::parse_key2(&mut self.reader, &mut self.scratch)?;
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
                            Self::parse_horizontal_whitespace2(
                                &mut self.reader,
                                &mut self.scratch,
                            )?;
                            let space = unsafe { str::from_utf8_unchecked(&self.scratch) };
                            if space.is_empty() {
                                element.pre_space = None;
                            } else {
                                element.pre_space = Some(space.to_string());
                            }
                        } else {
                            Self::drop_whitespace_and_comments2(&mut self.reader)?;
                            let ch = self.reader.peek()?;
                            if ch != b']' {
                                let unquoted = Self::parse_unquoted_string2(
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
        Self::end_value(&mut self.stack)?;
        Ok(())
    }

    fn check_depth_limit(&mut self) -> Result<()> {
        if self.stack.len() > self.options.max_include_depth {
            Err(Error::RecursionDepthExceeded { max_depth: self.options.max_include_depth })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use crate::config_options::ConfigOptions;
    use crate::parser::read::StreamRead;
    use crate::parser::HoconParser;
    use crate::Result;
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
