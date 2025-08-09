use crate::parser::{hocon_horizontal_multi_space0, is_hocon_horizontal_whitespace, is_hocon_whitespace, R};
use crate::raw::raw_string::{ConcatString, RawString};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while, take_while_m_n};
use nom::character::char;
use nom::character::complete::{anychar, multispace1};
use nom::combinator::{map, map_opt, map_res, not, peek, value, verify};
use nom::multi::{fold, many1, separated_list1};
use nom::sequence::{delimited, preceded, terminated};
use nom::Parser;
use std::ops::{Deref, DerefMut};

const FORBIDDEN_CHARACTERS: [char; 19] = ['$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\'];

#[derive(Debug, Copy, Clone)]
enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
    EscapedWS,
}

#[derive(Debug, Clone)]
enum Path {
    Quoted(String),
    Unquoted(String),
    Multiline(String),
}

impl Deref for Path {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        match self {
            Path::Quoted(s) |
            Path::Unquoted(s) |
            Path::Multiline(s) => s
        }
    }
}

impl DerefMut for Path {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Path::Quoted(s) |
            Path::Unquoted(s) |
            Path::Multiline(s) => s
        }
    }
}

// 辅助谓词函数：检查字符是否不是双引号或反斜杠
fn not_quote_slash(c: char) -> bool {
    c != '"' && c != '\\'
}

/// 解析 \u{XXXX} Unicode 转义序列，其中 XXXX 是 1 到 6 位十六进制数字。
fn parse_unicode(input: &str) -> R<'_, char> {
    preceded(
        char('\\'),
        map_opt(
            map_res(
                preceded(
                    char('{'),
                    terminated(
                        take_while_m_n(1, 6, |c: char| c.is_ascii_hexdigit()), char('}'),
                    ),
                ),
                |hex| u32::from_str_radix(hex, 16),
            ),
            std::char::from_u32,
        ),
    ).parse_complete(input)
}

/// 解析标准 JSON 转义序列，如 \n, \t, \", \\, \/, \b, \f，并集成 parse_unicode。
fn parse_escaped_char(input: &str) -> R<'_, char> {
    preceded(
        char('\\'),
        alt((
            parse_unicode,
            value('\n', char('n')),
            value('\r', char('r')),
            value('\t', char('t')),
            value('\u{08}', char('b')), // Backspace
            value('\u{0C}', char('f')), // Form feed
            value('\\', char('\\')),
            value('/', char('/')),
            value('"', char('"')),
        )),
    )
        .parse_complete(input)
}

/// 解析反斜杠后跟一个或多个空白字符的序列。
fn parse_escaped_whitespace(input: &str) -> R<'_, &str> {
    preceded(char('\\'), multispace1).parse_complete(input)
}

/// 解析一系列不是双引号（"）或反斜杠（\）的字符。
fn parse_literal(input: &str) -> R<'_, &str> {
    verify(take_while(not_quote_slash), |s: &str| !s.is_empty()).parse_complete(input)
}

/// 将不同类型的字符串片段（字面量、转义字符、转义空白）组合成一个 StringFragment 枚举。
fn parse_fragment(input: &str) -> R<'_, StringFragment<'_>> {
    alt((
        map(parse_literal, StringFragment::Literal),
        map(parse_escaped_char, StringFragment::EscapedChar),
        value(StringFragment::EscapedWS, parse_escaped_whitespace),
    )).parse_complete(input)
}

/// 解析完整的 HOCON 引用字符串，从开头的 " 到结尾的 "，并将内部片段组装成一个 String。
pub(crate) fn parse_quoted_string(input: &str) -> R<'_, String> {
    let build_string = fold(
        0.., // 解析 `parse_fragment` 零次或多次
        parse_fragment, // 用于单个字符串片段的解析器
        String::new, // 初始累加器，一个空字符串

        |mut string, fragment| { // 折叠函数：将片段追加到字符串
            match fragment {
                StringFragment::Literal(s) => string.push_str(s),
                StringFragment::EscapedChar(c) => string.push(c),
                StringFragment::EscapedWS => {} // 忽略转义的空白
            }
            string
        },
    );

    delimited(char('"'), build_string, char('"')).parse_complete(input)
}

// 谓词函数：检查字符是否是未引用字符串中的禁止字符
fn is_forbidden_unquoted_char(c: char) -> bool {
    FORBIDDEN_CHARACTERS.contains(&c) || is_hocon_whitespace(c)
}

/// 解析未引用字符串中的单个允许字符，处理 `//` 序列以避免将其作为字符串内容。
fn parse_unquoted_char(input: &str) -> R<'_, char> {
    alt(
        (
            // 匹配任何允许且不是 '/' 的字符
            verify(anychar, |&c| !is_forbidden_unquoted_char(c) && c != '/'),
            // 匹配 '/' 字符，但仅当其后面没有另一个 '/' 时
            // 使用 `peek` 组合器向前查看而不消耗输入。
            (char('/'), peek(not(char('/')))).map(|_| '/'),
        ),
    ).parse_complete(input)
}

fn parse_unquoted_path_char(input: &str) -> R<'_, char> {
    alt(
        (
            verify(anychar, |c| !FORBIDDEN_CHARACTERS.contains(&c) && *c != '/' && *c != '.'),
            (char('/'), peek(not(char('/')))).map(|_| '/'),
        ),
    ).parse_complete(input)
}

pub(crate) fn parse_unquoted_string(input: &str) -> R<'_, String> {
    fold(
        1..,
        parse_unquoted_char,
        String::new,
        |mut acc, char| {
            acc.push(char);
            acc
        },
    ).parse_complete(input)
}

fn parse_unquoted_path_expression(input: &str) -> R<'_, String> {
    verify(
        fold(
            1..,
            parse_unquoted_path_char,
            String::new,
            |mut acc, c| {
                acc.push(c);
                acc
            },
        ),
        |path: &String| path.chars().any(|c| !is_hocon_horizontal_whitespace(c)),
    ).parse_complete(input)
}

pub(crate) fn parse_multiline_string(input: &str) -> R<'_, String> {
    delimited(
        tag(r#"""""#),
        take_until(r#"""""#),
        tag(r#"""""#),
    )
        .map(|x: &str| x.to_string())
        .parse_complete(input)
}

fn parse_path(input: &str) -> R<'_, Vec<Path>> {
    separated_list1(
        char('.'),
        alt(
            (
                parse_multiline_string.map(Path::Multiline),
                parse_quoted_string.map(Path::Quoted),
                parse_unquoted_path_expression.map(Path::Unquoted),
            )
        ),
    )
        .map(|mut path| {
            path.last_mut().map(|p| {
                let trimmed_len = p.trim_end_matches(is_hocon_horizontal_whitespace).len();
                p.truncate(trimmed_len);
            });
            path
        })
        .parse_complete(input)
}

pub(crate) fn parse_key(input: &str) -> R<'_, RawString> {
    parse_path.map(|paths| {
        let mut keys = Vec::with_capacity(paths.len());
        for path in paths {
            let key = match path {
                Path::Quoted(s) => (RawString::quoted(s), ""),
                Path::Unquoted(s) => (RawString::unquoted(s), ""),
                Path::Multiline(s) => (RawString::multiline(s), "")
            };
            keys.push(key);
        }
        RawString::concat(keys.into_iter())
    }).parse_complete(input)
}

pub(crate) fn parse_path_expression(input: &str) -> R<'_, RawString> {
    parse_key.parse_complete(input)
}

pub(crate) fn parse_string(input: &str) -> R<'_, RawString> {
    many1(
        (
            alt(
                (
                    parse_multiline_string.map(RawString::MultilineString),
                    parse_quoted_string.map(RawString::QuotedString),
                    parse_unquoted_string.map(RawString::UnquotedString),
                )
            ),
            hocon_horizontal_multi_space0.map(|s| s.to_string()),
        )
    )
        .map(maybe_concat)
        .parse_complete(input)
}

fn maybe_concat(mut values: Vec<(RawString, String)>) -> RawString {
    assert!(!values.is_empty());
    if values.len() == 1 {
        values.remove(0).0
    } else {
        RawString::ConcatString(ConcatString::new(values))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::string::{parse_multiline_string, parse_path, parse_path_expression, parse_quoted_string, parse_string, parse_unquoted_path_char, parse_unquoted_path_expression, parse_unquoted_string};
    use crate::parser::substitution::parse_substitution;

    #[test]
    fn test_unquoted_string() {
        let (r, o) = parse_unquoted_string("4  5.0").unwrap();
        assert_eq!(r, "  5.0");
        assert_eq!(o, "4");
    }

    #[test]
    fn test_quoted_string() {
        let (remainder, result) = parse_quoted_string("\"world\"").unwrap();
        assert_eq!(remainder, "");
        assert_eq!(result, "world");
        let (remainder, result) = parse_quoted_string("\"world\"112233").unwrap();
        assert_eq!(remainder, "112233");
        assert_eq!(result, "world");
    }

    #[test]
    fn test_multiline_string() {
        let (r, o) = parse_multiline_string(r#""""""Hello
World!""""""#).unwrap();
        assert_eq!(r, r#""""#);
        assert_eq!(o, "\"\"Hello\nWorld!");
    }

    #[test]
    fn test_string() {
        let (r, o) = parse_string("4 5.0").unwrap();
        let (r, o) = parse_string("\"\"\"a\"\"\". b.\" c\"").unwrap();
        println!("{}", o.synthetic());
        // assert!(r.is_empty());
        // let v = RawString::ConcatString(ConcatString::new(vec![(RawString::UnquotedString("4".to_string()), " ".to_string()), (RawString::UnquotedString("5.0".to_string()), "".to_string())]));
        // assert_eq!(o, v);
        // let v = quoted_string("\"\"").unwrap();
        // println!("{}=={}", v.0, v.1);
        // let (r, o) = parse_hocon_quoted_string("\"vv \" 1").unwrap();
        // println!("{}={}", r, o);
        // let (r, o) = parse_hocon_quoted_string("\"1\r\n\"").unwrap();
        // println!("k{}={:?}", r, o);
    }

    #[test]
    fn test_unquoted_path_expression() {
        let (r, o) = parse_substitution("${a}").unwrap();
        println!("{o:?}");
        // parse_unquoted_path_char("/ ").unwrap();
        // let (r, o) = parse_unquoted_path_expression(" c / }").unwrap();
        // println!("{}={}", r, o);
    }
}