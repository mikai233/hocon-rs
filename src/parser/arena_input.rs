use nom::{AsBytes, Mode};
use nom::{CompareResult, IsStreaming};
use std::fmt::Display;
use std::ops::Bound;
use std::ops::RangeBounds;
use std::{
    ops::Deref,
    str::{CharIndices, Chars},
};

use bumpalo::Bump;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArenaInput<'a> {
    pub(crate) arean: &'a Bump,
    pub(crate) input: &'a str,
}

impl<'a> ArenaInput<'a> {
    pub(crate) fn copy_from(self, input: &'a str) -> ArenaInput<'a> {
        ArenaInput {
            arean: self.arean,
            input,
        }
    }
}

impl ArenaInput<'_> {
    fn split_at(&self, mid: usize) -> (Self, Self) {
        let (first, last) = self.input.split_at(mid);
        (self.copy_from(first), self.copy_from(last))
    }

    unsafe fn get_unchecked<R>(&self, range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.input.len(),
        };
        let s = unsafe { self.input.get_unchecked(start..end) };
        self.copy_from(s)
    }
}

impl Deref for ArenaInput<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<'a> nom::Input for ArenaInput<'a> {
    type Item = char;

    type Iter = Chars<'a>;

    type IterIndices = CharIndices<'a>;

    fn input_len(&self) -> usize {
        <&str as nom::Input>::input_len(&self.input)
    }

    fn take(&self, index: usize) -> Self {
        self.copy_from(<&str as nom::Input>::take(&self.input, index))
    }

    fn take_from(&self, index: usize) -> Self {
        self.copy_from(<&str as nom::Input>::take(&self.input, index))
    }

    fn take_split(&self, index: usize) -> (Self, Self) {
        let (suffix, prefix) = <&str as nom::Input>::take_split(&self.input, index);
        (self.copy_from(suffix), self.copy_from(prefix))
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        <&str as nom::Input>::position(&self.input, predicate)
    }

    fn iter_elements(&self) -> Self::Iter {
        <&str as nom::Input>::iter_elements(&self.input)
    }

    fn iter_indices(&self) -> Self::IterIndices {
        <&str as nom::Input>::iter_indices(&self.input)
    }

    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        <&str as nom::Input>::slice_index(&self.input, count)
    }

    fn split_at_position<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.input.find(predicate) {
            Some(i) => {
                let (str1, str2) = self.split_at(i);
                Ok((str2, str1))
            }
            None => Err(nom::Err::Incomplete(nom::Needed::new(1))),
        }
    }

    fn split_at_position1<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
        e: nom::error::ErrorKind,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.find(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
            Some(i) => {
                let (str1, str2) = self.split_at(i);
                Ok((str2, str1))
            }
            None => Err(nom::Err::Incomplete(nom::Needed::new(1))),
        }
    }

    fn split_at_position_complete<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.find(predicate) {
            Some(i) => {
                let (str1, str2) = self.split_at(i);
                Ok((str2, str1))
            }
            None => Ok(self.split_at(0)),
        }
    }

    fn split_at_position1_complete<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
        e: nom::error::ErrorKind,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.find(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
            Some(i) => {
                let (str1, str2) = self.split_at(i);
                Ok((str2, str1))
            }
            None => {
                if self.is_empty() {
                    Err(nom::Err::Error(E::from_error_kind(*self, e)))
                } else {
                    let (str1, str2) = self.split_at(self.len());
                    Ok((str2, str1))
                }
            }
        }
    }

    fn split_at_position_mode<OM: nom::OutputMode, P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
    ) -> nom::PResult<OM, Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.find(predicate) {
            Some(n) => unsafe {
                Ok((
                    self.get_unchecked(n..),
                    OM::Output::bind(|| self.get_unchecked(..n)),
                ))
            },
            None => {
                if OM::Incomplete::is_streaming() {
                    Err(nom::Err::Incomplete(nom::Needed::new(1)))
                } else {
                    unsafe {
                        Ok((
                            self.get_unchecked(self.len()..),
                            OM::Output::bind(|| self.get_unchecked(..self.len())),
                        ))
                    }
                }
            }
        }
    }

    fn split_at_position_mode1<OM: nom::OutputMode, P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
        e: nom::error::ErrorKind,
    ) -> nom::PResult<OM, Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.find(predicate) {
            Some(0) => Err(nom::Err::Error(OM::Error::bind(|| {
                E::from_error_kind(*self, e)
            }))),
            Some(n) => unsafe {
                Ok((
                    self.get_unchecked(n..),
                    OM::Output::bind(|| self.get_unchecked(..n)),
                ))
            },
            None => {
                if OM::Incomplete::is_streaming() {
                    Err(nom::Err::Incomplete(nom::Needed::new(1)))
                } else if self.is_empty() {
                    Err(nom::Err::Error(OM::Error::bind(|| {
                        E::from_error_kind(*self, e)
                    })))
                } else {
                    unsafe {
                        Ok((
                            self.get_unchecked(self.len()..),
                            OM::Output::bind(|| self.get_unchecked(..self.len())),
                        ))
                    }
                }
            }
        }
    }
}

impl nom::Offset for ArenaInput<'_> {
    fn offset(&self, second: &Self) -> usize {
        <str as nom::Offset>::offset(&self.input, second.input)
    }
}

impl nom::Offset for &ArenaInput<'_> {
    fn offset(&self, second: &Self) -> usize {
        <&str as nom::Offset>::offset(&self.input, &second.input)
    }
}

impl nom::AsBytes for ArenaInput<'_> {
    fn as_bytes(&self) -> &[u8] {
        <str as nom::AsBytes>::as_bytes(&self.input)
    }
}

impl nom::AsBytes for &ArenaInput<'_> {
    fn as_bytes(&self) -> &[u8] {
        <&str as nom::AsBytes>::as_bytes(&self.input)
    }
}

impl<'a, 'b> nom::Compare<&'b str> for ArenaInput<'a> {
    fn compare(&self, t: &'b str) -> CompareResult {
        self.input.as_bytes().compare(t.as_bytes())
    }

    fn compare_no_case(&self, t: &'b str) -> CompareResult {
        todo!()
    }
}

impl Display for ArenaInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.input)
    }
}
