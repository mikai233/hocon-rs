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
    R, hocon_horizontal_multi_space0, is_hocon_horizontal_whitespace, is_hocon_whitespace,
};
use crate::raw::raw_string::{ConcatString, RawString};
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until, take_while, take_while_m_n};
use nom::character::char;
use nom::character::complete::{anychar, multispace1};
use nom::combinator::{map, map_opt, map_res, not, peek, value, verify};
use nom::multi::{fold, many1, separated_list1};
use nom::sequence::{delimited, preceded, terminated};
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
/// characters, or escaped whitespace), which are parsed and then combined
/// into the final string value.
#[derive(Debug, Copy, Clone)]
enum StringFragment<'a> {
    /// A literal slice of the string (no escaping).
    Literal(&'a str),
    /// A single escaped character (e.g., `\n`, `\t`, `\"`).
    EscapedChar(char),
    /// Escaped whitespace (ignored in the final string).
    EscapedWS,
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

/// Parses a `\u{XXXX}` Unicode escape sequence, where `XXXX` is 1 to 6 hexadecimal digits.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<char>` containing the remaining input and the parsed Unicode character on success,
/// or an error on failure.
fn parse_unicode(input: &str) -> R<'_, char> {
    preceded(
        char('\\'),
        map_opt(
            map_res(
                preceded(
                    char('{'),
                    terminated(
                        take_while_m_n(1, 6, |c: char| c.is_ascii_hexdigit()),
                        char('}'),
                    ),
                ),
                |hex| u32::from_str_radix(hex, 16),
            ),
            std::char::from_u32,
        ),
    )
    .parse_complete(input)
}

/// Parses standard JSON escape sequences such as `\n`, `\t`, `\"`, `\\`, `\/`, `\b`, `\f`,
/// and integrates with [`parse_unicode`] for `\u{XXXX}` sequences.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<char>` containing the remaining input and the parsed character on success,
/// or an error on failure.
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

/// Parses a sequence consisting of a backslash followed by one or more whitespace characters.
///
/// # Parameters
/// * `input` - The input string slice to parse.
///
/// # Returns
/// An `R<&str>` containing the remaining input and the parsed whitespace slice on success,
/// or an error on failure.
fn parse_escaped_whitespace(input: &str) -> R<'_, &str> {
    preceded(char('\\'), multispace1).parse_complete(input)
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
        value(StringFragment::EscapedWS, parse_escaped_whitespace),
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
            StringFragment::EscapedWS => {}
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
            !FORBIDDEN_CHARACTERS.contains(&c) && *c != '/' && *c != '.'
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
                for path in paths {
                    let key = match path {
                        Path::Quoted(s) => (RawString::quoted(s), Some(".")),
                        Path::Unquoted(s) => (RawString::unquoted(s), Some(".")),
                        Path::Multiline(s) => (RawString::multiline(s), Some(".")),
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
    many1((
        alt((
            parse_multiline_string.map(RawString::MultilineString),
            parse_quoted_string.map(RawString::QuotedString),
            parse_unquoted_string.map(RawString::UnquotedString),
        )),
        hocon_horizontal_multi_space0.map(|s| {
            if s.len() == 0 {
                None
            } else {
                Some(s.to_string())
            }
        }),
    ))
    .map(maybe_concat)
    .parse_complete(input)
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
    if values.len() == 1 {
        values.remove(0).0
    } else {
        RawString::ConcatString(ConcatString::new(values))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::string::{
        parse_multiline_string, parse_quoted_string, parse_string, parse_unquoted_string,
    };
    use crate::parser::substitution::parse_substitution;

    #[test]
    fn test_unquoted_string() {
        let (r, o) = parse_unquoted_string("4  5.0").unwrap();
        assert_eq!(r, "  5.0");
        assert_eq!(o, "4");
    }

    #[test]
    fn test_quoted_string() {
        parse_quoted_string("\"\"").unwrap();
        let (remainder, result) = parse_quoted_string("\"world\"").unwrap();
        assert_eq!(remainder, "");
        assert_eq!(result, "world");
        let (remainder, result) = parse_quoted_string("\"world\"112233").unwrap();
        assert_eq!(remainder, "112233");
        assert_eq!(result, "world");
    }

    #[test]
    fn test_multiline_string() {
        let (r, o) = parse_multiline_string(
            r#""""""Hello
World!""""""#,
        )
        .unwrap();
        assert_eq!(r, r#""""#);
        assert_eq!(o, "\"\"Hello\nWorld!");
    }

    #[test]
    fn test_string() {
        let (r, o) = parse_string("4 5.0").unwrap();
        let (r, o) = parse_string("\"\"\"a.\"\"\". b.\" c\"").unwrap();
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
