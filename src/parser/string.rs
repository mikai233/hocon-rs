use crate::parser::{hocon_multi_space0, is_hocon_horizontal_whitespace, R};
use crate::raw::raw_string::{ConcatString, RawString};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while, take_while_m_n};
use nom::character::char;
use nom::character::complete::{anychar, multispace1, none_of};
use nom::combinator::{map, map_opt, map_res, peek, value, verify};
use nom::multi::{fold, many1};
use nom::sequence::{delimited, preceded, terminated};
use nom::Parser;

// pub(crate) fn unquoted_string(input: &str) -> R<&str> {
//     take_while1(|c| !is_hocon_whitespace(c) && !FORBIDDEN_CHARACTERS.contains(&c)).parse_complete(input)
// }

#[derive(Debug, Copy, Clone)]
enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
    EscapedWS,
}

// 辅助谓词函数：检查字符是否不是双引号或反斜杠
fn not_quote_slash(c: char) -> bool {
    c != '"' && c != '\\'
}

/// 解析 \u{XXXX} Unicode 转义序列，其中 XXXX 是 1 到 6 位十六进制数字。
fn parse_unicode(input: &str) -> R<char> {
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
fn parse_escaped_char(input: &str) -> R<char> {
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
fn parse_escaped_whitespace(input: &str) -> R<&str> {
    preceded(char('\\'), multispace1).parse_complete(input)
}

/// 解析一系列不是双引号（"）或反斜杠（\）的字符。
fn parse_literal(input: &str) -> R<&str> {
    verify(take_while(not_quote_slash), |s: &str| !s.is_empty()).parse_complete(input)
}

/// 将不同类型的字符串片段（字面量、转义字符、转义空白）组合成一个 StringFragment 枚举。
fn parse_fragment(input: &str) -> R<StringFragment> {
    alt((
        map(parse_literal, StringFragment::Literal),
        map(parse_escaped_char, StringFragment::EscapedChar),
        value(StringFragment::EscapedWS, parse_escaped_whitespace),
    )).parse_complete(input)
}

/// 解析完整的 HOCON 引用字符串，从开头的 " 到结尾的 "，并将内部片段组装成一个 String。
pub(crate) fn parse_quoted_string(input: &str) -> R<String> {
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
    matches!(c, '$' | '"' | '{' | '}' | '[' | ']' | ':' | '=' | ',' | '+' | '#' | '`' | '^' | '?' | '!' | '@' | '*' | '&' | '\\') ||
        is_hocon_horizontal_whitespace(c)
}

// 谓词函数：检查字符是否是未引用字符串中允许的单个字符（不包括对 `//` 的特殊处理）
fn is_unquoted_char_allowed_single(c: char) -> bool {
    !is_forbidden_unquoted_char(c)
}

/// 解析未引用字符串中的单个允许字符，处理 `//` 序列以避免将其作为字符串内容。
fn parse_unquoted_char(input: &str) -> R<char> {
    alt(
        (
            // 匹配任何允许且不是 '/' 的字符
            verify(anychar, |&c| is_unquoted_char_allowed_single(c) && c != '/'),
            // 匹配 '/' 字符，但仅当其后面没有另一个 '/' 时
            // 使用 `peek` 组合器向前查看而不消耗输入。
            preceded(char('/'), peek(none_of("/"))).map(|_| '/'),
        ),
    ).parse_complete(input)
}

/// 解析 HOCON 未引用字符串。
/// 未引用字符串不能包含特定禁止字符，也不能包含 `//` 序列 [2, 6, 7]。
/// 此外，它不能以 `true`, `false`, `null` 或数字开头（此检查通常在更高级别处理）[2, 6, 7]。
pub(crate) fn parse_unquoted_string(input: &str) -> R<String> {
    // `many1` 确保至少解析一个字符。
    let (remaining, fragments) = many1(parse_unquoted_char).parse_complete(input)?;

    let s: String = fragments.into_iter().collect();

    // HOCON 规范指出未引用字符串不能以 `true`, `false`, `null` 或数字开头 [2, 6, 7]。
    // 此函数仅处理字符集和 `//` 规则。
    // 完整的 HOCON 解析器应在尝试未引用字符串之前，先尝试解析布尔值、null 和数字。
    Ok((remaining, s))
}

/// 解析 HOCON 多行字符串。
/// 多行字符串以 `"""` 开头和结尾，内部所有字符（包括换行符和空格）都按字面意义处理 [2, 6]。
/// 不支持转义序列 [2]。
/// 任何至少三个引号的序列都会终止字符串，额外的引号被视为字符串内容的一部分 [2]。
// pub fn parse_hocon_multiline_string(input: &str) -> R<String> {
//     // 匹配开头的 """
//     let (input, _) = tag("\"\"\"")(input)?;
//
//     // 匹配直到下一个 """ 的所有字符。`take_until` 是非贪婪的，它会停止在第一个匹配项。
//     let (remaining, content) = take_until("\"\"\"")(input)?;
//
//     // 匹配结束的 """
//     let (remaining, _) = tag("\"\"\"")(remaining)?;
//
//     // 处理额外的引号：任何在结束 """ 之后的额外 " 字符都被视为字符串内容的一部分 [2]。
//     let (remaining, extra_quotes) = take_while(|c| c == '"')(remaining)?;
//
//     // 组合内容和额外引号
//     let mut result = String::from(content);
//     result.push_str(extra_quotes);
//
//     Ok((remaining, result))
// }
//
// pub(crate) fn quoted_string(input: &str) -> R<String> {
//     todo!()
// }

pub(crate) fn parse_multiline_string(input: &str) -> R<String> {
    delimited(
        tag(r#"""""#),
        take_until(r#"""""#),
        tag(r#"""""#),
    )
        .map(|x: &str| x.to_string())
        .parse_complete(input)
}

pub(crate) fn parse_hocon_string(input: &str) -> R<RawString> {
    many1(
        (
            alt(
                (
                    parse_multiline_string.map(RawString::MultiLineString),
                    parse_quoted_string.map(RawString::QuotedString),
                    parse_unquoted_string.map(RawString::UnquotedString),
                )
            ),
            hocon_multi_space0.map(|s| s.to_string()),
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
    use crate::parser::string::{parse_hocon_string, parse_multiline_string, parse_quoted_string, parse_unquoted_string};

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
        let (r, o) = parse_hocon_string("4 5.0").unwrap();
        let (r, o) = parse_hocon_string("\"\"\"a\"\"\". b.\" c\"").unwrap();
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
}