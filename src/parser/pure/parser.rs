use crate::parser::pure::{
    horizontal_whitespace,
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

    pub(crate) fn parse_horizontal_whitespace<F>(&mut self, callback: F) -> Result<(), DecoderError>
    where
        F: Fn(&str) -> Result<(), DecoderError>,
    {
        loop {
            match self.reader.peek_chunk() {
                Some(s) => {
                    let (first, _) = horizontal_whitespace(s);
                    if first.is_empty() {
                        return Ok(());
                    }
                    let len = first.len();
                    callback(first)?;
                    self.reader.consume(len);
                }
                None => match self.reader.fill_buf() {
                    Ok(_) => {}
                    Err(DecoderError::Eof) => {
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                },
            }
        }
    }
}
