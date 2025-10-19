use std::str;

use derive_more::{Deref, DerefMut};

use crate::Result;
use crate::error::Error;

// We should peek at least 7 bytes because the include token has a length of 7 bytes.
pub(crate) const MAX_PEEK_N: usize = 7;

pub(crate) const DEFAULT_BUFFER_SIZE: usize = 512;

/// Return the length in bytes of the leading whitespace character, if any,
/// according to the HOCON specification.
///
/// Whitespace includes:
/// - ASCII whitespace + control separators (U+0009–000D, U+001C–001F, space)
/// - U+0085 (NEL)
/// - U+00A0 (NO-BREAK SPACE)
/// - U+1680 (OGHAM SPACE MARK)
/// - U+2000..=U+200A (EN QUAD..HAIR SPACE, includes U+2007 FIGURE SPACE)
/// - U+2028, U+2029 (line/paragraph separators)
/// - U+202F (NARROW NO-BREAK SPACE)
/// - U+205F (MEDIUM MATHEMATICAL SPACE)
/// - U+3000 (IDEOGRAPHIC SPACE)
/// - U+FEFF (BOM, must be treated as whitespace)
#[inline]
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

        // U+2000..=U+200A (general spaces, includes U+2007 FIGURE SPACE)
        [0xE2, 0x80, 0x80..=0x8A, ..] => 3,

        // U+2028, U+2029 (line/paragraph separator)
        [0xE2, 0x80, 0xA8..=0xA9, ..] => 3,

        // U+202F (NARROW NO-BREAK SPACE)
        [0xE2, 0x80, 0xAF, ..] => 3,

        // U+205F (MEDIUM MATHEMATICAL SPACE)
        [0xE2, 0x81, 0x9F, ..] => 3,

        // U+3000 (IDEOGRAPHIC SPACE)
        [0xE3, 0x80, 0x80, ..] => 3,

        // U+FEFF (BOM)
        [0xEF, 0xBB, 0xBF, ..] => 3,

        _ => 0, // not whitespace
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
        _ => return Err(Error::InvalidEscape),
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
/// ```text
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
                _ => return Err(Error::InvalidEscape),
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
            return Err(Error::InvalidEscape);
        }
        let n2 = parse_hex16(reader)? as u32;
        if !(0xDC00..=0xDFFF).contains(&n2) {
            return Err(Error::InvalidEscape);
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
        Err(Error::InvalidEscape)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line: usize,
    pub column: usize,
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

    fn peek_n(&mut self, n: usize) -> Result<&[u8]>;

    #[inline]
    fn peek(&mut self) -> Result<u8> {
        let chars = self.peek_n(1)?;
        Ok(chars[0])
    }

    #[inline]
    fn peek2(&mut self) -> Result<(u8, u8)> {
        let chars = self.peek_n(2)?;
        Ok((chars[0], chars[1]))
    }

    fn next(&mut self) -> Result<u8>;

    #[inline]
    fn discard(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.next()?;
        }
        Ok(())
    }

    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: F,
    ) -> Result<Reference<'de, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>;

    #[inline]
    fn peek_whitespace(&mut self) -> Result<Option<usize>> {
        let n = match self.peek_n(3) {
            Ok(bytes) => leading_whitespace_bytes(bytes),
            Err(Error::Eof) => match self.peek_n(2) {
                Ok(bytes) => leading_whitespace_bytes(bytes),
                Err(Error::Eof) => match self.peek_n(1) {
                    Ok(bytes) => leading_whitespace_bytes(bytes),
                    Err(err) => {
                        return Err(err);
                    }
                },
                Err(err) => return Err(err),
            },
            Err(err) => return Err(err),
        };
        if n > 0 { Ok(Some(n)) } else { Ok(None) }
    }

    #[inline]
    fn starts_with_whitespace(&mut self) -> Result<bool> {
        self.peek_whitespace().map(|n| n.is_some())
    }

    #[inline]
    fn peek_horizontal_whitespace(&mut self) -> Result<Option<usize>> {
        if self.peek()? != b'\n' {
            self.peek_whitespace()
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn starts_with_horizontal_whitespace(&mut self) -> Result<bool> {
        self.peek_horizontal_whitespace().map(|n| n.is_some())
    }
}

pub struct StreamRead<R: std::io::Read> {
    inner: R,
    buffer: [u8; DEFAULT_BUFFER_SIZE],
    head: usize,
    tail: usize,
    eof: bool,
    line: usize,
    col: usize,
}

impl<R: std::io::Read> StreamRead<R> {
    pub fn new(reader: R) -> Self {
        StreamRead {
            inner: reader,
            buffer: [0u8; _],
            head: 0,
            tail: 0,
            eof: false,
            line: 0,
            col: 0,
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

    #[inline]
    fn available_data_len(&self) -> usize {
        self.tail - self.head
    }
}

impl<'de, R: std::io::Read> Read<'de> for StreamRead<R> {
    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.col,
        }
    }

    #[inline]
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

    #[inline]
    fn next(&mut self) -> Result<u8> {
        if self.available_data_len() == 0 && !self.eof {
            self.fill_buf()?;
        }
        let byte = self.buffer[self.head];
        if byte == b'\n' {
            self.line += 1;
        } else {
            self.col += 1;
        }
        self.head += 1;
        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
        }
        Ok(byte)
    }

    #[inline]
    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: F,
    ) -> Result<Reference<'de, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>,
    {
        loop {
            if !delimiter(self)? {
                match self.next()? {
                    b'\\' if escape => {
                        parse_escaped_char(self, scratch)?;
                    }
                    ch => {
                        scratch.push(ch);
                    }
                }
            } else {
                break;
            }
        }
        str::from_utf8(scratch)
            .map_err(|_| Error::InvalidUtf8)
            .map(Reference::Copied)
    }
}

macro_rules! parse_str_bytes_impl {
    ($self:expr, $escape:expr, $scratch:expr, $delimiter:expr, $result:expr) => {{
        let mut start = $self.index;
        loop {
            if !$delimiter($self)? {
                if $self.index == $self.slice.len() {
                    break;
                }
                match $self.slice[$self.index] {
                    b'\\' if $escape => {
                        $scratch.extend_from_slice(&$self.slice[start..$self.index]);
                        $self.index += 1;
                        parse_escaped_char($self, $scratch)?;
                        start = $self.index;
                    }
                    _ => {
                        $self.index += 1;
                    }
                }
            } else {
                break;
            }
        }
        if $scratch.is_empty() {
            let borrowed = &$self.slice[start..$self.index];
            $result(borrowed).map(Reference::Borrowed)
        } else {
            $scratch.extend_from_slice(&$self.slice[start..$self.index]);
            $result($scratch).map(Reference::Copied)
        }
    }};
}

pub struct SliceRead<'de> {
    slice: &'de [u8],
    index: usize,
}

impl<'de> SliceRead<'de> {
    pub fn new(slice: &'de [u8]) -> Self {
        SliceRead { slice, index: 0 }
    }

    fn position_of_index(&self, i: usize) -> Position {
        let start_of_line = match memchr::memrchr(b'\n', &self.slice[..i]) {
            Some(position) => position + 1,
            None => 0,
        };
        Position {
            line: 1 + memchr::memchr_iter(b'\n', &self.slice[..start_of_line]).count(),
            column: i - start_of_line,
        }
    }

    #[inline]
    fn available_data_len(&self) -> usize {
        self.slice.len() - self.index
    }

    pub(crate) fn rest(&self) -> &[u8] {
        &self.slice[self.index..]
    }

    #[inline]
    fn parse_str_bytes<'s, E, T, R>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: E,
        result: R,
    ) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        E: Fn(&mut Self) -> Result<bool>,
        R: for<'f> FnOnce(&'f [u8]) -> Result<&'f T>,
    {
        parse_str_bytes_impl!(self, escape, scratch, delimiter, result)
    }
}

impl<'de> Read<'de> for SliceRead<'de> {
    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    #[inline]
    fn peek_n(&mut self, n: usize) -> Result<&[u8]> {
        debug_assert!(n > 0 && n <= MAX_PEEK_N);
        if self.available_data_len() < n {
            Err(Error::Eof)
        } else {
            Ok(&self.slice[self.index..self.index + n])
        }
    }

    #[inline]
    fn next(&mut self) -> Result<u8> {
        if self.index == self.slice.len() {
            return Err(Error::Eof);
        }
        let byte = self.slice[self.index];
        self.index += 1;
        Ok(byte)
    }

    fn discard(&mut self, n: usize) -> Result<()> {
        if self.available_data_len() < n {
            Err(Error::Eof)
        } else {
            self.index += n;
            Ok(())
        }
    }

    #[inline]
    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        end: F,
    ) -> Result<Reference<'de, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>,
    {
        self.parse_str_bytes(escape, scratch, end, |bytes| {
            str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        })
    }
}

#[derive(Deref, DerefMut)]
pub struct StrRead<'de> {
    delegate: SliceRead<'de>,
}

impl<'de> StrRead<'de> {
    pub fn new(s: &'de str) -> Self {
        Self {
            delegate: SliceRead::new(s.as_bytes()),
        }
    }

    pub fn rest(&self) -> Result<&str> {
        str::from_utf8(self.delegate.rest()).map_err(|_| Error::InvalidUtf8)
    }

    #[inline]
    fn parse_str_bytes<'s, E, T, R>(
        &'s mut self,
        no_escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: E,
        result: R,
    ) -> Result<Reference<'de, 's, T>>
    where
        T: ?Sized + 's,
        E: Fn(&mut Self) -> Result<bool>,
        R: for<'f> FnOnce(&'f [u8]) -> Result<&'f T>,
    {
        parse_str_bytes_impl!(self, no_escape, scratch, delimiter, result)
    }
}

impl<'de> Read<'de> for StrRead<'de> {
    fn position(&self) -> Position {
        self.delegate.position()
    }

    #[inline]
    fn peek_n(&mut self, n: usize) -> Result<&[u8]> {
        self.delegate.peek_n(n)
    }

    #[inline]
    fn next(&mut self) -> Result<u8> {
        self.delegate.next()
    }

    #[inline]
    fn parse_str<'s, F>(
        &'s mut self,
        no_escape: bool,
        scratch: &'s mut Vec<u8>,
        end: F,
    ) -> Result<Reference<'de, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>,
    {
        self.parse_str_bytes(no_escape, scratch, end, |bytes| {
            Ok(unsafe { str::from_utf8_unchecked(bytes) })
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::parser::read::leading_whitespace_bytes;
    use crate::parser::read::{Read, StreamRead};
    use rstest::rstest;

    #[test]
    fn test_stream_peek() -> Result<()> {
        let input = "hello world";
        let mut read = StreamRead::new(input.as_bytes());
        let ch = read.peek()?;
        assert_eq!(ch, b'h');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, b'h');
        assert_eq!(ch2, b'e');
        let chars = read.peek_n(3)?;
        assert_eq!(chars, b"hel");
        read.discard(3)?;
        let ch = read.peek()?;
        assert_eq!(ch, b'l');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, b'l');
        assert_eq!(ch2, b'o');
        let chars = read.peek_n(3)?;
        assert_eq!(chars, b"lo ");
        Ok(())
    }

    #[rstest]
    #[case(&[] as &[u8], 0)]
    #[case(b"\txyz", 1)]
    #[case(b"\nabc", 1)]
    #[case(&[0x0B, b'a', b'b'], 1)]
    #[case(&[0x0C, b'a', b'b'], 1)]
    #[case(b"\rHELLO", 1)]
    #[case(b" world", 1)]
    #[case(&[0x1C, b'X', b'Y'], 1)]
    #[case(&[0x1F, b'Z'], 1)]
    #[case(&[0xC2, 0x85, b'a', b'b'], 2)]
    #[case(&[0xC2, 0xA0, b'X'], 2)]
    #[case(&[0xE1, 0x9A, 0x80, b'!'], 3)]
    #[case(&[0xE2, 0x80, 0x80, b'a'], 3)]
    #[case(&[0xE2, 0x80, 0x87, b'b'], 3)]
    #[case(&[0xE2, 0x80, 0x8A, b'c'], 3)]
    #[case(&[0xE2, 0x80, 0xA8, b'x'], 3)]
    #[case(&[0xE2, 0x80, 0xA9, b'y'], 3)]
    #[case(&[0xE2, 0x80, 0xAF, b'Z'], 3)]
    #[case(&[0xE2, 0x81, 0x9F, b'M'], 3)]
    #[case(&[0xE3, 0x80, 0x80, b'A'], 3)]
    #[case(&[0xEF, 0xBB, 0xBF, b'h'], 3)]
    #[case(b"Hello", 0)]
    #[case(&[0xE6, 0x97, 0xA5, b'X'], 0)]
    #[case(&[0xC2], 0)]
    #[case(&[0xE2, 0x80], 0)]
    fn test_leading_whitespace_bytes(#[case] bytes: &[u8], #[case] expected: usize) {
        assert_eq!(leading_whitespace_bytes(bytes), expected);
    }
}
