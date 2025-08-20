use crate::parser::comment::parse_comment;
use crate::parser::include::parse_include;
use crate::parser::string::parse_key;
use crate::parser::{R, hocon_horizontal_multi_space0, hocon_multi_space0, parse_value};
use crate::raw::comment::Comment;
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{map, opt, peek, value};
use nom::error::context;
use nom::multi::{many_m_n, many0};
use nom::sequence::delimited;
use tracing::info;

pub(crate) fn parse_object(input: &str) -> R<'_, RawObject> {
    let (remainder, object) = delimited(
        (char('{'), hocon_multi_space0),
        map(many0(object_element), |fields| {
            RawObject::from_iter(fields.into_iter().flatten())
        }),
        (hocon_multi_space0, char('}')),
    )
    .parse_complete(input)?;
    Ok((remainder, object))
}

pub(crate) fn parse_root_object(input: &str) -> R<'_, RawObject> {
    delimited(
        hocon_multi_space0,
        map(many0(object_element), |fields| {
            RawObject::from_iter(fields.into_iter().flatten())
        }),
        hocon_multi_space0,
    )
    .parse_complete(input)
}

fn object_element(input: &str) -> R<'_, Vec<ObjectField>> {
    fn newline_comments(input: &str) -> R<'_, Vec<ObjectField>> {
        many0(delimited(
            hocon_multi_space0,
            parse_comment.map(|(ty, content)| {
                ObjectField::newline_comment(Comment::new(content.to_string(), ty))
            }),
            hocon_multi_space0,
        ))
        .parse_complete(input)
    }

    fn current_line_comment(input: &str) -> R<'_, Comment> {
        parse_comment
            .map(|(ty, content)| Comment::new(content.to_string(), ty))
            .parse_complete(input)
    }

    fn object_field(input: &str) -> R<'_, ObjectField> {
        alt((
            map(parse_include, ObjectField::inclusion),
            map(parse_key_value, |(k, v)| ObjectField::key_value(k, v)),
            map(parse_add_assign, |(k, v)| ObjectField::key_value(k, v)),
        ))
        .parse_complete(input)
    }

    fn separator(input: &str) -> R<'_, ()> {
        value(
            (),
            (hocon_horizontal_multi_space0, many_m_n(0, 1, char(','))),
        )
        .parse_complete(input)
    }

    let (
        remainder,
        (newline_comments_before, mut field, _, current_line_comment, newline_comments_after),
    ) = (
        newline_comments,
        object_field,
        separator,
        opt(current_line_comment),
        newline_comments,
    )
        .parse_complete(input)?;
    if let Some(c) = current_line_comment {
        field.set_comment(c);
    }
    let mut fields = newline_comments_before;
    fields.push(field);
    fields.extend(newline_comments_after);
    Ok((remainder, fields))
}

fn parse_key_value(input: &str) -> R<'_, (RawString, RawValue)> {
    fn separator(input: &str) -> R<'_, ()> {
        value((), alt((char(':'), char('='), peek(char('{'))))).parse_complete(input)
    }

    let (remainder, (_, key, _, _, _, value)) = context(
        "parse_key_value",
        (
            hocon_multi_space0,
            parse_key,
            hocon_multi_space0,
            separator,
            hocon_multi_space0,
            parse_value,
        ),
    )
    .parse_complete(input)?;
    Ok((remainder, (key, value)))
}

fn parse_add_assign(input: &str) -> R<'_, (RawString, RawValue)> {
    let (remainder, (_, key, _, _, _, value)) = context(
        "parse_add_assign",
        (
            hocon_multi_space0,
            parse_key,
            hocon_multi_space0,
            tag("+="),
            hocon_multi_space0,
            parse_value.map(RawValue::add_assign),
        ),
    )
    .parse_complete(input)?;
    Ok((remainder, (key, value)))
}

#[cfg(test)]
mod tests {
    use crate::parser::object::{object_element, parse_key_value, parse_object};
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
    fn test_object_element() {
        let (r, o) = object_element("b = ${b},, // test comment").unwrap();
        println!("{} {:?}", r, o);
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

    #[test]
    fn test_object3() -> crate::Result<()> {
        let conf = load_conf("object3")?;
        let (remainder, object) = parse(conf.as_str()).unwrap();
        assert_eq!(remainder, "");
        println!("{:?}", object);
        Ok(())
    }
}
