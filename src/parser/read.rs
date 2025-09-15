use std::{slice, str};

use derive_more::{Deref, DerefMut};

use crate::Result;
use crate::error::Error;

// We should peek at least 7 bytes because the include token has a length of 7 bytes.
pub(crate) const MIN_BUFFER_SIZE: usize = 4096;

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
        _ => return Err(Error::InvalidEscape),
    }
    Ok(())
}
fn parse_escaped_unicode<'de, R: Read<'de>>(reader: &mut R, scratch: &mut Vec<u8>) -> Result<()> {
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
    let mut n = parse_hex16(reader)? as u32;

    // Handle surrogate pair
    if (0xD800..=0xDBFF).contains(&n) {
        // Expect \u for low surrogate
        if reader.next()? != b'\\' || reader.next()? != b'u' {
            return Err(Error::InvalidEscape);
        }
        let n2 = parse_hex16(reader)? as u32;
        if !(0xDC00..=0xDFFF).contains(&n2) {
            return Err(Error::InvalidEscape);
        }
        n = 0x10000 + (((n - 0xD800) << 10) | (n2 - 0xDC00));
    }

    // Encode as UTF-8 manually
    if n <= 0x7F {
        scratch.push(n as u8);
    } else if n <= 0x7FF {
        scratch.push((n >> 6 & 0x1F) as u8 | 0xC0);
        scratch.push((n & 0x3F) as u8 | 0x80);
    } else if n <= 0xFFFF {
        scratch.push((n >> 12 & 0x0F) as u8 | 0xE0);
        scratch.push((n >> 6 & 0x3F) as u8 | 0x80);
        scratch.push((n & 0x3F) as u8 | 0x80);
    } else if n <= 0x10FFFF {
        scratch.push((n >> 18 & 0x07) as u8 | 0xF0);
        scratch.push((n >> 12 & 0x3F) as u8 | 0x80);
        scratch.push((n >> 6 & 0x3F) as u8 | 0x80);
        scratch.push((n & 0x3F) as u8 | 0x80);
    } else {
        return Err(Error::InvalidEscape);
    }
    Ok(())
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

pub trait Read<'a> {
    fn position(&self) -> Position;

    fn peek_n<const N: usize>(&mut self) -> Result<&[u8]>;

    fn peek(&mut self) -> Result<u8> {
        let chars = self.peek_n::<1>()?;
        Ok(chars[0])
    }

    fn peek2(&mut self) -> Result<(u8, u8)> {
        let chars = self.peek_n::<2>()?;
        Ok((chars[0], chars[1]))
    }

    fn peek3(&mut self) -> Result<(u8, u8, u8)> {
        let chars = self.peek_n::<3>()?;
        Ok((chars[0], chars[1], chars[2]))
    }

    fn next(&mut self) -> Result<u8>;

    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: F,
    ) -> Result<Reference<'a, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>;

    fn peek_whitespace(&mut self) -> Result<Option<usize>> {
        let n = match self.peek_n::<3>() {
            Ok(bytes) => leading_whitespace_bytes(bytes),
            Err(Error::Eof) => match self.peek_n::<2>() {
                Ok(bytes) => leading_whitespace_bytes(bytes),
                Err(Error::Eof) => match self.peek_n::<1>() {
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

    fn starts_with_whitespace(&mut self) -> Result<bool> {
        self.peek_whitespace().map(|n| n.is_some())
    }

    fn peek_horizontal_whitespace(&mut self) -> Result<Option<usize>> {
        if self.peek()? != b'\n' {
            self.peek_whitespace()
        } else {
            Ok(None)
        }
    }

    fn starts_with_horizontal_whitespace(&mut self) -> Result<bool> {
        self.peek_horizontal_whitespace().map(|n| n.is_some())
    }
}

pub struct StreamRead<R: std::io::Read> {
    inner: R,
    buffer: [u8; MIN_BUFFER_SIZE],
    decoded_start: usize,
    decoded_end: usize,
    eof: bool,
    line: usize,
    col: usize,
    start_of_line: usize,
}

impl<R: std::io::Read> StreamRead<R> {
    pub fn new(reader: R) -> Self {
        StreamRead {
            inner: reader,
            buffer: [0u8; _],
            decoded_start: 0,
            decoded_end: 0,
            eof: false,
            line: 0,
            col: 0,
            start_of_line: 0,
        }
    }

    fn fill_buf(&mut self) -> Result<()> {
        if self.eof {
            return Err(Error::Eof);
        }

        // 如果 buffer 已经满了，就不能再读
        if self.decoded_end == self.buffer.len() {
            return Ok(());
        }

        let empty_buf = &mut self.buffer[self.decoded_end..];
        let n = self.inner.read(empty_buf)?;
        if n == 0 {
            self.eof = true;
        }
        self.decoded_end += n;
        Ok(())
    }

    fn available_data_len(&self) -> usize {
        self.decoded_end - self.decoded_start
    }
}

impl<'a, R: std::io::Read> Read<'a> for StreamRead<R> {
    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.col,
        }
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<&[u8]> {
        debug_assert!(N > 0 && N <= MIN_BUFFER_SIZE);

        while self.available_data_len() < N {
            // 如果 buffer 已经写满但数据不够 -> 搬移一下
            if self.decoded_end == self.buffer.len() && self.decoded_start > 0 {
                let len = self.decoded_end - self.decoded_start;
                self.buffer
                    .copy_within(self.decoded_start..self.decoded_end, 0);
                self.decoded_start = 0;
                self.decoded_end = len;
            }

            match self.fill_buf() {
                Ok(()) => {}
                Err(Error::Eof) => break,
                Err(e) => return Err(e),
            }
        }

        if self.available_data_len() < N {
            Err(Error::Eof)
        } else {
            Ok(&self.buffer[self.decoded_start..self.decoded_start + N])
        }
    }

    fn next(&mut self) -> Result<u8> {
        if self.available_data_len() > 0 {
            let byte = self.buffer[self.decoded_start];
            self.decoded_start += 1;
            if self.decoded_start == self.decoded_end {
                self.decoded_start = 0;
                self.decoded_end = 0;
            }
            Ok(byte)
        } else {
            let mut byte = 0;
            match self.inner.read(slice::from_mut(&mut byte)) {
                Ok(0) => Err(Error::Eof),
                Ok(..) => Ok(byte),
                Err(e) => Err(Error::Io(e)),
            }
        }
    }

    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: F,
    ) -> Result<Reference<'a, 's, str>>
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
        match str::from_utf8(scratch) {
            Ok(s) => Ok(Reference::Copied(s)),
            Err(_) => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid UTF-8",
            ))),
        }
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

pub struct SliceRead<'a> {
    slice: &'a [u8],
    index: usize,
}

impl<'a> SliceRead<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
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

    fn available_data_len(&self) -> usize {
        self.slice.len() - self.index
    }

    pub(crate) fn rest(&self) -> &[u8] {
        &self.slice[self.index..]
    }

    fn parse_str_bytes<'s, E, T, R>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: E,
        result: R,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        E: Fn(&mut Self) -> Result<bool>,
        R: for<'f> FnOnce(&'f [u8]) -> Result<&'f T>,
    {
        parse_str_bytes_impl!(self, escape, scratch, delimiter, result)
    }
}

impl<'a> Read<'a> for SliceRead<'a> {
    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<&[u8]> {
        debug_assert!(N > 0 && N <= MIN_BUFFER_SIZE);
        if self.available_data_len() < N {
            Err(Error::Eof)
        } else {
            Ok(&self.slice[self.index..self.index + N])
        }
    }

    fn next(&mut self) -> Result<u8> {
        if self.index == self.slice.len() {
            return Err(Error::Eof);
        }
        let byte = self.slice[self.index];
        self.index += 1;
        Ok(byte)
    }

    fn parse_str<'s, F>(
        &'s mut self,
        escape: bool,
        scratch: &'s mut Vec<u8>,
        end: F,
    ) -> Result<Reference<'a, 's, str>>
    where
        F: Fn(&mut Self) -> Result<bool>,
    {
        self.parse_str_bytes(escape, scratch, end, |bytes| {
            str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        })
    }
}

#[derive(Deref, DerefMut)]
pub struct StrRead<'a> {
    delegate: SliceRead<'a>,
}

impl<'a> StrRead<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            delegate: SliceRead::new(s.as_bytes()),
        }
    }

    pub fn rest(&self) -> Result<&str> {
        str::from_utf8(self.delegate.rest()).map_err(|_| Error::InvalidUtf8)
    }

    fn parse_str_bytes<'s, E, T, R>(
        &'s mut self,
        no_escape: bool,
        scratch: &'s mut Vec<u8>,
        delimiter: E,
        result: R,
    ) -> Result<Reference<'a, 's, T>>
    where
        T: ?Sized + 's,
        E: Fn(&mut Self) -> Result<bool>,
        R: for<'f> FnOnce(&'f [u8]) -> Result<&'f T>,
    {
        parse_str_bytes_impl!(self, no_escape, scratch, delimiter, result)
    }
}

impl<'a> Read<'a> for StrRead<'a> {
    fn position(&self) -> Position {
        self.delegate.position()
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<&[u8]> {
        self.delegate.peek_n::<N>()
    }

    fn next(&mut self) -> Result<u8> {
        self.delegate.next()
    }

    fn parse_str<'s, F>(
        &'s mut self,
        no_escape: bool,
        scratch: &'s mut Vec<u8>,
        end: F,
    ) -> Result<Reference<'a, 's, str>>
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
    use crate::parser::read::{Read, StreamRead};

    #[test]
    fn test_stream_peek() -> Result<()> {
        let input = "hello world";
        let mut read = StreamRead::new(input.as_bytes());
        let ch = read.peek()?;
        assert_eq!(ch, b'h');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, b'h');
        assert_eq!(ch2, b'e');
        let (ch1, ch2, ch3) = read.peek3()?;
        assert_eq!(ch1, b'h');
        assert_eq!(ch2, b'e');
        assert_eq!(ch3, b'l');
        read.next()?;
        read.next()?;
        read.next()?;
        let ch = read.peek()?;
        assert_eq!(ch, b'l');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, b'l');
        assert_eq!(ch2, b'o');
        let (ch1, ch2, ch3) = read.peek3()?;
        assert_eq!(ch1, b'l');
        assert_eq!(ch2, b'o');
        assert_eq!(ch3, b' ');
        Ok(())
    }
}
