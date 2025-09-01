use std::str::FromStr;

use crate::{
    parser::pure::{
        include::INCLUDE,
        is_hocon_horizontal_whitespace,
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::{
        comment::Comment, field::ObjectField, raw_object::RawObject, raw_string::RawString,
        raw_value::RawValue,
    },
};

#[macro_export]
macro_rules! try_peek {
    ($reader:expr) => {
        match $reader.peek() {
            Ok(ch) => ch,
            Err(DecoderError::Eof) => break,
            Err(err) => return Err(err),
        }
    };
}

impl<R: Read> Parser<R> {
    pub(crate) fn parse_key(&mut self) -> Result<RawString, DecoderError> {
        self.drop_horizontal_whitespace()?;
        self.parse_path_expression()
    }

    pub(crate) fn parse_value(&mut self) -> Result<RawValue, DecoderError> {
        self.drop_whitespace()?;
        let mut values = vec![];
        let mut scratch = vec![];
        let mut spaces = vec![];
        loop {
            let ch = try_peek!(self.reader);
            match ch {
                '[' => {
                    // Parse array
                    let array = self.parse_array()?;
                    let v = RawValue::Array(array);
                    values.push(v);
                }
                '{' => {
                    // Parse object
                    let object = self.parse_object()?;
                    let v = RawValue::Object(object);
                    values.push(v);
                }
                '"' => {
                    // Parse quoted string or multi-line string
                    let v = if let Ok(chars) = self.reader.peek_n::<3>() && chars == ['"', '"', '"'] {
                        let multiline = self.parse_multiline_string()?;
                        RawValue::String(RawString::MultilineString(multiline))
                    } else {
                        let quoted = self.parse_quoted_string()?;
                        RawValue::String(RawString::QuotedString(quoted))
                    };
                    values.push(v);
                }
                '$' => {
                    let substitution = self.parse_substitution()?;
                    let v = RawValue::Substitution(substitution);
                    values.push(v);
                }
                ']' | '}' => {
                    break;
                }
                ',' | '#' | '\n' => {
                    if values.is_empty() {
                        return Err(DecoderError::UnexpectedToken {
                            expected: "a valid value",
                            found_beginning: ch,
                        });
                    }
                    break;
                }
                '/' => {
                    if let Ok((_, ch2)) = self.reader.peek2() {
                        if ch2 == '/' && !values.is_empty() {
                            break;
                        } else {
                            return Err(DecoderError::UnexpectedToken {
                                expected: "a valid value",
                                found_beginning: ch,
                            });
                        }
                    }
                }
                '\r' => {
                    if let Ok((_, ch2)) = self.reader.peek2() {
                        if ch2 == '\n' && !values.is_empty() {
                            break;
                        } else {
                            return Err(DecoderError::UnexpectedToken {
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
                        spaces.push(space.to_string());
                    } else {
                        let unquoted = self.parse_unquoted_string()?;
                        let v = RawValue::String(RawString::UnquotedString(unquoted));
                        values.push(v);
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
            Ok(RawValue::concat(values))
        }
    }

    pub(crate) fn parse_key_value(&mut self) -> Result<(RawString, RawValue), DecoderError> {
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

    pub fn drop_kv_separator(&mut self) -> Result<bool, DecoderError> {
        let ch = self.reader.peek()?;
        match ch {
            ':' | '=' => {
                self.reader.next()?;
            }
            '+' => {
                let (_, ch2) = self.reader.peek2()?;
                if ch2 != '=' {
                    return Err(DecoderError::UnexpectedToken {
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
                return Err(DecoderError::UnexpectedToken {
                    expected: ": or =",
                    found_beginning: ch,
                });
            }
        }
        Ok(false)
    }

    pub(crate) fn parse_object_field(&mut self) -> Result<ObjectField, DecoderError> {
        let ch = self.reader.peek()?;
        // It maybe a include syntax, we need to peek more chars to determine.
        let field = if ch == 'i' && self.reader.peek_n::<7>()? == INCLUDE {
            let inclusion = self.parse_include()?;
            ObjectField::inclusion(inclusion)
        } else {
            let (key, value) = self.parse_key_value()?;
            ObjectField::key_value(key, value)
        };
        Ok(field)
    }

    pub(crate) fn parse_object_fields(&mut self) -> Result<Vec<ObjectField>, DecoderError> {
        let mut fields = vec![];
        loop {
            self.drop_comments()?;
            let ch = self.reader.peek()?;
            if ch == '}' {
                break;
            }
            match self.parse_object_field() {
                Ok(field) => {
                    fields.push(field);
                }
                Err(DecoderError::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
            self.drop_whitespace()?;
            if self.drop_comma_separator()? {
                break;
            }
        }
        Ok(fields)
    }

    pub(crate) fn parse_root_object(&mut self) -> Result<RawObject, DecoderError> {
        let fields = self.parse_object_fields()?;
        let raw_obj = RawObject::new(fields);
        Ok(raw_obj)
    }

    pub(crate) fn parse_object(&mut self) -> Result<RawObject, DecoderError> {
        let ch = self.reader.peek()?;
        if ch != '{' {
            return Err(DecoderError::UnexpectedToken {
                expected: "{",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let fields = self.parse_object_fields()?;
        let ch = self.reader.peek()?;
        if ch != '}' {
            return Err(DecoderError::UnexpectedToken {
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

    pub(crate) fn parse_newline_comments(&mut self) -> Result<Vec<ObjectField>, DecoderError> {
        let mut fields = vec![];
        loop {
            match self.parse_comment() {
                Ok((ty, content)) => {
                    let comment = Comment::new(content, ty);
                    fields.push(ObjectField::newline_comment(comment));
                }
                Err(DecoderError::Eof | DecoderError::UnexpectedToken { .. }) => {
                    break Ok(fields);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
}
