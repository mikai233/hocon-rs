use std::io::{self};
use std::str;

use crate::Result;
use crate::error::Error;
use encoding_rs::{Decoder, UTF_8};

// We should peek at least 7 bytes because the include token has a length of 7 bytes.
pub(crate) const MIN_BUFFER_SIZE: usize = 7;
pub(crate) const DEFAULT_BUFFER: usize = 4096;

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub trait Read {
    fn fill_buf(&mut self) -> Result<()>;

    fn position(&self) -> Position;

    fn peek_n<const N: usize>(&mut self) -> Result<[char; N]>;

    fn peek(&mut self) -> Result<char> {
        let chars = self.peek_n::<1>()?;
        Ok(chars[0])
    }

    fn peek2(&mut self) -> Result<(char, char)> {
        let chars = self.peek_n::<2>()?;
        Ok((chars[0], chars[1]))
    }

    fn peek3(&mut self) -> Result<(char, char, char)> {
        let chars = self.peek_n::<3>()?;
        Ok((chars[0], chars[1], chars[2]))
    }

    fn next(&mut self) -> Result<(char, &[u8])>;
}

#[inline]
fn decode_first_char(slice: &[u8]) -> Option<(char, usize)> {
    let b0 = *slice.first()?;
    if b0 < 128 {
        Some((b0 as char, 1))
    } else if b0 < 0xE0 {
        let b1 = *slice.get(1)?;
        let ch = ((b0 as u32 & 0x1F) << 6) | (b1 as u32 & 0x3F);
        Some((unsafe { char::from_u32_unchecked(ch) }, 2))
    } else if b0 < 0xF0 {
        let b1 = *slice.get(1)?;
        let b2 = *slice.get(2)?;
        let ch = ((b0 as u32 & 0x0F) << 12) | ((b1 as u32 & 0x3F) << 6) | (b2 as u32 & 0x3F);
        Some((unsafe { char::from_u32_unchecked(ch) }, 3))
    } else {
        let b1 = *slice.get(1)?;
        let b2 = *slice.get(2)?;
        let b3 = *slice.get(3)?;
        let ch = ((b0 as u32 & 0x07) << 18)
            | ((b1 as u32 & 0x3F) << 12)
            | ((b2 as u32 & 0x3F) << 6)
            | (b3 as u32 & 0x3F);
        Some((unsafe { char::from_u32_unchecked(ch) }, 4))
    }
}

pub struct StreamRead<R: std::io::Read, const N: usize = 8192> {
    inner: R,
    decoder: Decoder,
    buffer: [u8; N],
    decoded_start: usize,
    decoded_end: usize,
    eof: bool,
    line: usize,
    col: usize,
    start_of_line: usize,
}

impl<R: io::BufRead, const BUFFER: usize> StreamRead<R, BUFFER> {
    pub fn new(reader: R) -> Self {
        StreamRead {
            inner: reader,
            decoder: UTF_8.new_decoder(),
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

        // 回收前面已消费的空间，类似滑动窗口
        if self.decoded_start > 0 {
            let len = self.decoded_end - self.decoded_start;
            // 把剩余数据移到开头
            self.buffer
                .copy_within(self.decoded_start..self.decoded_end, 0);
            self.decoded_start = 0;
            self.decoded_end = len;
        }

        loop {
            let input_buf = self.inner.fill_buf()?;
            let empty_buf = &mut self.buffer[self.decoded_end..];

            if input_buf.is_empty() {
                self.eof = true;
                let (_, _, written, err) = self.decoder.decode_to_utf8(b"", empty_buf, true);
                self.decoded_end += written;
                if err {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid UTF-8 sequence",
                    )
                    .into());
                }
                break;
            }

            let (_, read, written, err) = self.decoder.decode_to_utf8(input_buf, empty_buf, false);

            self.inner.consume(read);
            self.decoded_end += written;

            if err {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 sequence").into(),
                );
            }

            if self.decoded_end > 0 {
                break;
            }
        }

        Ok(())
    }
}

impl<R: std::io::BufRead, const BUFFER: usize> Read for StreamRead<R, BUFFER> {
    fn fill_buf(&mut self) -> Result<()> {
        self.fill_buf()
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.col,
        }
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<[char; N]> {
        debug_assert!(N > 0 && N <= BUFFER);

        let mut out: [char; N] = ['\0'; N];
        let mut idx = 0usize;
        // 本次 peek 的“临时前进”字节数（不修改 decoded_start）
        let mut off = 0usize;

        loop {
            // 已经拿够 N 个字符
            if idx == N {
                return Ok(out);
            }

            // 需要更多数据？（我们临时前进到尾了）
            if self.decoded_start + off == self.decoded_end {
                // 试图补充数据；fill_buf 可能会:
                // 1) 把 [decoded_start..decoded_end] 拷到开头，令 decoded_start=0;
                // 2) 追加新的解码字节到 decoded_end;
                self.fill_buf()?;

                // 仍然没有数据 => EOF
                if self.decoded_start == self.decoded_end {
                    return Err(Error::Eof);
                }

                // 继续从“已前进 off 字节”的位置接着解码
                // 注意：fill_buf 可能把未消费数据搬到了 0 开头，但 off 仍然是
                // “相对于未消费区域起点”的偏移量，不需要回退。
                if off > self.decoded_end - self.decoded_start {
                    // 理论上不会发生；防御式处理一下
                    off = self.decoded_end - self.decoded_start;
                }
            }

            let slice = &self.buffer[self.decoded_start + off..self.decoded_end];

            // 试着在当前 slice 上解一个字符
            if let Some((ch, len)) = decode_first_char(slice) {
                out[idx] = ch;
                idx += 1;
                off += len; // 仅本次 peek 的临时偏移，真实 decoded_start 不变
                continue;
            }

            // 如果这里拿不到字符，多半是遇到“被截断的多字节字符”，再填充一次继续
            self.fill_buf()?;
            if self.decoded_start == self.decoded_end {
                return Err(Error::Eof);
            }
            // 回到循环顶部，保持 off 不变，接着从相同逻辑位置继续
        }
    }

    fn next(&mut self) -> Result<(char, &[u8])> {
        if self.decoded_start == self.decoded_end {
            self.fill_buf()?;
        }
        let slice = &self.buffer[self.decoded_start..self.decoded_end];
        let (ch, len_utf8) = decode_first_char(slice).ok_or(Error::Eof)?;
        if ch == '\n' {
            self.start_of_line += self.col + 1;
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        let bytes = &slice[..len_utf8];
        self.decoded_start += len_utf8;
        if self.decoded_start == self.decoded_end {
            self.decoded_start = 0;
            self.decoded_end = 0;
        }
        Ok((ch, bytes))
    }
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
}

impl<'a> Read for SliceRead<'a> {
    fn fill_buf(&mut self) -> Result<()> {
        Err(Error::Eof)
    }

    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<[char; N]> {
        debug_assert!(N > 0 && N <= MIN_BUFFER_SIZE);
        //TODO
        unimplemented!()
    }

    fn next(&mut self) -> Result<(char, &[u8])> {
        //TODO
        unimplemented!()
    }
}

pub struct StrRead<'a> {
    s: &'a str,
    index: usize,
}

impl<'a> StrRead<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, index: 0 }
    }

    pub fn position_of_index(&self, i: usize) -> Position {
        let start_of_line = match memchr::memrchr(b'\n', &self.s.as_bytes()[..i]) {
            Some(position) => position + 1,
            None => 0,
        };
        Position {
            line: 1 + memchr::memchr_iter(b'\n', &self.s.as_bytes()[..start_of_line]).count(),
            column: i - start_of_line,
        }
    }

    pub fn rest(&self) -> &str {
        &self.s[self.index..]
    }
}

impl<'a> Read for StrRead<'a> {
    fn fill_buf(&mut self) -> Result<()> {
        Err(Error::Eof)
    }

    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<[char; N]> {
        debug_assert!(N > 0 && N <= MIN_BUFFER_SIZE);
        let mut slice = &self.s[self.index..];
        let mut chars: [char; N] = ['\0'; N]; // 先用零初始化
        let mut idx = 0;

        while idx < N {
            if let Some((ch, len)) = decode_first_char(slice.as_bytes()) {
                chars[idx] = ch;
                slice = &slice[len..];
                idx += 1;
            } else {
                return Err(Error::Eof);
            }
        }

        Ok(chars)
    }

    fn next(&mut self) -> Result<(char, &[u8])> {
        if self.index == self.s.len() {
            return Err(Error::Eof);
        }
        let slice = &self.s[self.index..];
        let (ch, len_utf8) = decode_first_char(slice.as_bytes()).ok_or(Error::Eof)?;
        let bytes = &slice.as_bytes()[..len_utf8];
        self.index += len_utf8;
        Ok((ch, bytes))
    }
}

#[cfg(test)]
pub(crate) struct TestRead {
    slice: Vec<u8>,
    index: usize,
    fill_buf: Box<dyn FnMut() -> Vec<u8>>,
}

#[cfg(test)]
impl TestRead {
    pub(crate) fn new<F>(slice: Vec<u8>, fill_buf: F) -> Self
    where
        F: FnMut() -> Vec<u8> + 'static,
    {
        TestRead {
            slice,
            index: 0,
            fill_buf: Box::new(fill_buf),
        }
    }

    pub(crate) fn from_input(input: Vec<&str>) -> Self {
        let mut input = input
            .into_iter()
            .map(|s| s.as_bytes().to_vec())
            .collect::<Vec<_>>();
        TestRead::new(vec![], move || {
            if input.is_empty() {
                vec![]
            } else {
                input.remove(0)
            }
        })
    }

    pub(crate) fn position_of_index(&self, i: usize) -> Position {
        let start_of_line = match memchr::memrchr(b'\n', &self.slice[..i]) {
            Some(position) => position + 1,
            None => 0,
        };
        Position {
            line: 1 + memchr::memchr_iter(b'\n', &self.slice[..start_of_line]).count(),
            column: i - start_of_line,
        }
    }

    pub(crate) fn rest(&mut self) -> &str {
        while let Ok(_) = self.fill_buf() {}
        unsafe { str::from_utf8_unchecked(&self.slice[self.index..]) }
    }
}

#[cfg(test)]
impl Read for TestRead {
    fn fill_buf(&mut self) -> Result<()> {
        let buf = (self.fill_buf)();
        if buf.is_empty() {
            Err(Error::Eof)
        } else {
            self.slice.extend(buf);
            Ok(())
        }
    }

    fn position(&self) -> Position {
        self.position_of_index(self.index)
    }

    #[inline]
    fn peek_n<const N: usize>(&mut self) -> Result<[char; N]> {
        debug_assert!(N > 0 && N <= MIN_BUFFER_SIZE);

        let mut out: [char; N] = ['\0'; N];
        let mut idx = 0usize;
        // 本次 peek 的“临时前进”字节数（不修改 decoded_start）
        let mut off = 0usize;

        loop {
            // 已经拿够 N 个字符
            if idx == N {
                return Ok(out);
            }

            // 需要更多数据？（我们临时前进到尾了）
            if self.index + off == self.slice.len() {
                // 试图补充数据；fill_buf 可能会:
                // 1) 把 [decoded_start..decoded_end] 拷到开头，令 decoded_start=0;
                // 2) 追加新的解码字节到 decoded_end;
                self.fill_buf()?;

                // 仍然没有数据 => EOF
                if self.index == self.slice.len() {
                    return Err(Error::Eof);
                }

                // 继续从“已前进 off 字节”的位置接着解码
                // 注意：fill_buf 可能把未消费数据搬到了 0 开头，但 off 仍然是
                // “相对于未消费区域起点”的偏移量，不需要回退。
                if off > self.slice.len() - self.index {
                    // 理论上不会发生；防御式处理一下
                    off = self.slice.len() - self.index
                }
            }

            let slice = &self.slice[self.index + off..];

            // 试着在当前 slice 上解一个字符
            if let Some((ch, len)) = decode_first_char(slice) {
                out[idx] = ch;
                idx += 1;
                off += len; // 仅本次 peek 的临时偏移，真实 decoded_start 不变
                continue;
            }

            // 如果这里拿不到字符，多半是遇到“被截断的多字节字符”，再填充一次继续
            self.fill_buf()?;
            if self.index == self.slice.len() {
                return Err(Error::Eof);
            }
            // 回到循环顶部，保持 off 不变，接着从相同逻辑位置继续
        }
    }

    fn next(&mut self) -> Result<(char, &[u8])> {
        if self.index == self.slice.len() {
            self.fill_buf()?;
        }
        let slice = &self.slice[self.index..];
        let (ch, len_utf8) = decode_first_char(slice).ok_or(Error::Eof)?;
        let bytes = &slice[..len_utf8];
        self.index += len_utf8;
        Ok((ch, bytes))
    }
}

#[cfg(test)]
pub(crate) type TestStreamRead<R> = StreamRead<R, 3>;

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::parser::read::{Read, StreamRead, TestRead};
    use std::io::BufReader;

    #[test]
    fn test_slice_peek() -> Result<()> {
        let input = vec!["h", "e", "l", "l", "o"];
        let mut read = TestRead::from_input(input);
        let ch = read.peek()?;
        assert_eq!(ch, 'h');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, 'h');
        assert_eq!(ch2, 'e');
        let (ch1, ch2, ch3) = read.peek3()?;
        assert_eq!(ch1, 'h');
        assert_eq!(ch2, 'e');
        assert_eq!(ch3, 'l');
        Ok(())
    }

    #[test]
    fn test_stream_peek() -> Result<()> {
        let input = "hello world";
        let mut read: StreamRead<_, 4> = StreamRead::new(BufReader::new(input.as_bytes()));
        let ch = read.peek()?;
        assert_eq!(ch, 'h');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, 'h');
        assert_eq!(ch2, 'e');
        let (ch1, ch2, ch3) = read.peek3()?;
        assert_eq!(ch1, 'h');
        assert_eq!(ch2, 'e');
        assert_eq!(ch3, 'l');
        read.next()?;
        read.next()?;
        read.next()?;
        let ch = read.peek()?;
        assert_eq!(ch, 'l');
        let (ch1, ch2) = read.peek2()?;
        assert_eq!(ch1, 'l');
        assert_eq!(ch2, 'o');
        let (ch1, ch2, ch3) = read.peek3()?;
        assert_eq!(ch1, 'l');
        assert_eq!(ch2, 'o');
        assert_eq!(ch3, ' ');
        Ok(())
    }
}
