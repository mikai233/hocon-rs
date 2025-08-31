use crate::parser::pure::{
    is_hocon_horizontal_whitespace, leading_horizontal_whitespace,
    read::{DecoderError, Read},
};

#[derive(Debug)]
pub(crate) struct Parser<R: Read> {
    pub(crate) reader: R,
}

impl<R: Read> Parser<R> {
    pub(crate) fn new(reader: R) -> Self {
        Parser { reader }
    }

    pub(crate) fn parse_leading_horizontal_whitespace<F>(
        &mut self,
        mut callback: F,
    ) -> Result<(), DecoderError>
    where
        F: FnMut(&str) -> Result<(), DecoderError>,
    {
        loop {
            match self.reader.peek_chunk() {
                Some(s) => {
                    let (first, _) = leading_horizontal_whitespace(s);
                    if first.is_empty() {
                        return Ok(());
                    }
                    let len = first.len();
                    callback(first)?;
                    self.reader.consume(len);
                }
                None => {
                    if self.fill_buf()? {
                        break Ok(());
                    }
                }
            }
        }
    }

    pub(crate) fn parse_leading_horizontal_whitespace2<'a>(
        &mut self,
        scratch: &'a mut Vec<u8>,
    ) -> Result<&'a str, DecoderError> {
        loop {
            match self.reader.peek() {
                Ok(ch) => {
                    if is_hocon_horizontal_whitespace(ch) {
                        let (_, bytes) = self.reader.next()?;
                        scratch.extend_from_slice(bytes);
                    } else {
                        break;
                    }
                }
                Err(DecoderError::Eof) => {
                    break;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        let s = unsafe { str::from_utf8_unchecked(scratch) };
        Ok(s)
    }

    /// Returns true when it reaches the end of the input.
    pub(crate) fn fill_buf(&mut self) -> Result<bool, DecoderError> {
        let mut eof = false;
        match self.reader.fill_buf() {
            Ok(_) => {}
            Err(DecoderError::Eof) => {
                eof = true;
            }
            Err(e) => return Err(e),
        }
        Ok(eof)
    }
}

pub(crate) fn empty_callback(_s: &str) -> Result<(), DecoderError> {
    Ok(())
}
