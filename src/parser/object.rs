use crate::parser::include::parse_include;
use crate::parser::string::parse_key;
use crate::parser::{hocon_multi_space0, next_element_whitespace, parse_value, R};
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use nom::branch::alt;
use nom::character::complete::char;
use nom::combinator::{map, peek, value};
use nom::error::context;
use nom::multi::many0;
use nom::sequence::delimited;
use nom::Parser;

pub(crate) fn parse_object(input: &str) -> R<'_, RawObject> {
    let (remainder, (object, )) = (
        delimited(
            char('{'),
            map(many0(object_element), RawObject::new),
            char('}'),
        ),
    ).parse_complete(input)?;
    Ok((remainder, object))
}

pub(crate) fn parse_root_object(input: &str) -> R<'_, RawObject> {
    let (remainder, (_, object, _)) = (
        hocon_multi_space0,
        map(many0(object_element), RawObject::new),
        hocon_multi_space0,
    ).parse_complete(input)?;
    Ok((remainder, object))
}

fn object_element(input: &str) -> R<'_, ObjectField> {
    let (remainder, (_, field, _)) = (
        hocon_multi_space0,
        alt(
            (
                map(parse_include, ObjectField::Inclusion),
                map(parse_key_value, |(k, v)| ObjectField::KeyValue(k, v)),
            )
        ),
        next_element_whitespace,
    ).parse_complete(input)?;
    Ok((remainder, field))
}

fn parse_key_value(input: &str) -> R<'_, (RawString, RawValue)> {
    let (remainder, (_, key, _, _, _, value, _)) = context(
        "key_value",
        (
            hocon_multi_space0,
            parse_key,
            hocon_multi_space0,
            separator,
            hocon_multi_space0,
            parse_value,
            next_element_whitespace,
        ),
    ).parse_complete(input)?;
    Ok((remainder, (key, value)))
}

fn separator(input: &str) -> R<'_, ()> {
    value((), alt((char(':'), char('='), peek(char('{'))))).parse_complete(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::object::{parse_key_value, parse_object};
    use crate::parser::{load_conf, parse};
    use nom::Err;
    use nom_language::error::convert_error;

    #[test]
    fn test_key_value() -> crate::Result<()> {
        let (_, (k, v)) = parse_key_value("hello=world").unwrap();
        println!("{} {}", k, v);
        let (_, (k, v)) = parse_key_value("hello=true").unwrap();
        println!("{} {}", k, v);
        let (_, (k, v)) = parse_key_value("hello = true false").unwrap();
        println!("{} {}", k, v);
        Ok(())
    }

    #[test]
    fn test_object1() -> crate::Result<()> {
        let conf = load_conf("object1")?;
        let (r, o) = parse(conf.as_str()).unwrap();
        println!("remainder:{r}");
        println!("result:{o}");
        // assert!(r.is_empty());
        // let k = RawString::unquoted("b");
        // let v = RawValue::object_kv([(RawString::unquoted("hello"), RawValue::quoted_string("world"))]);
        // assert_eq!(&o[0], &ObjectField::KeyValue(k, v));
        // let k = RawString::unquoted("a");
        // let v = RawValue::object_kv([]);
        // assert_eq!(&o[1], &ObjectField::KeyValue(k, v));
        Ok(())
    }

    #[test]
    fn test_object2() -> crate::Result<()> {
        let conf = load_conf("object2")?;
        let e = parse_object(conf.as_str()).err().unwrap();
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