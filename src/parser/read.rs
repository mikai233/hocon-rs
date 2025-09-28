use std::fmt::Display;
use std::str;

use derive_more::Constructor;

use crate::Result;
use crate::error::{Error, Parse};
use crate::parser::string::FORBIDDEN_TABLE;

// We should peek at least 11 bytes because the `classpath(` token has a length of 11 bytes.
pub(crate) const MAX_PEEK_N: usize = 11;

pub(crate) const DEFAULT_BUFFER_SIZE: usize = 512;

pub(crate) const DISCARD_ERROR: &str =
    "parser state error: attempted to discard data after successful peek operation";

/// Returns the number of bytes of the first character in `bytes`
/// if it is a whitespace character, otherwise returns 0.
///
/// This function recognizes:
/// - ASCII whitespace and control characters: `\t` (0x09), `\n` (0x0A),
///   vertical tab 0x0B, form feed 0x0C, carriage return `\r` (0x0D), space ` ` (0x20),
///   and the additional control characters U+001C..=U+001F (0x1C..0x1F)
/// - Multi-byte Unicode whitespace:
///     - U+0085 (NEL) → 0xC2 0x85
///     - U+00A0 (NO-BREAK SPACE) → 0xC2 0xA0
///     - U+1680 (OGHAM SPACE MARK) → 0xE1 0x9A 0x80
///     - U+2000..U+200A (EN QUAD..HAIR SPACE) → 0xE2 0x80 0x80..0x8A
///     - U+2028, U+2029 (LINE SEPARATOR, PARAGRAPH SEPARATOR) → 0xE2 0x80 0xA8..0xA9
///     - U+202F (NARROW NO-BREAK SPACE) → 0xE2 0x80 0xAF
///     - U+205F (MEDIUM MATHEMATICAL SPACE) → 0xE2 0x81 0x9F
///     - U+3000 (IDEOGRAPHIC SPACE) → 0xE3 0x80 0x80
///
/// # Returns
///
/// - The number of bytes of the first character if it is a recognized whitespace
/// - 0 if the first character is not whitespace or the slice is empty
///
/// # Note
///
/// This function only examines the first character and does **not** count
/// consecutive whitespace.
#[inline(always)]
pub fn leading_whitespace_bytes(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    match bytes {
        // ASCII whitespace + extra control characters U+001C..=U+001F
        [b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ' | 0x1C..=0x1F, ..] => 1,

        // U+0085 (NEL)
        [0xC2, 0x85, ..] => 2,

        // U+00A0 (NO-BREAK SPACE)
        [0xC2, 0xA0, ..] => 2,

        // U+1680 (OGHAM SPACE MARK)
        [0xE1, 0x9A, 0x80, ..] => 3,

        // U+2000..U+200A
        [0xE2, 0x80, 0x80..=0x8A, ..] => 3,

        // U+2028, U+2029
        [0xE2, 0x80, 0xA8..=0xA9, ..] => 3,

        // U+202F
        [0xE2, 0x80, 0xAF, ..] => 3,

        // U+205F
        [0xE2, 0x81, 0x9F, ..] => 3,

        // U+3000
        [0xE3, 0x80, 0x80, ..] => 3,

        _ => 0, // first character is not whitespace
    }
}

fn parse_escaped_char<'de, R: Read<'de>>(reader: &mut R, scratch: &mut Vec<u8>) -> Result<()> {
    let ch = reader.next()?;
    match ch {
        b'"' => scratch.push(b'"'),
        b'\\' => scratch.push(b'\\'),
        b'/' => scratch.push(b'/'),
        b'b' => scratch.push(b'\x08'),
        b'f' => scratch.push(b'\x0c'),
        b'n' => scratch.push(b'\n'),
        b'r' => scratch.push(b'\r'),
        b't' => scratch.push(b'\t'),
        b'u' => parse_escaped_unicode(reader, scratch)?,
        _ => {
            return Err(reader.error(Parse::InvalidEscape));
        }
    }
    Ok(())
}

/// Parses a Unicode escape sequence of the form `\uXXXX` (and possibly a surrogate pair).
///
/// This function reads exactly 4 hexadecimal digits after `\u` and converts them into
/// a Unicode code point. If the code point is in the high-surrogate range (`0xD800..=0xDBFF`),
/// it expects another `\uXXXX` low surrogate (`0xDC00..=0xDFFF`) to follow and combines
/// them into a single supplementary character.
///
/// The resulting Unicode scalar value is then encoded as UTF-8 and appended to `scratch`.
///
/// # Arguments
/// * `reader`  - The input reader providing bytes (typically HOCON/JSON parser input).
/// * `scratch` - A temporary buffer to which the decoded UTF-8 bytes are appended.
///
/// # Errors
/// Returns `Error::InvalidEscape` if:
/// - the escape sequence is malformed,
/// - contains invalid hex digits,
/// - contains an unpaired surrogate,
/// - or produces an invalid Unicode code point.
///
/// # Safety
/// This implementation uses `char::from_u32` + `encode_utf8` to guarantee that only valid
/// Unicode scalar values are emitted, avoiding panics or undefined behavior.
///
/// # Example
/// ```ignore
/// // parsing "\u0041" should append 'A'
/// let mut buf = Vec::new();
/// let mut input = SliceReader::new(br"0041"); // hypothetical reader
/// parse_escaped_unicode(&mut input, &mut buf).unwrap();
/// assert_eq!(buf, b"A");
/// ```
fn parse_escaped_unicode<'de, R: Read<'de>>(reader: &mut R, scratch: &mut Vec<u8>) -> Result<()> {
    /// Reads 4 hexadecimal digits (`\uXXXX`) and returns a `u16`.
    fn parse_hex16<'de, R: Read<'de>>(reader: &mut R) -> Result<u16> {
        let mut n: u16 = 0;
        for _ in 0..4 {
            let b = reader.next()?;
            n = match b {
                b'0'..=b'9' => (n << 4) | (b - b'0') as u16,
                b'a'..=b'f' => (n << 4) | (10 + b - b'a') as u16,
                b'A'..=b'F' => (n << 4) | (10 + b - b'A') as u16,
                _ => {
                    return Err(reader.error(Parse::InvalidEscape));
                }
            };
        }
        Ok(n)
    }

    // Parse first 4 hex digits
    let mut n = parse_hex16(reader)? as u32;

    // Handle surrogate pair (UTF-16 encoding for supplementary characters)
    if (0xD800..=0xDBFF).contains(&n) {
        // Expect `\u` for the low surrogate
        if reader.next()? != b'\\' || reader.next()? != b'u' {
            return Err(reader.error(Parse::InvalidEscape));
        }
        let n2 = parse_hex16(reader)? as u32;
        if !(0xDC00..=0xDFFF).contains(&n2) {
            return Err(reader.error(Parse::InvalidEscape));
        }
        // Combine surrogate pair into a single code point
        n = 0x10000 + (((n - 0xD800) << 10) | (n2 - 0xDC00));
    }

    // Convert to `char` and encode as UTF-8
    if let Some(ch) = char::from_u32(n) {
        let mut buf = [0u8; 4];
        scratch.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        Ok(())
    } else {
        Err(reader.error(Parse::InvalidEscape))
    }
}

#[inline(always)]
fn as_str<'de, 's, R>(reader: &R, slice: &'s [u8]) -> Result<&'s str>
where
    R: Read<'de>,
{
    str::from_utf8(slice).map_err(|_| reader.error(Parse::InvalidUtf8))
}

macro_rules! next_position {
    ($self:expr, $byte:expr) => {{
        if $byte == b'\n' {
            $self.line += 1;
            $self.column = 1;
        } else {
            $self.column += 1;
        }
    }};
}

macro_rules! peek_position {
    ($line:expr, $column:expr, $byte:expr) => {{
        if $byte == b'\n' {
            $line += 1;
            $column = 1;
        } else {
            $column += 1;
        }
    }};
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Constructor)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line: {}, column: {}", self.line, self.column)
    }
}

pub enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

impl<'b, 'c, T> std::ops::Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

pub trait Read<'de> {
    fn position(&self) -> Position;

    fn peek_position(&mut self) -> Position;

    fn peek_n(&mut self, n: usize) -> Result<&[u8]>;

    #[inline(always)]
    fn peek(&mut self) -> Result<u8> {
        let chars = self.peek_n(1)?;
        Ok(chars[0])
    }

    fn next(&mut self) -> Result<u8>;

    #[inline(always)]
    fn discard(&mut self, n: usize) {
        for _ in 0..n {
            self.next().expect(DISCARD_ERROR);
        }
    }

    fn parse_quoted_str<'s>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>>;

    fn parse_multiline_str<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>>;

    fn parse_unquoted_str<'s>(
        &mut self,
        scratch: &'s mut Vec<u8>,
        allow_dot: bool,
    ) -> Result<Reference<'de, 's, str>>;

    fn parse_to_line_ending<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>>;

    #[inline(always)]
    fn peek_whitespace(&mut self) -> Result<usize> {
        for n in (1..=3).rev() {
            let n = match self.peek_n(n) {
                Ok(bytes) => leading_whitespace_bytes(bytes),
                Err(Error::Eof) => {
                    continue;
                }
                Err(err) => return Err(err),
            };
            return Ok(n);
        }
        Ok(0)
    }

    #[inline(always)]
    fn starts_with_whitespace(&mut self) -> Result<bool> {
        self.peek_whitespace().map(|n| n > 0)
    }

    #[inline(always)]
    fn peek_horizontal_whitespace(&mut self) -> Result<usize> {
        if self.peek()? != b'\n' {
            self.peek_whitespace()
        } else {
            Ok(0)
        }
    }

    #[inline(always)]
    fn starts_with_horizontal_whitespace(&mut self) -> Result<bool> {
        self.peek_horizontal_whitespace().map(|n| n > 0)
    }

    #[inline(always)]
    fn error(&self, error: Parse) -> Error {
        Error::Parse {
            parse: error,
            position: self.position(),
        }
    }

    #[inline(always)]
    fn peek_error(&mut self, error: Parse) -> Error {
        Error::Parse {
            parse: error,
            position: self.peek_position(),
        }
    }
}

pub struct StreamRead<R: std::io::Read> {
    inner: R,
    buffer: [u8; DEFAULT_BUFFER_SIZE],
    head: usize,
    tail: usize,
    eof: bool,
    line: usize,
    column: usize,
}

impl<R: std::io::Read> StreamRead<R> {
    pub fn new(reader: R) -> Self {
        StreamRead {
            inner: reader,
            buffer: [0u8; _],
            head: 0,
            tail: 0,
            eof: false,
            line: 1,
            column: 1,
        }
    }

    fn fill_buf(&mut self) -> Result<()> {
        if self.eof {
            return Err(Error::Eof);
        }

        // 如果 buffer 已经满了，就不能再读
        if self.tail == self.buffer.len() {
            return Ok(());
        }

        let empty_buf = &mut self.buffer[self.tail..];
        let n = self.inner.read(empty_buf)?;
        if n == 0 {
            self.eof = true;
        }
        self.tail += n;
        Ok(())
    }

    #[inline(always)]
    fn available_data_len(&self) -> usize {
        self.tail - self.head
    }
}

impl<'de, R: std::io::Read> Read<'de> for StreamRead<R> {
    #[inline]
    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
        }
    }

    #[inline(always)]
    fn peek_position(&mut self) -> Position {
        let (mut line, mut column) = (self.line, self.column);
        if let Ok(byte) = self.peek() {
            peek_position!(line, column, byte);
        }
        Position { line, column }
    }

    #[inline(always)]
    fn peek_n(&mut self, n: usize) -> Result<&[u8]> {
        debug_assert!(n > 0 && n <= MAX_PEEK_N);

        if self.available_data_len() < n && !self.eof {
            // 如果 buffer 已经写满但数据不够 -> 搬移一下
            if self.tail == self.buffer.len() && self.head > 0 {
                let len = self.tail - self.head;
                self.buffer.copy_within(self.head..self.tail, 0);
                self.head = 0;
                self.tail = len;
            }
            self.fill_buf()?;
        }
        if self.available_data_len() < n {
            Err(Error::Eof)
        } else {
            Ok(&self.buffer[self.head..self.head + n])
        }
    }

    #[inline(always)]
    fn next(&mut self) -> Result<u8> {
        let data_len = self.available_data_len();
        if data_len == 0 && !self.eof {
            self.fill_buf()?;
        } else if data_len == 0 && self.eof {
            return Err(Error::Eof);
        }
        let byte = self.buffer[self.head];
        next_position!(self, byte);
        self.head += 1;
        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
        }
        Ok(byte)
    }

    fn parse_quoted_str<'s>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        loop {
            match self.next() {
                Ok(b'\\') if escape => parse_escaped_char(self, scratch)?,
                Ok(b'"') => {
                    break;
                }
                Ok(byte) => {
                    scratch.push(byte);
                }
                Err(_) => {
                    return Err(self.error(Parse::Expected("\"")));
                }
            }
        }
        as_str(self, scratch).map(Reference::Copied)
    }

    fn parse_multiline_str<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        loop {
            match self.next() {
                Ok(b'"') => match self.peek_n(2) {
                    Ok(bytes) if bytes == b"\"\"" => {
                        self.discard(2);
                        break;
                    }
                    Ok(_) => {
                        scratch.push(b'\"');
                    }
                    Err(_) => {
                        return Err(self.error(Parse::Expected("\"\"")));
                    }
                },
                Ok(byte) => {
                    scratch.push(byte);
                }
                Err(_) => {
                    return Err(self.error(Parse::Expected("\"\"\"")));
                }
            }
        }
        as_str(self, scratch).map(Reference::Copied)
    }

    fn parse_unquoted_str<'s>(
        &mut self,
        scratch: &'s mut Vec<u8>,
        allow_dot: bool,
    ) -> Result<Reference<'de, 's, str>> {
        loop {
            match self.peek() {
                Ok(b'/') => match self.peek_n(2) {
                    Ok(bytes) if bytes == b"//" => break,
                    Ok(_) | Err(Error::Eof) => {
                        self.discard(1);
                        scratch.push(b'/');
                    }
                    Err(err) => return Err(err),
                },
                Ok(b'.') if !allow_dot => break,
                Ok(byte) => {
                    if FORBIDDEN_TABLE[byte as usize] || self.starts_with_whitespace()? {
                        break;
                    } else {
                        self.discard(1);
                        scratch.push(byte);
                    }
                }
                Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        as_str(self, scratch).map(Reference::Copied)
    }

    fn parse_to_line_ending<'s>(
        &'s mut self,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        loop {
            match self.peek() {
                Ok(b'\n') => {
                    break;
                }
                Ok(b'\r') => match self.peek_n(2) {
                    Ok(bytes) if bytes == b"\r\n" => {
                        break;
                    }
                    Ok(_) | Err(Error::Eof) => {
                        self.discard(1);
                        scratch.push(b'\r');
                    }
                    Err(err) => return Err(err),
                },
                Ok(byte) => {
                    self.discard(1);
                    scratch.push(byte);
                }
                Err(Error::Eof) => break,
                Err(err) => return Err(err),
            }
        }
        as_str(self, scratch).map(Reference::Copied)
    }
}

pub struct SliceRead<'de> {
    slice: &'de [u8],
    index: usize,
    line: usize,
    column: usize,
}

impl<'de> SliceRead<'de> {
    pub fn new(slice: &'de [u8]) -> Self {
        SliceRead {
            slice,
            index: 0,
            line: 1,
            column: 1,
        }
    }

    #[inline(always)]
    fn available_data_len(&self) -> usize {
        self.slice.len() - self.index
    }

    pub(crate) fn rest(&self) -> &[u8] {
        &self.slice[self.index..]
    }

    fn parse_quoted_str_bytes<'s, F, T>(
        &mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        result: F,
    ) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'a> FnOnce(&Self, &'a [u8]) -> Result<&'a T>,
    {
        let mut start = self.index;
        let len_before = scratch.len();
        loop {
            match self.slice.get(self.index) {
                Some(b'\\') if escape => {
                    scratch.extend_from_slice(&self.slice[start..self.index]);
                    self.index += 1;
                    parse_escaped_char(self, scratch)?;
                    start = self.index;
                }
                Some(b'"') => break,
                Some(_) => {
                    self.index += 1;
                }
                None => {
                    return Err(self.error(Parse::Expected("\"")));
                }
            }
        }
        let result = if len_before == scratch.len() {
            let borrowed = &self.slice[start..self.index];
            result(self, borrowed).map(Reference::Borrowed)
        } else {
            scratch.extend_from_slice(&self.slice[start..self.index]);
            result(self, scratch).map(Reference::Copied)
        };
        self.column += 1;
        self.index += 1;
        result
    }

    fn parse_multiline_str_bytes<'s, F, T>(&mut self, result: F) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'a> FnOnce(&Self, &'a [u8]) -> Result<&'a T>,
    {
        let start = self.index;
        loop {
            match self.slice.get(self.index) {
                Some(b'"') => match self.slice.get(self.index + 1..=self.index + 2) {
                    Some(bytes) if bytes == b"\"\"" => {
                        break;
                    }
                    Some(_) => {
                        self.column += 1;
                        self.index += 1;
                    }
                    None => {
                        return Err(self.error(Parse::Expected("\"\"")));
                    }
                },
                Some(byte) => {
                    next_position!(self, *byte);
                    self.index += 1;
                }
                None => {
                    return Err(self.error(Parse::Expected("\"\"\"")));
                }
            }
        }
        let borrowed = &self.slice[start..self.index];
        let result = result(self, borrowed).map(Reference::Borrowed);
        self.column += 3;
        self.index += 3;
        result
    }

    fn parse_unquoted_str_bytes<'s, F, T>(
        &mut self,
        allow_dot: bool,
        result: F,
    ) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'a> FnOnce(&Self, &'a [u8]) -> Result<&'a T>,
    {
        let start = self.index;
        loop {
            match self.slice.get(self.index) {
                Some(b'/') => match self.slice.get(self.index..=self.index + 1) {
                    Some(bytes) if bytes == b"//" => break,
                    Some(_) | None => {
                        self.column += 1;
                        self.index += 1;
                    }
                },
                Some(b'.') if !allow_dot => break,
                Some(byte) => {
                    if FORBIDDEN_TABLE[*byte as usize] || self.starts_with_whitespace()? {
                        break;
                    } else {
                        next_position!(self, *byte);
                        self.index += 1;
                    }
                }
                None => break,
            }
        }
        let borrowed = &self.slice[start..self.index];
        result(self, borrowed).map(Reference::Borrowed)
    }

    fn parse_to_line_ending_bytes<'s, F, T>(
        &'s mut self,
        result: F,
    ) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        F: for<'a> FnOnce(&Self, &'a [u8]) -> Result<&'a T>,
    {
        let start = self.index;
        loop {
            match self.slice.get(self.index) {
                Some(b'\n') => {
                    break;
                }
                Some(b'\r') => match self.slice.get(self.index..=self.index + 1) {
                    Some(bytes) if bytes == b"\r\n" => {
                        break;
                    }
                    Some(_) | None => {
                        self.column += 1;
                        self.index += 1;
                    }
                },
                Some(byte) => {
                    next_position!(self, *byte);
                    self.index += 1;
                }
                None => break,
            }
        }
        let borrowed = &self.slice[start..self.index];
        result(self, borrowed).map(Reference::Borrowed)
    }
}

impl<'de> Read<'de> for SliceRead<'de> {
    #[inline(always)]
    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
        }
    }

    #[inline(always)]
    fn peek_position(&mut self) -> Position {
        let (mut line, mut column) = (self.line, self.column);
        if let Some(byte) = self.slice.get(self.index + 1) {
            peek_position!(line, column, *byte);
        }
        Position { line, column }
    }

    #[inline(always)]
    fn peek_n(&mut self, n: usize) -> Result<&[u8]> {
        debug_assert!(n > 0 && n <= MAX_PEEK_N);
        if self.available_data_len() < n {
            Err(Error::Eof)
        } else {
            Ok(&self.slice[self.index..self.index + n])
        }
    }

    #[inline(always)]
    fn next(&mut self) -> Result<u8> {
        if self.index == self.slice.len() {
            return Err(Error::Eof);
        }
        let byte = self.slice[self.index];
        next_position!(self, byte);
        self.index += 1;
        Ok(byte)
    }

    #[inline(always)]
    fn discard(&mut self, n: usize) {
        if self.available_data_len() < n {
            panic!("{}", DISCARD_ERROR)
        } else {
            for byte in &self.slice[self.index..] {
                next_position!(self, *byte);
            }
            self.index += n;
        }
    }

    fn parse_quoted_str<'s>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.parse_quoted_str_bytes(escape, scratch, as_str)
    }

    fn parse_multiline_str<'s>(
        &'s mut self,
        _scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.parse_multiline_str_bytes(as_str)
    }

    fn parse_unquoted_str<'s>(
        &mut self,
        _scratch: &'s mut Vec<u8>,
        allow_dot: bool,
    ) -> Result<Reference<'de, 's, str>> {
        self.parse_unquoted_str_bytes(allow_dot, as_str)
    }

    fn parse_to_line_ending<'s>(
        &'s mut self,
        _scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.parse_to_line_ending_bytes(as_str)
    }
}

pub struct StrRead<'de> {
    delegate: SliceRead<'de>,
}

impl<'de> StrRead<'de> {
    pub fn new(s: &'de str) -> Self {
        Self {
            delegate: SliceRead::new(s.as_bytes()),
        }
    }

    pub fn rest(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.delegate.rest()) }
    }
}

impl<'de> Read<'de> for StrRead<'de> {
    #[inline(always)]
    fn position(&self) -> Position {
        self.delegate.position()
    }

    #[inline(always)]
    fn peek_position(&mut self) -> Position {
        self.delegate.peek_position()
    }

    #[inline(always)]
    fn peek_n(&mut self, n: usize) -> Result<&[u8]> {
        self.delegate.peek_n(n)
    }

    #[inline(always)]
    fn next(&mut self) -> Result<u8> {
        self.delegate.next()
    }

    fn parse_quoted_str<'s>(
        &mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.delegate
            .parse_quoted_str_bytes(escape, scratch, |_, bytes| {
                Ok(unsafe { str::from_utf8_unchecked(bytes) })
            })
    }

    fn parse_multiline_str<'s>(
        &mut self,
        _scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.delegate
            .parse_multiline_str_bytes(|_, bytes| Ok(unsafe { str::from_utf8_unchecked(bytes) }))
    }

    fn parse_unquoted_str<'s>(
        &mut self,
        _scratch: &'s mut Vec<u8>,
        allow_dot: bool,
    ) -> Result<Reference<'de, 's, str>> {
        self.delegate
            .parse_unquoted_str_bytes(allow_dot, |_, bytes| {
                Ok(unsafe { str::from_utf8_unchecked(bytes) })
            })
    }

    fn parse_to_line_ending<'s>(
        &'s mut self,
        _scratch: &'s mut Vec<u8>,
    ) -> Result<Reference<'de, 's, str>> {
        self.delegate
            .parse_to_line_ending_bytes(|_, bytes| Ok(unsafe { str::from_utf8_unchecked(bytes) }))
    }
}

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::parser::read::{Read, StreamRead};

    #[test]
    fn test_stream_peek() -> Result<()> {
        let input = "hello world";
        let mut read = StreamRead::new(input.as_bytes());
        let byte = read.peek()?;
        assert_eq!(byte, b'h');
        let bytes = read.peek_n(2)?;
        assert_eq!(bytes, b"he");
        let bytes = read.peek_n(3)?;
        assert_eq!(bytes, b"hel");
        read.discard(3);
        let byte = read.peek()?;
        assert_eq!(byte, b'l');
        let bytes = read.peek_n(2)?;
        assert_eq!(bytes, b"lo");
        let bytes = read.peek_n(3)?;
        assert_eq!(bytes, b"lo ");
        Ok(())
    }
}
