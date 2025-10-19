use derive_more::{Constructor, Deref, DerefMut};

use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

/// Represents an additive assignment (`+=`) expression in a HOCON document.
///
/// In HOCON, a key can use `+=` to *append* values to an existing key instead of
/// overwriting it. This is particularly useful when merging arrays or concatenating
/// strings.
///
/// For example:
///
/// ```hocon
/// path = ["/usr/lib"]
/// path += ["/usr/local/lib"]
/// ```
///
/// After resolution, the final value of `path` would be:
///
/// ```hocon
/// path = ["/usr/lib", "/usr/local/lib"]
/// ```
///
/// Internally, `AddAssign` wraps a [`RawValue`] — the right-hand side of the
/// additive assignment. The surrounding parser or evaluator is responsible for
/// resolving it by combining it with the previous value of the same key.
///
/// # Notes
///
/// - `AddAssign` does **not** perform merging itself; it only represents the
///   syntactic form of the `+=` operator.
/// - It is primarily used during parsing and intermediate representation phases,
///   before the final HOCON resolution and substitution steps.
///
/// # Example
///
/// ```rust
/// use hocon_rs::raw::{raw_value::RawValue, raw_array::RawArray, add_assign::AddAssign};
/// // Represents: key += [1, 2, 3]
/// let val = RawValue::Array(RawArray::new(vec![
///     RawValue::Number(1.into()),
///     RawValue::Number(2.into()),
///     RawValue::Number(3.into()),
/// ]));
/// let add_assign = AddAssign::new(Box::new(val));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut, Constructor)]
pub struct AddAssign(Box<RawValue>);

impl Display for AddAssign {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AddAssign> for RawValue {
    fn from(val: AddAssign) -> Self {
        *val.0
    }
}
