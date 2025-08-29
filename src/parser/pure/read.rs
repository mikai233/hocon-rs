use std::io::BufReader;
use std::io::{self, BufRead};
use std::str;

use encoding_rs::{Decoder, UTF_8};

#[derive(Debug)]
pub enum DecoderError {
    Io(io::Error),
    Utf8(str::Utf8Error),
    Incomplete,
    InvalidEscape,
    UnexpectedEof,
    UnexpectedToken,
    Eof,
}

impl From<io::Error> for DecoderError {
    fn from(err: io::Error) -> Self {
        DecoderError::Io(err)
    }
}

impl From<str::Utf8Error> for DecoderError {
    fn from(err: str::Utf8Error) -> Self {
        DecoderError::Utf8(err)
    }
}

pub trait Read {
    fn peek_chunk(&self) -> Option<&str>;

    fn consume(&mut self, len_utf8: usize);

    fn fill_buf(&mut self) -> Result<(), DecoderError>;

    fn available_chars(&self) -> usize;
}

pub struct StreamRead<R: std::io::Read> {
    inner: BufReader<R>,
    decoder: Decoder,
    buffer: [u8; 8192],
    decoded_start: usize,
    decoded_end: usize,
    eof: bool,
}

impl<R: std::io::Read> StreamRead<R> {
    pub fn new(reader: R) -> Self {
        StreamRead {
            inner: BufReader::new(reader),
            decoder: UTF_8.new_decoder(),
            buffer: [0u8; 8192],
            decoded_start: 0,
            decoded_end: 0,
            eof: false,
        }
    }

    fn fill_buf(&mut self) -> Result<(), DecoderError> {
        // 1. 如果缓冲区中已经有数据，直接返回。
        if self.decoded_start < self.decoded_end {
            return Ok(());
        }

        // 2. 如果之前已经处理完文件末尾(EOF)，则返回错误，防止重复调用。
        if self.eof {
            return Err(DecoderError::Eof);
        }

        // 3. 核心循环：持续读取和解码，直到缓冲区被填充或到达文件末尾。
        loop {
            // 从底层读取器获取原始字节数据
            let input_buf = self.inner.fill_buf()?;

            // 准备用于接收解码后字符的输出缓冲区
            let empty_buf = &mut self.buffer[self.decoded_end..];

            // 检查是否已到达输入流的末尾
            if input_buf.is_empty() {
                self.eof = true;
                // 这是最后一次调用解码器，last=true 会处理所有缓冲中的字节。
                let (_, _, written, err) = self.decoder.decode_to_utf8(b"", empty_buf, true);
                self.decoded_end += written;

                if err {
                    return Err(
                        io::Error::new(io::ErrorKind::InvalidData, "无效的UTF-8序列").into(),
                    );
                }

                // 已到达文件末尾，无论是否解码出新字符，都应跳出循环。
                break;
            }

            // --- 如果未到末尾，则正常解码 ---
            let (_, read, written, err) = self.decoder.decode_to_utf8(input_buf, empty_buf, false);

            // 标记已处理的原始字节
            self.inner.consume(read);
            // 更新解码后的缓冲区末尾指针
            self.decoded_end += written;

            if err {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的UTF-8序列").into());
            }

            // 如果成功解码出了一些字符，我们的任务就完成了，跳出循环。
            // `decoded_start` 在这个函数开始时必为0（或等于decoded_end），所以 `decoded_end > 0` 即可。
            if self.decoded_end > 0 {
                break;
            }

            // 如果 `written` 为 0 (例如，只读到了一个不完整的UTF-8序列)，
            // 循环会继续，尝试从 `inner` 读取更多字节，而不会发生递归。
        }

        Ok(())
    }
}

impl<R: std::io::Read> Read for StreamRead<R> {
    fn peek_chunk(&self) -> Option<&str> {
        if self.decoded_start == self.decoded_end {
            return None;
        }
        let slice = &self.buffer[self.decoded_start..self.decoded_end];
        Some(unsafe { str::from_utf8_unchecked(slice) })
    }

    fn consume(&mut self, len_utf8: usize) {
        if self.decoded_start + len_utf8 > self.decoded_end {
            panic!("consume 超出缓冲区范围");
        }
        self.decoded_start += len_utf8;
        if self.decoded_start == self.decoded_end {
            self.decoded_start = 0;
            self.decoded_end = 0;
        }
    }

    fn fill_buf(&mut self) -> Result<(), DecoderError> {
        self.fill_buf()
    }

    fn available_chars(&self) -> usize {
        self.peek_chunk()
            .map(|s| s.chars().count())
            .unwrap_or_default()
    }
}

pub struct SliceRead<'a> {
    slice: &'a str,
    index: usize,
}

impl<'a> SliceRead<'a> {
    pub fn new(slice: &'a str) -> Self {
        SliceRead { slice, index: 0 }
    }
}

impl<'a> Read for SliceRead<'a> {
    fn peek_chunk(&self) -> Option<&str> {
        if self.index == self.slice.len() {
            return None;
        }
        Some(&self.slice[self.index..])
    }

    fn consume(&mut self, len_utf8: usize) {
        if self.index + len_utf8 > self.slice.len() {
            panic!(
                "consume 超出缓冲区范围 len_utf8:{}, index:{}, slice_len:{}",
                len_utf8,
                self.index,
                self.slice.len()
            );
        }
        self.index += len_utf8;
    }

    fn fill_buf(&mut self) -> Result<(), DecoderError> {
        return Err(DecoderError::Eof);
    }

    fn available_chars(&self) -> usize {
        self.peek_chunk()
            .map(|s| s.chars().count())
            .unwrap_or_default()
    }
}

pub(crate) struct TestRead {
    slice: Vec<u8>,
    index: usize,
    fill_buf: Box<dyn FnMut() -> Vec<u8>>,
}

impl TestRead {
    pub fn new<F>(slice: Vec<u8>, fill_buf: F) -> Self
    where
        F: FnMut() -> Vec<u8> + 'static,
    {
        TestRead {
            slice,
            index: 0,
            fill_buf: Box::new(fill_buf),
        }
    }
}

impl Read for TestRead {
    fn peek_chunk(&self) -> Option<&str> {
        if self.index == self.slice.len() {
            return None;
        }
        Some(std::str::from_utf8(&self.slice[self.index..]).unwrap())
    }

    fn consume(&mut self, len_utf8: usize) {
        if self.index + len_utf8 > self.slice.len() {
            panic!(
                "consume 超出缓冲区范围 len_utf8:{}, index:{}, slice_len:{}",
                len_utf8,
                self.index,
                self.slice.len()
            );
        }
        self.index += len_utf8;
    }

    fn fill_buf(&mut self) -> Result<(), DecoderError> {
        let buf = (self.fill_buf)();
        if buf.is_empty() {
            Err(DecoderError::Eof)
        } else {
            self.slice.extend(buf);
            Ok(())
        }
    }

    fn available_chars(&self) -> usize {
        self.peek_chunk()
            .map(|s| s.chars().count())
            .unwrap_or_default()
    }
}
