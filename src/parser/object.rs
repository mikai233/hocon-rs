use crate::parser::include::include;
use crate::parser::string::{quoted_string, unquoted_string};
use crate::parser::{next_element_whitespace, parse_value, whitespace, R};
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::character::complete::char;
use nom::combinator::{map, peek, value};
use nom::error::context;
use nom::multi::{separated_list0, separated_list1};
use nom::Parser;

pub(crate) fn object(input: &str) -> R<RawObject> {
    context(
        "object",
        alt(
            (
                (
                    whitespace,
                    char('{'),
                    object_fields0,
                    whitespace,
                    char('}'),
                ).map(|(_, _, o, _, _)| o),
                object_fields1,
            )
        ),
    ).parse(input)
}

pub(crate) fn object_fields0(input: &str) -> R<RawObject> {
    context(
        "object_fields0",
        separated_list0(
            alt(
                (char(','), whitespace.map(|_| ','))
            ),
            alt(
                (
                    map(include, |v| ObjectField::Inclusion(v)),
                    map(key_value, |(k, v)| ObjectField::KeyValue(k, v)),
                )
            ),
        ).map(RawObject::new))
        .parse(input)
}

pub(crate) fn object_fields1(input: &str) -> R<RawObject> {
    context(
        "object_fields1",
        separated_list1(
            alt(
                (char(','), whitespace.map(|_| ','))
            ),
            alt(
                (
                    map(include, |v| ObjectField::Inclusion(v)),
                    map(key_value, |(k, v)| ObjectField::KeyValue(k, v)),
                )
            ),
        ).map(RawObject::new))
        .parse(input)
}

fn key_value(input: &str) -> R<(String, RawValue)> {
    let (input, (_, path, _, _, _, value, _)) = context(
        "key_value",
        (
            whitespace,
            alt((quoted_string, unquoted_string)),
            whitespace,
            separator,
            whitespace,
            parse_value,
            next_element_whitespace,
        ),
    ).parse(input)?;
    Ok((input, (path.to_string(), value)))
}

fn separator(input: &str) -> R<()> {
    value((), alt((char(':'), char('='), peek(char('{'))))).parse(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::load_conf;
    use crate::parser::object::object;
    use crate::raw::field::ObjectField;
    use crate::raw::raw_object::RawObject;
    use crate::raw::raw_value::RawValue;

    #[test]
    fn test_object1() -> crate::Result<()> {
        let conf = load_conf("object1")?;
        let (r, o) = object(conf.as_str()).unwrap();
        assert!(r.is_empty());
        assert_eq!(&o[0], &ObjectField::KeyValue("b".to_string(), RawValue::Object(RawObject::with_kvs([("hello".to_string(), RawValue::String("world".to_string()))]))));
        assert_eq!(&o[1], &ObjectField::KeyValue("a".to_string(), RawValue::Object(RawObject::default())));
        Ok(())
    }

    #[test]
    fn test_object2() -> crate::Result<()> {
        let conf = load_conf("object2")?;
        let (r, o) = object(conf.as_str()).unwrap();
        assert!(r.is_empty());
        assert_eq!(&o[0], &ObjectField::KeyValue("a".to_string(), RawValue::Object(RawObject::with_kvs([("b".to_string(), RawValue::String("hello".to_string()))]))));
        assert_eq!(&o[1], &ObjectField::KeyValue("b".to_string(), RawValue::Object(RawObject::with_kvs([("bb".to_string(), RawValue::Object(RawObject::default())), ("cc".to_string(), RawValue::Object(RawObject::default()))]))));
        Ok(())
    }
}