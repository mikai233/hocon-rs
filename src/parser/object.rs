use crate::parser::comment::parse_comment;
use crate::parser::include::parse_include;
use crate::parser::string::parse_key;
use crate::parser::{R, hocon_horizontal_space0, hocon_multi_space0, parse_value};
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
        value((), (hocon_horizontal_space0, many_m_n(0, 1, char(',')))).parse_complete(input)
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
    use crate::parser::object::parse_key_value;
    use crate::raw::raw_object::RawObject;
    use crate::raw::raw_string::RawString;
    use crate::raw::raw_value::RawValue;
    use rstest::rstest;

    #[rstest]
    #[case(
        "hello=world",
        RawString::unquoted("hello"),
        RawValue::unquoted_string("world"),
        ""
    )]
    #[case(
        "hello= \tworld",
        RawString::unquoted("hello"),
        RawValue::unquoted_string("world"),
        ""
    )]
    #[case(
        "\nhello= \r\nworld",
        RawString::unquoted("hello"),
        RawValue::unquoted_string("world"),
        ""
    )]
    #[case(
        "\n\"foo\"= \r\n\"bar\"",
        RawString::quoted("foo"),
        RawValue::quoted_string("bar"),
        ""
    )]
    #[case(
        "hello : world\n",
        RawString::unquoted("hello"),
        RawValue::unquoted_string("world"),
        "\n"
    )]
    #[case(
        "hello : world,\n",
        RawString::unquoted("hello"),
        RawValue::unquoted_string("world"),
        ",\n"
    )]
    #[case(
        "hello : {a = 1},\n",
        RawString::unquoted("hello"),
        RawValue::Object(RawObject::key_value(vec![(RawString::unquoted("a"),RawValue::Number(serde_json::Number::from_i128(1).unwrap()))]
        )),
        ",\n"
    )]
    fn test_valid_key_value(
        #[case] input: &str,
        #[case] expected_key: RawString,
        #[case] expected_value: RawValue,
        #[case] expected_rest: &str,
    ) {
        let result = parse_key_value(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, (key, value)) = result.unwrap();
        assert_eq!(expected_key, key);
        assert_eq!(expected_value, value);
        assert_eq!(rest, expected_rest);
    }
}
