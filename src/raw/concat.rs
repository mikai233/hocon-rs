use crate::{error::Error, join, raw::raw_value::RawValue};
use std::fmt::Display;

/// Represents a concatenation of multiple HOCON values.
///
/// In HOCON, values can be concatenated implicitly (without explicit separators),
/// e.g., `"hello" "world"` becomes a concatenated value. This struct stores the
/// sequence of values along with optional spaces (or separators) between them.
///
/// # Fields
/// - `values`: The list of HOCON values being concatenated.
/// - `spaces`: Optional string fragments representing spaces between values.
///   `spaces.len() + 1` must equal `values.len()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Concat {
    values: Vec<RawValue>,
    spaces: Vec<Option<String>>,
}

impl Concat {
    /// Creates a new `Concat` instance.
    ///
    /// # Arguments
    /// * `values` - A vector of `RawValue` elements to be concatenated.
    /// * `spaces` - A vector of optional strings representing spaces between values.
    ///
    /// # Errors
    /// Returns `Error::InvalidConcat` if `values.len() != spaces.len() + 1`.
    /// Returns `Error::InvalidValue` if any value is a nested `Concat` or `AddAssign`,
    /// which are not allowed within a concatenation.
    pub fn new(values: Vec<RawValue>, spaces: Vec<Option<String>>) -> crate::Result<Self> {
        if values.len() != spaces.len() + 1 {
            return Err(Error::InvalidConcat(values.len(), spaces.len()));
        }
        let concat = Self { values, spaces };
        for v in &concat.values {
            if matches!(v, RawValue::Concat(_)) || matches!(v, RawValue::AddAssign(_)) {
                return Err(Error::InvalidValue {
                    val: v.ty(),
                    ty: "concat",
                });
            }
        }
        Ok(concat)
    }

    /// Consumes the `Concat` and returns its internal vectors.
    ///
    /// Returns a tuple `(values, spaces)`.
    pub fn into_inner(self) -> (Vec<RawValue>, Vec<Option<String>>) {
        (self.values, self.spaces)
    }

    /// Returns a reference to the vector of concatenated values.
    pub fn get_values(&self) -> &Vec<RawValue> {
        &self.values
    }

    /// Returns a reference to the vector of optional spaces between values.
    pub fn get_spaces(&self) -> &Vec<Option<String>> {
        &self.spaces
    }
}

impl Display for Concat {
    /// Formats the concatenated values as a string.
    ///
    /// Uses the `join` function to output all `values` separated by a single space.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        join(self.values.iter(), " ", f)
    }
}
