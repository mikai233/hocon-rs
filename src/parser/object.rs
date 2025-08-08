use crate::parser::include::parse_include;
use crate::parser::string::parse_hocon_string;
use crate::parser::{hocon_multi_space0, next_element_whitespace, parse_value, R};
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
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
                    hocon_multi_space0,
                    char('{'),
                    object_fields0,
                    hocon_multi_space0,
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
                (char(','), hocon_multi_space0.map(|_| ','))
            ),
            alt(
                (
                    map(parse_include, |v| ObjectField::Inclusion(v)),
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
                (char(','), hocon_multi_space0.map(|_| ','))
            ),
            alt(
                (
                    map(parse_include, |v| ObjectField::Inclusion(v)),
                    map(key_value, |(k, v)| ObjectField::KeyValue(k, v)),
                )
            ),
        ).map(RawObject::new))
        .parse(input)
}

fn key_value(input: &str) -> R<(RawString, RawValue)> {
    let (input, (_, path, _, _, _, value, _)) = context(
        "key_value",
        (
            hocon_multi_space0,
            parse_hocon_string,
            hocon_multi_space0,
            separator,
            hocon_multi_space0,
            parse_value,
            next_element_whitespace,
        ),
    ).parse(input)?;
    Ok((input, (path, value)))
}

fn separator(input: &str) -> R<()> {
    value((), alt((char(':'), char('='), peek(char('{'))))).parse(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::load_conf;
    use crate::parser::object::{key_value, object};
    use crate::raw::field::ObjectField;
    use crate::raw::raw_string::RawString;
    use crate::raw::raw_value::RawValue;
    use nom::Err;
    use nom_language::error::convert_error;

    #[test]
    fn test_key_value() -> crate::Result<()> {
        let (_, (k, v)) = key_value("hello=world").unwrap();
        println!("{} {}", k, v);
        let (_, (k, v)) = key_value("hello=true").unwrap();
        println!("{} {}", k, v);
        let (_, (k, v)) = key_value("hello = true false").unwrap();
        println!("{} {}", k, v);
        Ok(())
    }

    #[test]
    fn test_object1() -> crate::Result<()> {
        let conf = load_conf("object1")?;
        let (r, o) = object(conf.as_str()).unwrap();
        assert!(r.is_empty());
        let k = RawString::unquoted("b");
        let v = RawValue::object_kv([(RawString::unquoted("hello"), RawValue::quoted_string("world"))]);
        assert_eq!(&o[0], &ObjectField::KeyValue(k, v));
        let k = RawString::unquoted("a");
        let v = RawValue::object_kv([]);
        assert_eq!(&o[1], &ObjectField::KeyValue(k, v));
        Ok(())
    }

    #[test]
    fn test_object2() -> crate::Result<()> {
        let conf = load_conf("object2")?;
        let e = object(conf.as_str()).err().unwrap();
        match e {
            Err::Incomplete(_) => {}
            Err::Error(e) => {
                println!("{}", convert_error(conf.as_str(), e));
            }
            Err::Failure(_) => {}
        }
        return Ok(());
        // let (r, o) = object(conf.as_str()).unwrap();
        // let v = ("a", ("b", "hello".r()).r()).r();
        // assert!(r.is_empty());
        // assert_eq!(&o[0], &ObjectField::KeyValue("a".to_string(), RawValue::Object(RawObject::with_kvs([("b".to_string(), RawValue::QuotedString("hello".to_string()))]))));
        // assert_eq!(&o[1], &ObjectField::KeyValue("b".to_string(), RawValue::Object(RawObject::with_kvs([("bb".to_string(), RawValue::Object(RawObject::default())), ("cc".to_string(), RawValue::Object(RawObject::default()))]))));
        Ok(())
    }
}