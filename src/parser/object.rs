use crate::Result;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::Read;
use crate::raw::{
    comment::Comment, field::ObjectField, raw_string::RawString, raw_value::RawValue,
};
use std::str::FromStr;

#[macro_export(local_inner_macros)]
macro_rules! try_peek {
    ($reader:expr) => {
        match $reader.peek() {
            Ok(ch) => ch,
            Err($crate::error::Error::Eof) => break,
            Err(err) => return Err(err),
        }
    };
}

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_key(reader: &mut R, scratch: &mut Vec<u8>) -> Result<RawString> {
        Self::drop_horizontal_whitespace(reader)?;
        Self::parse_path_expression(reader, scratch)
    }

    pub(crate) fn resolve_unquoted_string(string: RawString) -> RawValue {
        if let RawString::UnquotedString(unquoted) = &string {
            match &**unquoted {
                "true" => RawValue::Boolean(true),
                "false" => RawValue::Boolean(false),
                "null" => RawValue::Null,
                other => match other.as_bytes().first() {
                    Some(b'-' | b'0'..=b'9') => match serde_json::Number::from_str(other) {
                        Ok(number) => RawValue::Number(number),
                        Err(_) => RawValue::unquoted_string(unquoted),
                    },
                    Some(_) | None => RawValue::String(string),
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
            match Self::parse_comment(&mut self.reader) {
                Ok((ty, content)) => {
                    let comment = Comment::new(content, ty);
                    fields.push(ObjectField::newline_comment(comment));
                }
                Err(Error::Eof | Error::Parse { .. }) => {
                    break Ok(fields);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
}
