use std::collections::VecDeque;
use std::{cell::RefCell, fmt::Display};

use crate::error::Error;
use crate::merge::{path::RefPath, value::Value};
use crate::{Result, join_format};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Concat {
    values: VecDeque<RefCell<Value>>,
    spaces: VecDeque<Option<String>>,
}

impl Concat {
    pub(crate) fn new(
        values: VecDeque<RefCell<Value>>,
        spaces: VecDeque<Option<String>>,
    ) -> Result<Self> {
        if values.len() != spaces.len() + 1 {
            return Err(Error::InvalidConcat(values.len(), spaces.len()));
        }
        Ok(Self { values, spaces })
    }

    pub(crate) fn two(left: Value, space: Option<String>, right: Value) -> Self {
        let values = VecDeque::from_iter([RefCell::new(left), RefCell::new(right)]);
        let spaces = VecDeque::from_iter([space]);
        Self { values, spaces }
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::concat::Concat,
    ) -> Result<Self> {
        let (raw_values, spaces) = raw.into_inner();
        let spaces = VecDeque::from_iter(spaces);
        let mut values = VecDeque::with_capacity(raw_values.len());
        for val in raw_values {
            let val = Value::from_raw(parent, val)?;
            values.push_back(RefCell::new(val));
        }
        Self::new(values, spaces)
    }

    pub(crate) fn push_back(&mut self, space: Option<String>, val: RefCell<Value>) {
        if self.values.is_empty() {
            debug_assert!(space.is_none());
            self.values.push_back(val);
        } else {
            self.values.push_back(val);
            self.spaces.push_back(space);
        }
        debug_assert_eq!(self.values.len(), self.spaces.len() + 1);
    }

    pub(crate) fn pop_back(&mut self) -> Option<(Option<String>, RefCell<Value>)> {
        let v = self.values.pop_back();
        match v {
            Some(v) => {
                if self.values.is_empty() {
                    Some((None, v))
                } else {
                    let s = self
                        .spaces
                        .pop_back()
                        .expect("logic error, space should not be empty");
                    debug_assert_eq!(self.values.len(), self.spaces.len() + 1);
                    Some((s, v))
                }
            }
            None => {
                debug_assert!(self.spaces.is_empty());
                None
            }
        }
    }

    pub(crate) fn pop_front(&mut self) -> Option<(RefCell<Value>, Option<String>)> {
        let v = self.values.pop_front();
        match v {
            Some(v) => {
                if self.values.is_empty() {
                    Some((v, None))
                } else {
                    let s = self
                        .spaces
                        .pop_front()
                        .expect("logic error, space should not be empty");
                    debug_assert_eq!(self.values.len(), self.spaces.len() + 1);
                    Some((v, s))
                }
            }
            None => {
                debug_assert!(self.spaces.is_empty());
                None
            }
        }
    }

    pub(crate) fn push_front(&mut self, val: RefCell<Value>, space: Option<String>) {
        if self.values.is_empty() {
            debug_assert!(space.is_none());
            self.values.push_front(val);
        } else {
            self.values.push_front(val);
            self.spaces.push_front(space);
        }
        debug_assert_eq!(self.values.len(), self.spaces.len() + 1);
    }

    pub(crate) fn get_values(&self) -> &VecDeque<RefCell<Value>> {
        &self.values
    }

    pub(crate) fn get_spaces(&self) -> &VecDeque<Option<String>> {
        &self.spaces
    }

    pub(crate) fn values_mut(
        &mut self,
    ) -> std::collections::vec_deque::IterMut<'_, RefCell<Value>> {
        self.values.iter_mut()
    }

    pub(crate) fn into_inner(self) -> (VecDeque<RefCell<Value>>, VecDeque<Option<String>>) {
        (self.values, self.spaces)
    }

    pub(crate) fn try_resolve(mut self, path: &RefPath) -> Result<Value> {
        if self.values.is_empty() {
            Ok(Value::None)
        } else if self.values.len() == 1 {
            let (_, v) = self.pop_back().unwrap();
            Ok(v.into_inner())
        } else {
            let (first, first_space) = self.pop_front().unwrap();
            let mut space = first_space;
            let mut first = first.into_inner();
            while let Some((second, second_space)) = self.pop_front() {
                first = Value::concatenate(path, first, space, second.into_inner())?;
                space = second_space;
            }
            Ok(first)
        }
    }
}

impl Display for Concat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Concat(")?;
        join_format(
            self.values.iter(),
            f,
            |f| write!(f, ", "),
            |f, v| write!(f, "{}", v.borrow()),
        )?;
        write!(f, ")")?;
        Ok(())
    }
}
