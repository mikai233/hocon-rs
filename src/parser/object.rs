use crate::Result;
use crate::error::Error;
use crate::parser::include::INCLUDE;
use crate::parser::is_hocon_horizontal_whitespace;
use crate::parser::read::Read;
use crate::parser::string::TRIPLE_DOUBLE_QUOTE;
use crate::parser::{Context, HoconParser};
use crate::raw::{
    comment::Comment, field::ObjectField, raw_object::RawObject, raw_string::RawString,
    raw_value::RawValue,
};
use std::str::FromStr;

#[macro_export]
macro_rules! try_peek {
    ($reader:expr) => {
        match $reader.peek() {
            Ok(ch) => ch,
            Err($crate::error::Error::Eof) => break,
            Err(err) => return Err(err),
        }
    };
}

impl<R: Read> HoconParser<R> {
    pub(crate) fn parse_key(&mut self) -> Result<RawString> {
        self.drop_horizontal_whitespace()?;
        self.parse_path_expression()
    }

    pub(crate) fn parse_value(&mut self) -> Result<RawValue> {
        self.drop_whitespace()?;
        let mut values = vec![];
        let mut scratch = vec![];
        let mut spaces = vec![];
        let mut prev_space = None;
        fn push_value_and_space(
            values: &mut Vec<RawValue>,
            spaces: &mut Vec<Option<String>>,
            mut space_after_value: Option<String>,
            v: RawValue,
        ) -> Option<String> {
            if !values.is_empty() {
                spaces.push(space_after_value);
                space_after_value = None;
            }
            values.push(v);
            space_after_value
        }
        loop {
            let ch = try_peek!(self.reader);
            match ch {
                '[' => {
                    // Parse array
                    let array = self.parse_array()?;
                    let v = RawValue::Array(array);
                    prev_space = push_value_and_space(&mut values, &mut spaces, prev_space, v);
                }
                '{' => {
                    // Parse object
                    let current_depth = Context::increase_depth();
                    if current_depth > self.options.max_object_depth {
                        Context::reset();
                        return Err(Error::RecursionDepthExceeded {
                            max_depth: self.options.max_object_depth,
                        });
                    }
                    let result = self.parse_object();
                    if result.is_err() {
                        Context::reset();
                    }
                    let object = result?;
                    Context::decrease_depth();
                    let v = RawValue::Object(object);
                    prev_space = push_value_and_space(&mut values, &mut spaces, prev_space, v);
                }
                '"' => {
                    // Parse quoted string or multi-line string
                    let v = if let Ok(chars) = self.reader.peek_n::<3>()
                        && chars == TRIPLE_DOUBLE_QUOTE
                    {
                        let multiline = self.parse_multiline_string()?;
                        RawValue::String(RawString::MultilineString(multiline))
                    } else {
                        let quoted = self.parse_quoted_string()?;
                        RawValue::String(RawString::QuotedString(quoted))
                    };
                    prev_space = push_value_and_space(&mut values, &mut spaces, prev_space, v);
                }
                '$' => {
                    let substitution = self.parse_substitution()?;
                    let v = RawValue::Substitution(substitution);
                    prev_space = push_value_and_space(&mut values, &mut spaces, prev_space, v);
                }
                ']' | '}' => {
                    break;
                }
                ',' | '#' | '\n' => {
                    if values.is_empty() {
                        return Err(Error::UnexpectedToken {
                            expected: "a valid value",
                            found_beginning: ch,
                        });
                    }
                    break;
                }
                '/' if self.reader.peek2().is_ok_and(|(_, ch2)| ch2 == '/') => {
                    if !values.is_empty() {
                        break;
                    } else {
                        return Err(Error::UnexpectedToken {
                            expected: "a valid value",
                            found_beginning: ch,
                        });
                    }
                }
                '\r' => {
                    if let Ok((_, ch2)) = self.reader.peek2() {
                        if ch2 == '\n' && !values.is_empty() {
                            break;
                        } else {
                            return Err(Error::UnexpectedToken {
                                expected: "a valid value",
                                found_beginning: ch,
                            });
                        }
                    }
                }
                ch => {
                    // Parse unquoted string or space
                    if is_hocon_horizontal_whitespace(ch) {
                        scratch.clear();
                        self.parse_horizontal_whitespace(&mut scratch)?;
                        let space = unsafe { str::from_utf8_unchecked(&scratch) };
                        if space.is_empty() {
                            prev_space = None
                        } else {
                            prev_space = Some(space.to_string());
                        }
                    } else {
                        let unquoted = self.parse_unquoted_string()?;
                        let v = RawValue::String(RawString::UnquotedString(unquoted));
                        prev_space = push_value_and_space(&mut values, &mut spaces, prev_space, v);
                    }
                }
            };
        }
        debug_assert!(!values.is_empty());
        if values.len() == 1 {
            let v = values.remove(0);
            let v = if let RawValue::String(s) = v {
                Self::resolve_unquoted_string(s)
            } else {
                v
            };
            Ok(v)
        } else {
            debug_assert_eq!(values.len(), spaces.len() + 1);
            RawValue::concat(values, spaces)
        }
    }

    pub(crate) fn parse_key_value(&mut self) -> Result<(RawString, RawValue)> {
        self.drop_whitespace()?;
        let key = self.parse_key()?;
        self.drop_whitespace()?;
        let is_add_assign = self.drop_kv_separator()?;
        self.drop_whitespace()?;
        let mut value = self.parse_value()?;
        if is_add_assign {
            value = RawValue::add_assign(value)
        }
        Ok((key, value))
    }

    pub fn drop_kv_separator(&mut self) -> Result<bool> {
        let ch = self.reader.peek()?;
        match ch {
            ':' | '=' => {
                self.reader.next()?;
            }
            '+' => {
                let (_, ch2) = self.reader.peek2()?;
                if ch2 != '=' {
                    return Err(Error::UnexpectedToken {
                        expected: "=",
                        found_beginning: ch2,
                    });
                }
                self.reader.next()?;
                self.reader.next()?;
                return Ok(true);
            }
            '{' => {}
            ch => {
                return Err(Error::UnexpectedToken {
                    expected: ": or =",
                    found_beginning: ch,
                });
            }
        }
        Ok(false)
    }

    pub(crate) fn parse_object_field(&mut self) -> Result<ObjectField> {
        let ch = self.reader.peek()?;
        // It maybe an include syntax, we need to peek more chars to determine.
        let field = if ch == 'i' && self.reader.peek_n::<7>()? == INCLUDE {
            let mut inclusion = self.parse_include()?;
            self.parse_inclusion(&mut inclusion)?;
            ObjectField::inclusion(inclusion)
        } else {
            let (key, value) = self.parse_key_value()?;
            ObjectField::key_value(key, value)
        };
        Ok(field)
    }

    pub(crate) fn parse_object_fields(&mut self) -> Result<Vec<ObjectField>> {
        let mut fields = vec![];
        loop {
            self.drop_whitespace_and_comments()?;
            let ch = self.reader.peek()?;
            if ch == '}' {
                break;
            }
            match self.parse_object_field() {
                Ok(field) => {
                    fields.push(field);
                }
                Err(Error::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
            self.drop_whitespace_and_comments()?;
            if self.drop_comma_separator()? {
                break;
            }
        }
        Ok(fields)
    }

    pub(crate) fn parse_root_object(&mut self) -> Result<RawObject> {
        let fields = self.parse_object_fields()?;
        let raw_obj = RawObject::new(fields);
        Ok(raw_obj)
    }

    pub(crate) fn parse_object(&mut self) -> Result<RawObject> {
        let ch = self.reader.peek()?;
        if ch != '{' {
            return Err(Error::UnexpectedToken {
                expected: "{",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let fields = self.parse_object_fields()?;
        let ch = self.reader.peek()?;
        if ch != '}' {
            return Err(Error::UnexpectedToken {
                expected: "}",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let raw_obj = RawObject::new(fields);
        Ok(raw_obj)
    }

    pub(crate) fn resolve_unquoted_string(string: RawString) -> RawValue {
        if let RawString::UnquotedString(unquoted) = string {
            match &*unquoted {
                "true" => RawValue::Boolean(true),
                "false" => RawValue::Boolean(false),
                "null" => RawValue::Null,
                other => match serde_json::Number::from_str(other) {
                    Ok(number) => RawValue::Number(number),
                    Err(_) => RawValue::unquoted_string(unquoted),
                },
            }
        } else {
            RawValue::String(string)
        }
    }

    #[allow(unused)]
    pub(crate) fn parse_newline_comments(&mut self) -> Result<Vec<ObjectField>> {
        let mut fields = vec![];
        loop {
            match self.parse_comment() {
                Ok((ty, content)) => {
                    let comment = Comment::new(content, ty);
                    fields.push(ObjectField::newline_comment(comment));
                }
                Err(Error::Eof | Error::UnexpectedToken { .. }) => {
                    break Ok(fields);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
}
