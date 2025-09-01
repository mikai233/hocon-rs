use crate::{
    parser::pure::{
        parser::Parser,
        read::{DecoderError, Read},
    },
    raw::raw_array::RawArray,
};

impl<R: Read> Parser<R> {
    pub(crate) fn parse_array(&mut self) -> Result<RawArray, DecoderError> {
        let ch = self.reader.peek()?;
        if ch != '[' {
            return Err(DecoderError::UnexpectedToken {
                expected: "[",
                found_beginning: ch,
            });
        }
        self.reader.next()?;
        let mut values = vec![];
        loop {
            self.drop_comments()?;
            let ch = self.reader.peek()?;
            if ch == ']' {
                self.reader.next()?;
                break;
            }
            let v = self.parse_value()?;
            values.push(v);
            self.drop_whitespace()?;
            if self.drop_comma_separator()? {
                break;
            }
        }
        Ok(RawArray::new(values))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::{parser::pure::read::DecoderError, raw::raw_value::RawValue};

    #[rstest]
    #[case("[1,2,3]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[true,false,null]", vec![RawValue::Boolean(true), RawValue::Boolean(false), RawValue::Null])]
    #[case("[1,2 ,3,\n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[1\r\n2 ,3, \n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[1\r\n2.0001 ,3, \n]", vec![RawValue::number(1), RawValue::number(serde_json::Number::from_f64(2.0001).unwrap()), RawValue::number(3)])]
    #[case("[1\r\n2.0001f ,3, \n]", vec![RawValue::number(1), RawValue::unquoted_string("2.0001f"), RawValue::number(3)])]
    fn test_valid_array(
        #[case] input: &str,
        #[case] expected: Vec<RawValue>,
    ) -> Result<(), DecoderError> {
        use std::io::BufReader;

        use crate::parser::pure::{parser::Parser, read::TestStreamRead};

        let read = TestStreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = Parser::new(read);
        let values = parser.parse_array()?.into_inner();
        assert_eq!(values, expected);
        Ok(())
    }
}
