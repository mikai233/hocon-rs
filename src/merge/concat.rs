use std::collections::VecDeque;
use std::{cell::RefCell, fmt::Display};

use crate::error::Error;
use crate::merge::{path::RefPath, value::Value};
use crate::{Result, join_format};

/// Represents a concatenation of evaluated HOCON values during the merge phase.
///
/// This structure is derived from the `raw::concat::Concat` definition but uses
/// `Value` instead of `RawValue`. It allows deferred concatenation where
/// intermediate `Value`s may depend on unresolved substitutions or nested merges.
///
/// The concatenation logic here ensures that the final merged `Value` reflects
/// the semantics of implicit string/value concatenation in HOCON.
///
/// # Fields
/// - `values`: A queue of `Value` references (wrapped in `RefCell` to allow in-place modification).
/// - `spaces`: A queue of optional whitespace strings separating each value.
///   The invariant `values.len() == spaces.len() + 1` must always hold.
///
/// # Example
/// ```hocon
/// a = "hello"
/// b = ${a}" world"
/// ```
/// When resolved, `b` becomes a `Concat` of two `Value`s, which will be
/// concatenated into `"hello world"` after variable substitution.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Concat {
    values: VecDeque<RefCell<Value>>,
    spaces: VecDeque<Option<String>>,
}

impl Concat {
    /// Creates a new `Concat` instance with pre-validated values and spaces.
    ///
    /// # Errors
    /// Returns `Error::InvalidConcat` if the invariant `values.len() != spaces.len() + 1` is violated.
    pub(crate) fn new(
        values: VecDeque<RefCell<Value>>,
        spaces: VecDeque<Option<String>>,
    ) -> Result<Self> {
        if values.len() != spaces.len() + 1 {
            return Err(Error::InvalidConcat(values.len(), spaces.len()));
        }
        Ok(Self { values, spaces })
    }

    /// Constructs a minimal `Concat` with exactly two values and one optional space.
    pub(crate) fn two(left: Value, space: Option<String>, right: Value) -> Self {
        let values = VecDeque::from_iter([RefCell::new(left), RefCell::new(right)]);
        let spaces = VecDeque::from_iter([space]);
        Self { values, spaces }
    }

    /// Converts a raw concatenation (`raw::concat::Concat`) into a merge-time `Concat`.
    ///
    /// Each `RawValue` is transformed into a `Value` through `Value::from_raw`,
    /// preserving the space information between them.
    ///
    /// # Arguments
    /// * `parent` — The parent reference path in the configuration tree.
    /// * `raw` — The raw concatenation structure parsed from HOCON input.
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

    /// Appends a new value and its preceding space to the end of the concatenation.
    ///
    /// Maintains the invariant `values.len() == spaces.len() + 1`.
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

    /// Removes and returns the last value with its preceding space (if any).
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

    /// Removes and returns the first value with its following space (if any).
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

    /// Inserts a new value and its following space at the beginning of the concatenation.
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

    /// Returns the number of concatenated values.
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns a reference to all concatenated values.
    pub(crate) fn get_values(&self) -> &VecDeque<RefCell<Value>> {
        &self.values
    }

    /// Returns a mutable iterator over all value cells.
    pub(crate) fn values_mut(
        &mut self,
    ) -> std::collections::vec_deque::IterMut<'_, RefCell<Value>> {
        self.values.iter_mut()
    }

    /// Attempts to resolve the concatenation into a single `Value`.
    ///
    /// If the `Concat` contains:
    /// - **0 values** → returns `Value::None`
    /// - **1 value** → returns that single value directly
    /// - **multiple values** → iteratively concatenates them using
    ///   `Value::concatenate`, preserving spaces between each.
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
    /// Formats this `Concat` for debugging or display.
    ///
    /// Outputs the structure in a `Concat(v1, v2, v3, …)` format.
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
