use crate::{join, raw::raw_value::RawValue};
use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::Display;

/// Represents a raw HOCON array structure before any semantic resolution.
///
/// `RawArray` corresponds to an array literal in a HOCON document, preserving the
/// original order of elements, comments, and layout information (if present in
/// the surrounding data model).
///
/// Each element inside the array is represented as a [`RawValue`], which may itself
/// be a primitive, object, array, or substitution expression.
///
/// This structure is a low-level syntax node (before evaluation or merging),
/// mainly used during parsing or intermediate transformations.
///
/// # Example
///
/// ```hocon
/// a = [1, 2, ${x}, { b: 3 }]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut, Constructor)]
pub struct RawArray(pub Vec<RawValue>);

impl RawArray {
    /// Consumes the `RawArray` and returns the inner vector of [`RawValue`] elements.
    #[inline]
    pub fn into_inner(self) -> Vec<RawValue> {
        self.0
    }
}

impl Display for RawArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        join(self.iter(), ", ", f)?;
        write!(f, "]")?;
        Ok(())
    }
}
