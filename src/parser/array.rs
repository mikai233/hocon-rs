#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::Result;
    use crate::parser::HoconParser;
    use crate::parser::read::StrRead;
    use crate::raw::raw_value::RawValue;

    #[rstest]
    #[case("array = [1,2,3]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("array = [true,false,null]", vec![RawValue::Boolean(true), RawValue::Boolean(false), RawValue::Null])]
    #[case("array = [1,2 ,3,\n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("array = [1\r\n2 ,3, \n]", vec![RawValue::number(1), RawValue::number(2), RawValue::number(3)])]
    #[case("array = [1\r\n2.0001 ,3, \n]", vec![RawValue::number(1), RawValue::number(serde_json::Number::from_f64(2.0001).unwrap()), RawValue::number(3)])]
    #[case("array = [1\r\n2.0001f ,3, \n]", vec![RawValue::number(1), RawValue::unquoted_string("2.0001f"), RawValue::number(3)])]
    fn test_valid_array(#[case] input: &str, #[case] expected: Vec<RawValue>) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let raw_object = parser.parse()?;
        let field = &raw_object[0];
        match field {
            crate::raw::field::ObjectField::KeyValue { key, value, .. } => {
                assert_eq!(key.to_string(), "array");
                match value {
                    RawValue::Array(raw_array) => {
                        assert_eq!(raw_array.0, expected);
                    }
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("unexpected field type"),
        }
        Ok(())
    }
}
