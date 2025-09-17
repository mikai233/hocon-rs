#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::Result;
    use crate::parser::HoconParser;
    use crate::parser::frame::Frame;
    use crate::parser::read::StreamRead;
    use crate::raw::raw_value::RawValue;
    use std::io::BufReader;

    #[rstest]
    #[case("[1,2,3]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[true,false,null]", vec![RawValue::Boolean(true), RawValue::Boolean(false), RawValue::Null])]
    #[case("[1,2 ,3,\n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[1\r\n2 ,3, \n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("[1\r\n2.0001 ,3, \n]", vec![RawValue::number(1), RawValue::number(serde_json::Number::from_f64(2.0001).unwrap()), RawValue::number(3)])]
    #[case("[1\r\n2.0001f ,3, \n]", vec![RawValue::number(1), RawValue::unquoted_string("2.0001f"), RawValue::number(3)])]
    fn test_valid_array(#[case] input: &str, #[case] expected: Vec<RawValue>) -> Result<()> {
        let read = StreamRead::new(BufReader::new(input.as_bytes()));
        let mut parser = HoconParser::new(read);
        parser.parse_iteration()?;
        assert!(parser.stack.len() == 1);
        let frame = parser.stack.pop().unwrap();
        match frame {
            Frame::Array {
                elements: values, ..
            } => {
                assert_eq!(values, expected);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
