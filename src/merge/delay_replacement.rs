use std::{cell::RefCell, collections::VecDeque, fmt::Display};

use derive_more::{Constructor, Deref, DerefMut};

use crate::merge::value::Value;

/// A container for values that cannot be immediately merged during a replacement operation.
///
/// When merging HOCON values, a substitution expression (`${...}`) might be encountered.
/// Because the final value of this substitution is unknown until the entire configuration
/// has been parsed, we store these pending values in a `DelayReplacement` struct.
///
/// This structure holds a queue of values that need to be merged later, once all
/// substitutions have been resolved. The final merge result is uncertain until then,
/// as it depends on whether the substituted value is a simple type or an object.
///
#[derive(Debug, Clone, PartialEq, Deref, DerefMut, Constructor)]
pub(crate) struct DelayReplacement(pub(crate) VecDeque<RefCell<Value>>);

impl DelayReplacement {
    pub(crate) fn from_iter<I>(value: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        Self::new(value.into_iter().map(|v| RefCell::new(v)).collect())
    }

    pub(crate) fn into_values(self) -> VecDeque<RefCell<Value>> {
        self.0
    }
}

impl DelayReplacement {
    pub(crate) fn flatten(self) -> Self {
        let mut values = VecDeque::new();
        for val in self.into_values() {
            let val = val.into_inner();
            if let Value::DelayReplacement(de) = val {
                values.extend(de.flatten().into_values());
            } else {
                values.push_back(RefCell::new(val));
            }
        }
        Self::new(values)
    }
}

impl Display for DelayReplacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DelayReplacement(")?;
        let last_index = self.len().saturating_sub(1);
        for (index, ele) in self.iter().enumerate() {
            write!(f, "{}", ele.borrow())?;
            if index != last_index {
                write!(f, ", ")?;
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}
