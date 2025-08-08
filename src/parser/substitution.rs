use crate::parser::string::{parse_multiline_string, parse_quoted_string, parse_unquoted_string};
use crate::parser::{hocon_horizontal_multi_space0, hocon_multi_space0, R};
use crate::raw::substitution::Substitution;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::char;
use nom::combinator::{map, opt};
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::Parser;

/// 1. 解析路径中的单个标识符。
/// HOCON 路径标识符不能包含 . $ { } " 和空白。
fn parse_identifier(input: &str) -> R<&str> {
    is_not(".${}\" \t\r\n\u{001C}\u{001D}\u{001E}\u{001F}")(input)
}

fn p(input: &str) -> R<String> {
    alt(
        (
            parse_multiline_string,
            parse_quoted_string,
            parse_unquoted_string,
        )
    ).parse_complete(input)
}

/// 2. 解析由点分隔的完整路径，同时处理点周围的空白。
fn parse_path(input: &str) -> R<Vec<String>> {
    // 定义一个能匹配点(.)并且忽略其两边空白的解析器
    let dot_separator = delimited(hocon_multi_space0, char('.'), hocon_multi_space0);
    separated_list1(
        char('.'),
        alt(
            (
                parse_multiline_string,
                parse_quoted_string,
                parse_unquoted_string,
            )
        ),
    ).parse_complete(input)
}

/// 3. 解析完整的 HOCON 替换表达式
pub(crate) fn parse_substitution(input: &str) -> R<Substitution> {
    // // a. 首先，定义一个解析器，用于解析花括号内的所有内容
    //
    // // b. 使用 delimited 来处理花括号和内部的空白
    // let full_inner_parser = delimited(
    //     // 忽略左花括号之后和路径之前的所有空白
    //     hocon_multi_space0,
    //     pair(
    //         (
    //             opt(char('?')),
    //             hocon_multi_space0,
    //         ),
    //         parse_path,
    //     ),
    //     // 忽略路径之后和右花括号之前的所有空白
    //     hocon_multi_space0,
    // );
    //
    // // c. 将解析出的 (Option<char>, Vec<&str>) 映射到我们的 HoconSubstitution 结构体
    // let substitution_mapper = map(full_inner_parser, |((optional_char, _), path)| {
    //     Substitution::new(path, optional_char.is_some())
    // });
    //
    // // d. 最后，用 `delimited` 包裹整个表达式，匹配 `${` 和 `}`
    delimited(
        tag("${"),
        map(
            (
                opt(char('?')),
                hocon_horizontal_multi_space0,
                parse_path,
                hocon_horizontal_multi_space0,
            ),
            |(optional, _, path, _)| {
                Substitution::new(path, optional.is_some())
            },
        ),
        tag("}"),
    )
        .parse_complete(input)
}

#[cfg(test)]
mod tests {
    use crate::parser::substitution::parse_substitution;

    #[test]
    fn test_substitution() {
        parse_substitution("${? \"a .\".b. c}").unwrap();
        // let (r, o) = parse_substitution("${? a.b.c}").unwrap();
        // println!("{}={}", r, o);
        // let (r, o) = parse_substitution("${? \"a .\".b.c}").unwrap();
        // println!("{}={}", r, o);
        // let (r, o) = parse_substitution("${? \"a .\".b.c}").unwrap();
        // println!("{}={}", r, o);
        let (r, o) = parse_substitution("${? \"a .\".b. c}").unwrap();
        println!("{}={}", r, o);
    }
}