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

macro_rules! resolve_unquoted_string {
    ($s:expr, $scratch:expr) => {{
        use crate::ref_to_string;
        use std::ops::Deref;
        use std::str::FromStr;
        match $s.deref() {
            "true" => RawValue::Boolean(true),
            "false" => RawValue::Boolean(false),
            "null" => RawValue::Null,
            other => match other.as_bytes().first() {
                Some(b'-' | b'0'..=b'9') => match serde_json::Number::from_str(other) {
                    Ok(number) => RawValue::Number(number),
                    Err(_) => RawValue::unquoted_string(ref_to_string!($s, $scratch)),
                },
                Some(_) | None => RawValue::unquoted_string(ref_to_string!($s, $scratch)),
            },
        }
    }};
}

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
                Ok(n) if n > 0 => {
                    for _ in 0..n {
                        let byte = reader.next()?;
                        scratch.push(byte);
                    }
                }
                Ok(_) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_horizontal_whitespace(reader: &mut R) -> Result<()> {
        loop {
            match reader.peek_horizontal_whitespace() {
                Ok(n) if n > 0 => {
                    reader.discard(n);
                }
                Ok(_) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub(crate) fn drop_whitespace(reader: &mut R) -> Result<()> {
        loop {
            match reader.peek_whitespace() {
                Ok(n) if n > 0 => {
                    reader.discard(n);
                }
                Ok(_) | Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    pub fn parse(&mut self) -> Result<RawObject> {
        Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
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
        Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
        match self.reader.peek() {
            Ok(_) => {
                return Err(self.reader.error(Parse::Expected("end of file")));
            }
            Err(Error::Eof) => {}
            Err(err) => {
                return Err(err);
            }
        }
        debug_assert!(self.stack.len() == 1);
        let frame = self.stack.pop().expect("stack is empty");
        let raw_obj = match frame {
            Frame::Object { entries, .. } => RawObject::new(entries),
            _ => unreachable!("unexpected frame type, expect Object"),
        };
        Ok(raw_obj)
    }

    fn resolve_value(
        Value {
            mut values, spaces, ..
        }: Value,
    ) -> Result<RawValue> {
        let value = if values.len() == 1 {
            values.remove(0)
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

    pub(crate) fn end_object(&mut self) -> Result<bool> {
        self.end_value()?;
        if self.stack.len() == 1 {
            return Ok(true);
        }
        match self.stack.pop().expect("stack is empty") {
            Frame::Object { entries, .. } => {
                let object = RawValue::Object(RawObject(entries));
                let parent = Self::last_frame(&mut self.stack);
                Self::append_to_frame(parent, object)?;
            }
            _ => panic!("unexpected frame type"),
        }
        Ok(false)
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
            match try_peek!(self.reader) {
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
                        Frame::Array { .. } => {
                            return Err(self.reader.error(Parse::Expected(",")));
                        }
                    }
                    self.reader.discard(1);
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                }
                b'+' => {
                    match self.reader.peek_n(2) {
                        Ok(b"+=") => {}
                        _ => {
                            return Err(self.reader.peek_error(Parse::Expected("+=")));
                        }
                    }
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
                            entry.separator = Some(Separator::AddAssign);
                        }
                        Frame::Array { .. } => {
                            return Err(self.reader.error(Parse::Expected(",")));
                        }
                    }
                    self.reader.discard(2);
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                }
                b'[' => {
                    // If the key value is like `a []`, the separator will not be set, it's need to be set here manually.
                    // The stack cannot be empty since root configuration cannot start with `[]`
                    if let Frame::Object { next_entry, .. } = Self::last_frame(&mut self.stack) {
                        match next_entry {
                            Some(entry) if entry.key.is_none() => {
                                return Err(self.reader.error(Parse::Expected("key")));
                            }
                            Some(entry) if entry.separator.is_none() => {
                                entry.separator = Some(Separator::Assign);
                            }
                            Some(_) => {}
                            None => {
                                return Err(self.reader.error(Parse::Expected("key")));
                            }
                        }
                    }
                    // Parse array
                    self.reader.discard(1);
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                    Self::start_array(&mut self.stack);
                }
                b'{' => {
                    // Parse object
                    self.reader.discard(1);
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                    // If the key value is like `a {}`, the separator will not be set, it's need to be set here manually.
                    // The stack maybe empty when the configuration starts with `{}`
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
                    Self::push_value(Self::last_frame(&mut self.stack), v)?;
                }
                b'$' if Self::last_frame(&mut self.stack).expect_value() => {
                    let substitution = self.parse_substitution()?;
                    let v = RawValue::Substitution(substitution);
                    Self::push_value(Self::last_frame(&mut self.stack), v)?;
                }
                b']' => {
                    self.end_array()?;
                    self.reader.discard(1);
                }
                b'}' => {
                    let end_root = self.end_object()?;
                    self.reader.discard(1);
                    if end_root {
                        break;
                    }
                }
                byte @ b',' | byte @ b'#' | byte @ b'\n' => {
                    self.end_value()?;
                    if byte == b',' {
                        self.reader.discard(1);
                    }
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                }
                b'/' if self.reader.peek_n(2).is_ok_and(|bytes| bytes == b"//") => {
                    self.end_value()?;
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
                }
                b'\r' if self.reader.peek_n(2).is_ok_and(|bytes| bytes == b"\r\n") => {
                    self.end_value()?;
                    Self::drop_whitespace_and_comments(&mut self.reader, &mut self.scratch)?;
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
                                let s = Self::parse_unquoted_string(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let raw_value = resolve_unquoted_string!(s, &mut self.scratch);
                                entry_value.push_value(raw_value);
                            }
                        }
                        None => {
                            Self::drop_whitespace_and_comments(
                                &mut self.reader,
                                &mut self.scratch,
                            )?;
                            let byte = self.reader.peek()?;
                            if byte != b'}' {
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
                            Self::drop_whitespace_and_comments(
                                &mut self.reader,
                                &mut self.scratch,
                            )?;
                            let byte = self.reader.peek()?;
                            if byte != b']' {
                                let s = Self::parse_unquoted_string(
                                    &mut self.reader,
                                    &mut self.scratch,
                                )?;
                                let v = resolve_unquoted_string!(s, &mut self.scratch);
                                element.push_value(v);
                            }
                        }
                    }
                },
            };
        }
        self.end_value()?;
        if self.stack.len() > 1 {
            return match Self::last_frame(&mut self.stack) {
                Frame::Object { .. } => Err(self.reader.error(Parse::Expected("}"))),
                Frame::Array { .. } => Err(self.reader.error(Parse::Expected("]"))),
            };
        }
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

    use crate::config_options::ConfigOptions;
    use crate::error::Error;
    use crate::parser::HoconParser;
    use crate::parser::read::{Position, StreamRead};
    use crate::{Config, Result, Value};
    use rstest::rstest;

    #[rstest]
    #[case("test_conf/comprehensive/base.conf")]
    #[case("test_conf/comprehensive/concat.conf")]
    #[case("test_conf/comprehensive/concat2.conf")]
    #[case("test_conf/comprehensive/concat3.conf")]
    #[case("test_conf/comprehensive/demo.conf")]
    #[case("test_conf/comprehensive/deserialize.conf")]
    #[case("test_conf/comprehensive/empty.conf")]
    #[cfg_attr(
        feature = "urls_includes",
        case("test_conf/comprehensive/included.conf")
    )]
    #[cfg_attr(feature = "urls_includes", case("test_conf/comprehensive/main.conf"))]
    fn test_parse(#[case] path: impl AsRef<std::path::Path>) -> Result<()> {
        let file = std::fs::File::open(&path)?;
        let read = StreamRead::new(BufReader::new(file));
        let options = ConfigOptions::new(false, vec!["test_conf".to_string()]);
        let mut parser = HoconParser::with_options(read, options);
        parser.parse()?;
        Ok(())
    }

    #[rstest]
    #[case("test_conf/error/missing_key.conf", Position::new(1, 2))]
    #[case("test_conf/error/missing_value.conf", Position::new(1, 8))]
    #[case("test_conf/error/missing_value2.conf", Position::new(1, 9))]
    #[case("test_conf/error/missing_value3.conf", Position::new(1, 11))]
    #[case("test_conf/error/invalid_separator.conf", Position::new(1, 9))]
    #[case("test_conf/error/invalid_separator2.conf", Position::new(1, 8))]
    #[case("test_conf/error/invalid_separator3.conf", Position::new(1, 8))]
    #[case("test_conf/error/missing_square_brackets.conf", Position::new(1, 10))]
    #[case("test_conf/error/missing_square_brackets2.conf", Position::new(2, 8))]
    #[case("test_conf/error/missing_curly_braces.conf", Position::new(1, 10))]
    #[case("test_conf/error/missing_curly_braces2.conf", Position::new(3, 14))]
    #[case("test_conf/error/invalid_substitution.conf", Position::new(1, 7))]
    #[case("test_conf/error/invalid_substitution2.conf", Position::new(1, 7))]
    #[case("test_conf/error/invalid_substitution3.conf", Position::new(1, 7))]
    #[case("test_conf/error/invalid_substitution4.conf", Position::new(1, 8))]
    #[case("test_conf/error/invalid_substitution5.conf", Position::new(1, 12))]
    #[case("test_conf/error/invalid_array_value.conf", Position::new(1, 12))]
    #[case("test_conf/error/invalid_array_value2.conf", Position::new(1, 12))]
    #[case("test_conf/error/invalid_array_value3.conf", Position::new(1, 12))]
    #[case("test_conf/error/invalid_object_entry.conf", Position::new(1, 9))]
    #[case("test_conf/error/invalid_object_entry2.conf", Position::new(1, 7))]
    #[case("test_conf/error/invalid_root.conf", Position::new(1, 1))]
    #[case("test_conf/error/invalid_root2.conf", Position::new(1, 4))]
    #[case("test_conf/error/invalid_root3.conf", Position::new(1, 2))]
    #[case("test_conf/error/invalid_add_assign.conf", Position::new(1, 1))]
    #[case("test_conf/error/invalid_add_assign2.conf", Position::new(1, 7))]
    fn test_error_conf(
        #[case] path: impl AsRef<std::path::Path>,
        #[case] expected_position: Position,
    ) -> Result<()> {
        let file = std::fs::File::open(&path)?;
        let read = StreamRead::new(BufReader::new(file));
        let options = ConfigOptions::new(
            false,
            vec!["test_conf".to_string(), "test_conf/error".to_string()],
        );
        let mut parser = HoconParser::with_options(read, options);
        match parser.parse() {
            Err(Error::Parse { position, .. }) => {
                assert_eq!(position, expected_position);
            }
            _ => panic!("should be a parse error"),
        }
        Ok(())
    }

    #[rstest]
    #[case("test_conf/error/invalid_concatenation.conf")]
    #[case("test_conf/error/invalid_concatenation2.conf")]
    #[case("test_conf/error/invalid_concatenation3.conf")]
    #[case("test_conf/error/invalid_concatenation4.conf")]
    #[case("test_conf/error/invalid_concatenation5.conf")]
    #[case("test_conf/error/invalid_concatenation6.conf")]
    #[case("test_conf/error/invalid_concatenation7.conf")]
    #[case("test_conf/error/invalid_concatenation8.conf")]
    #[case("test_conf/error/invalid_concatenation9.conf")]
    #[case("test_conf/error/invalid_concatenation10.conf")]
    fn test_error_conf2(#[case] path: impl AsRef<std::path::Path>) {
        let options = ConfigOptions::new(
            false,
            vec!["test_conf".to_string(), "test_conf/error".to_string()],
        );
        let result: Result<Value> = Config::parse_file(path, Some(options));
        assert!(result.is_err());
    }
}
