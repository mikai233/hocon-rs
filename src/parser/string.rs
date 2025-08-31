//! HOCON string and path parsing module.
//!
//! This module provides parsers for HOCON (Human-Optimized Config Object Notation)
//! strings, keys, and paths using the `nom` combinator library. It supports:
//! - Quoted strings with standard and Unicode escape sequences.
//! - Multiline strings enclosed in triple quotes (`"""..."""`).
//! - Unquoted strings and path segments, with validation against forbidden characters.
//! - Parsing of HOCON paths, which may consist of multiple segments separated by dots (`.`).
//! - Assembly of parsed fragments into [`RawString`] values, handling concatenation and spacing.
//!
//! The module defines the following key components:
//! - `StringFragment`: Represents a fragment of a quoted string (literal, escaped char, or escaped whitespace).
//! - `Path`: Represents a path segment (quoted, unquoted, or multiline).
//! - `FORBIDDEN_CHARACTERS`: Characters that are not allowed in unquoted strings or keys.
//! - A collection of parser functions like `parse_quoted_string`, `parse_unquoted_string`,
//!   `parse_multiline_string`, `parse_path`, `parse_key`, and `parse_string`.
//!
//! These parsers return results as `R<'_, T>` (`nom::IResult` specialized with `HoconParseError`),
//! allowing precise error handling and composition with other parsers in the HOCON parser crate.

use crate::parser::{
    R, hocon_horizontal_space0, horizontal_ending, is_hocon_horizontal_whitespace,
    is_hocon_whitespace,
};
use crate::raw::raw_string::{ConcatString, RawString};
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while, take_while_m_n};
use nom::character::char;
use nom::character::complete::anychar;
use nom::combinator::{map, map_opt, not, opt, peek, value, verify};
use nom::multi::{fold, many1, separated_list1};
use nom::sequence::{delimited, preceded};
use std::ops::{Deref, DerefMut};

/// Characters that are forbidden in unquoted strings and keys in HOCON.
///
/// According to the HOCON specification, these characters cannot appear in
/// unquoted values or keys because they have special syntactic meaning
/// (delimiters, operators, or reserved symbols).
const FORBIDDEN_CHARACTERS: [char; 19] = [
    '$', '"', '{', '}', '[', ']', ':', '=', ',', '+', '#', '`', '^', '?', '!', '@', '*', '&', '\\',
];

/// Represents a fragment of a string inside a quoted HOCON string.
///
/// A string may be composed of different fragments (e.g., literals, escaped
/// characters), which are parsed and then combined
/// into the final string value.
#[derive(Debug, Copy, Clone)]
enum StringFragment<'a> {
    /// A literal slice of the string (no escaping).
    Literal(&'a str),
    /// A single escaped character (e.g., `\n`, `\t`, `\"`).
    EscapedChar(char),
}

/// Represents different forms of HOCON path segments.
///
/// HOCON paths can be written in different syntactic forms:
/// - Quoted strings (e.g., `"foo"`)
/// - Unquoted strings (e.g., `foo`)
/// - Multiline strings (enclosed in triple quotes)
#[derive(Debug, Clone)]
enum Path {
    Quoted(String),
    Unquoted(String),
    Multiline(String),
}

impl Deref for Path {
    type Target = String;

    /// Returns a shared reference to the inner `String`.
    fn deref(&self) -> &Self::Target {
        match self {
            Path::Quoted(s) | Path::Unquoted(s) | Path::Multiline(s) => s,
        }
    }
}

impl DerefMut for Path {
    /// Returns a mutable reference to the inner `String`.
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Path::Quoted(s) | Path::Unquoted(s) | Path::Multiline(s) => s,
        }
    }
}

/// Predicate function: checks whether a character is neither a double quote (`"`) nor a backslash (`\`).
///
/// # Parameters
/// * `c` - The character to check.
///
/// # Returns
/// Returns `true` if the character is not a double quote and not a backslash.
fn not_quote_slash(c: char) -> bool {
    c != '"' && c != '\\'
}

/// Parses escape sequences inside HOCON quoted strings.
///
/// Supported escapes include:
/// - Standard JSON escapes: `\b`, `\t`, `\n`, `\f`, `\r`, `\"`, `\\`, `\/`
/// - Unicode escapes: `\uXXXX` or `\UXXXXXXXX` (4–8 hex digits)
/// - Any other escaped character is treated literally (e.g., `\$`, `\#`, `\=`).
///
/// # Parameters
/// - `input`: The input string slice starting with a backslash escape.
///
/// # Returns
/// An [`R<char>`] containing the remaining input and the parsed character on success,
/// or an error if the escape sequence is invalid.
fn parse_escaped_char(input: &str) -> R<'_, char> {
    preceded(
        char('\\'),
        alt((
            value('\u{0008}', char('b')), // Backspace
            value('\u{0009}', char('t')), // Tab
            value('\u{000A}', char('n')), // Line feed
            value('\u{000C}', char('f')), // Form feed
            value('\u{000D}', char('r')), // Carriage return
            value('\"', char('"')),       // Double quote
            value('\\', char('\\')),      // Backslash
            value('/', char('/')),        // Solidus
            parse_unicode_escape,         // Unicode escape (\uXXXX or \UXXXXXXXX)
            anychar,                      // Literal escape (fallback)
        )),
    )
    .parse_complete(input)
}

/// Parses a Unicode escape sequence (`\uXXXX` or `\UXXXXXXXX`) inside a HOCON quoted string.
///
/// The sequence must be 4–8 hexadecimal digits. Surrogate code points (0xD800–0xDFFF)
/// are rejected as invalid.
///
/// # Parameters
/// - `input`: The input string slice starting after the `u` or `U` marker.
///
/// # Returns
/// An [`R<char>`] containing the remaining input and the parsed Unicode character on success,
/// or an error if the sequence is invalid.
fn parse_unicode_escape(input: &str) -> R<'_, char> {
    preceded(
        alt((tag("u"), tag("U"))),
        map_opt(
            take_while_m_n(4, 8, |c: char| c.is_ascii_hexdigit()),
            |hex_str| {
                let code_point = u32::from_str_radix(hex_str, 16).ok()?;
                if (0xD800..=0xDFFF).contains(&code_point) {
                    None // surrogate code points are invalid
                } else {
                    char::from_u32(code_point)
                }
            },
        ),
    )
    .parse_complete(input)
}

/// Parses a sequence of characters that are neither a double quote (`"`) nor a backslash (`\`).
///
/// The result is verified to be non-empty.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<&str>` containing the remaining input and the parsed literal slice on success,
/// or an error on failure.
fn parse_literal(input: &str) -> R<'_, &str> {
    verify(take_while(not_quote_slash), |s: &str| !s.is_empty()).parse_complete(input)
}

/// Parses a fragment of a quoted HOCON string and wraps it into a [`StringFragment`].
///
/// Supported fragments:
/// - Literal characters
/// - Escaped characters
/// - Escaped whitespace
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<StringFragment>` containing the remaining input and the parsed fragment on success,
/// or an error on failure.
fn parse_fragment(input: &str) -> R<'_, StringFragment<'_>> {
    alt((
        map(parse_literal, StringFragment::Literal),
        map(parse_escaped_char, StringFragment::EscapedChar),
    ))
    .parse_complete(input)
}

/// Parses a complete quoted string in HOCON, enclosed in double quotes (`"`).
///
/// The parser assembles internal fragments into a single `String`.
/// It handles literal characters, escaped characters, and escaped whitespace.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<String>` containing the remaining input and the parsed string on success,
/// or an error on failure.
pub(crate) fn parse_quoted_string(input: &str) -> R<'_, String> {
    let build_string = fold(0.., parse_fragment, String::new, |mut string, fragment| {
        match fragment {
            StringFragment::Literal(s) => string.push_str(s),
            StringFragment::EscapedChar(c) => string.push(c),
        }
        string
    });

    delimited(char('"'), build_string, char('"')).parse_complete(input)
}

/// Predicate function: checks if a character is forbidden in an unquoted string.
///
/// Forbidden characters include reserved symbols and whitespace.
///
/// # Parameters
/// * `c` - The character to check.
///
/// # Returns
/// Returns `true` if the character is forbidden, otherwise `false`.
fn is_forbidden_unquoted_char(c: char) -> bool {
    FORBIDDEN_CHARACTERS.contains(&c) || is_hocon_whitespace(c)
}

/// Parses a single valid character in an unquoted HOCON string.
///
/// Ensures the character is not forbidden or whitespace, and allows
/// a forward slash only if it does not start a line comment (`//`).
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<char>` containing the remaining input and the parsed character on success,
/// or an error on failure.
fn parse_unquoted_char(input: &str) -> R<'_, char> {
    alt((
        verify(anychar, |&c| !is_forbidden_unquoted_char(c) && c != '/'),
        (char('/'), peek(not(char('/')))).map(|_| '/'),
    ))
    .parse_complete(input)
}

/// Parses a single valid character in an unquoted HOCON path segment.
///
/// Ensures the character is not forbidden, not whitespace, and not a dot (`.`).
/// Allows a forward slash only if it does not start a line comment (`//`).
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<char>` containing the remaining input and the parsed character on success,
/// or an error on failure.
fn parse_unquoted_path_char(input: &str) -> R<'_, char> {
    alt((
        verify(anychar, |c| {
            !FORBIDDEN_CHARACTERS.contains(&c) && *c != '/' && *c != '.' && *c != '\r' && *c != '\n'
        }),
        (char('/'), peek(not(char('/')))).map(|_| '/'),
    ))
    .parse_complete(input)
}

/// Parses an unquoted HOCON string consisting of one or more valid characters.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<String>` containing the remaining input and the parsed string on success,
/// or an error on failure.
pub(crate) fn parse_unquoted_string(input: &str) -> R<'_, String> {
    fold(1.., parse_unquoted_char, String::new, |mut acc, char| {
        acc.push(char);
        acc
    })
    .parse_complete(input)
}

/// Parses an unquoted HOCON path expression (a single path segment).
///
/// Ensures the expression contains at least one non-whitespace character.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<String>` containing the remaining input and the parsed path segment on success,
/// or an error on failure.
fn parse_unquoted_path_expression(input: &str) -> R<'_, String> {
    verify(
        fold(1.., parse_unquoted_path_char, String::new, |mut acc, c| {
            acc.push(c);
            acc
        }),
        |path: &String| path.chars().any(|c| !is_hocon_horizontal_whitespace(c)),
    )
    .parse_complete(input)
}

/// Parses a multiline string enclosed in triple quotes (`"""..."""`).
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<String>` containing the remaining input and the parsed string on success,
/// or an error on failure.
pub(crate) fn parse_multiline_string(input: &str) -> R<'_, String> {
    delimited(tag(r#"""""#), take_until(r#"""""#), tag(r#"""""#))
        .map(|x: &str| x.to_string())
        .parse_complete(input)
}

/// Parses a HOCON path, which is a sequence of path expressions separated by dots (`.`).
///
/// Supports quoted strings, unquoted strings, and multiline strings as path segments.
/// Trims trailing horizontal whitespace from the final path segment.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<Vec<Path>>` containing the remaining input and the parsed path segments on success,
/// or an error on failure.
fn parse_path(input: &str) -> R<'_, Vec<Path>> {
    separated_list1(
        char('.'),
        alt((
            parse_multiline_string.map(Path::Multiline),
            parse_quoted_string.map(Path::Quoted),
            parse_unquoted_path_expression.map(Path::Unquoted),
        )),
    )
    .map(|mut path| {
        path.last_mut().map(|p| {
            // FIXME: Only Unquoted string should be trimmed
            let trimmed_len = p.trim_end_matches(is_hocon_horizontal_whitespace).len();
            p.truncate(trimmed_len);
        });
        path
    })
    .parse_complete(input)
}

/// Parses a HOCON key, which may be a single path or a concatenation of multiple paths.
///
/// Wraps the parsed result into a [`RawString`].
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<RawString>` containing the remaining input and the parsed key on success,
/// or an error on failure.
pub(crate) fn parse_key(input: &str) -> R<'_, RawString> {
    fn path_to_raw(path: Path) -> RawString {
        match path {
            Path::Quoted(s) => RawString::quoted(s),
            Path::Unquoted(s) => RawString::unquoted(s),
            Path::Multiline(s) => RawString::multiline(s),
        }
    }
    parse_path
        .map(|mut paths| {
            if paths.len() == 1 {
                path_to_raw(paths.remove(0))
            } else {
                let mut keys = Vec::with_capacity(paths.len());
                let last_index = paths.len().saturating_sub(1);
                for (index, path) in paths.iter().enumerate() {
                    let dot = if index != last_index { Some(".") } else { None };
                    let key = match path {
                        Path::Quoted(s) => (RawString::quoted(s), dot),
                        Path::Unquoted(s) => (RawString::unquoted(s), dot),
                        Path::Multiline(s) => (RawString::multiline(s), dot),
                    };
                    keys.push(key);
                }
                RawString::concat(keys.into_iter())
            }
        })
        .parse_complete(input)
}

/// Parses a HOCON path expression and wraps it into a [`RawString`].
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<RawString>` containing the remaining input and the parsed path expression on success,
/// or an error on failure.
pub(crate) fn parse_path_expression(input: &str) -> R<'_, RawString> {
    parse_key.parse_complete(input)
}

/// Parses a HOCON string, which may consist of multiple parts (multiline, quoted, or unquoted),
/// optionally separated by horizontal whitespace.
///
/// Assembles the parsed parts into a single [`RawString`].
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<RawString>` containing the remaining input and the parsed string on success,
/// or an error on failure.
pub(crate) fn parse_string(input: &str) -> R<'_, RawString> {
    let (remainder, mut values) = many1((
        alt((
            parse_multiline_string.map(RawString::MultilineString),
            parse_quoted_string.map(RawString::QuotedString),
            parse_unquoted_string.map(RawString::UnquotedString),
        )),
        opt(hocon_horizontal_space0).map(|v| v.map(|v| v.to_string())),
    ))
    .parse_complete(input)?;
    if peek(horizontal_ending).parse_complete(remainder).is_ok() {
        values.last_mut().unwrap().1 = None;
    };
    Ok((remainder, maybe_concat(values)))
}

/// Helper function: concatenates multiple parsed string fragments into a single [`RawString`].
///
/// # Parameters
/// * `values` - A vector of `(RawString, Option<String>)` pairs, where the second element
///   represents optional spacing after each fragment.
///
/// # Returns
/// A concatenated `RawString`. If only one fragment is present, it is returned directly.
fn maybe_concat(mut values: Vec<(RawString, Option<String>)>) -> RawString {
    assert!(!values.is_empty());
    if values.len() == 1 && values[0].1.is_none() {
        values.remove(0).0
    } else {
        RawString::ConcatString(ConcatString::new(values))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::string::{
        FORBIDDEN_CHARACTERS, parse_multiline_string, parse_quoted_string, parse_string,
        parse_unquoted_string,
    };
    #[rstest]
    #[case("abc", "abc", "")]
    #[case("foo123", "foo123", "")]
    #[case("bar-baz", "bar-baz", "")]
    #[case("path.to.value", "path.to.value", "")]
    #[case("UPPER_case", "UPPER_case", "")]
    #[case("foo bar", "foo", " bar")]
    #[case("a/b/c", "a/b/c", "")]
    #[case("a/b//c", "a/b", "//c")]
    #[case("a/b\n", "a/b", "\n")]
    #[case("a/b\r\n", "a/b", "\r\n")]
    fn test_valid_unquoted_string(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse_unquoted_string(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed, expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    fn test_forbidden_char() {
        for ele in FORBIDDEN_CHARACTERS {
            let input = format!("{}abc", ele);
            let result = parse_unquoted_string(&*input);
            assert!(result.is_err(), "expected Ok but got {:?}", result);
        }
    }

    #[rstest]
    #[case("")]
    #[case("//abc")]
    #[case("$123")]
    #[case("\na/b\r\n")]
    fn test_invalid_unquoted_string(#[case] input: &str) {
        let result = parse_unquoted_string(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }

    #[rstest]
    #[case("\"\"", "", "")]
    #[case("\"hello world\"", "hello world", "")]
    #[case("\"abc\r\t\"ccb", "abc\r\t", "ccb")]
    #[case("\"${a.b?}.c\" 123", "${a.b?}.c", " 123")]
    #[case("\"\n\"", "\n", "")]
    #[case("\"\r\n\"", "\r\n", "")]
    #[case("\"a\\\"\t\"", "a\"\t", "")]
    #[case(r#""""#, "", "")]
    #[case(r#""\u4F60\u597D""#, "你好", "")]
    #[case(r#""\b""#, "\u{0008}", "")]
    #[case(r#""\t""#, "\u{0009}", "")]
    #[case(r#""\n""#, "\u{000A}", "")]
    #[case(r#""\f""#, "\u{000C}", "")]
    #[case(r#""\r""#, "\u{000D}", "")]
    #[case(r#""\"""#, "\"", "")]
    #[case(r#""\\""#, "\\", "")]
    #[case(r#""\/""#, "/", "")]
    // Unicode 转义测试
    #[case(r#""\u0041""#, "A", "")] // 基本拉丁字母
    #[case(r#""\U0001F600""#, "😀", "")] // 表情符号
    #[case(r#""\u00E9""#, "é", "")] // 带重音字母
    // 任意字符转义测试
    #[case(r#""\$""#, "$", "")] // 美元符号
    #[case(r#""\#""#, "#", "")] // 井号
    #[case(r#""\=""#, "=", "")] // 等号
    #[case(r#""\ ""#, " ", "")] // 空格
    #[case(r#""\,""#, ",", "")] // 逗号
    fn test_valid_quoted_string(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse_quoted_string(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed, expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case("")]
    #[case("\"")]
    #[case("foo bar")]
    fn test_invalid_quoted_string(#[case] input: &str) {
        let result = parse_quoted_string(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }

    #[rstest]
    #[case(
        r#""""
        Hello,
        World!
        """"#,
        r#"
        Hello,
        World!
        "#,
        ""
    )]
    #[case(
        r#""""
        Hello,""
        World!
        """"#,
        r#"
        Hello,""
        World!
        "#,
        ""
    )]
    #[case(r#"""" Hello,""" World! """"#, r#" Hello,"#, r#" World! """"#)]
    fn test_valid_multiline_string(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse_multiline_string(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed, expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case(r#""foo bar""#)]
    fn test_invalid_multiline_string(#[case] input: &str) {
        let result = parse_multiline_string(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }

    #[rstest]
    #[case("4 5.0", "4 5.0", "")]
    #[case("5.0", "5.0", "")]
    #[case(r#""foo bar"hello"#, "\"foo bar\"hello", "")]
    #[case(r#""""a.""". b." c""#, r#""""a.""". b." c""#, "")]
    #[case("a.b.c", "a.b.c", "")]
    #[case("a.\"b.c\"", r#"a."b.c""#, "")]
    #[case("\"a.b\".c", r#""a.b".c"#, "")]
    #[case(r#""complex: \u0041 \n \t \\ \"""#, "\"complex: A \n \t \\ \"\"", "")]
    #[case("true false", "true false", "")]
    fn test_valid_string(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse_string(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed.synthetic(), expected_result);
        assert_eq!(rest, expected_rest);
    }
}
