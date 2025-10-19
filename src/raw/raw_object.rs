use crate::join;
use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::{path::Path, value::Value};
use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::{Display, Formatter};

/// Represents a raw HOCON object, which is a collection of fields
/// such as key-value pairs, include statements, and comments.
///
/// In HOCON, an object corresponds to `{}` blocks or top-level
/// configurations, and may contain nested objects, arrays, or primitive values.
///
/// Example:
/// ```hocon
/// {
///   include "common.conf"
///   host = "localhost"
///   port = 8080
///   # This is a comment
/// }
/// ```
///
/// This structure is a **direct representation of the parsed syntax**,
/// not yet evaluated or merged. It preserves comments and inclusions
/// for later processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deref, DerefMut, Constructor)]
pub struct RawObject(pub Vec<ObjectField>);

impl RawObject {
    /// Consumes the object and returns the inner vector of [`ObjectField`]s.
    ///
    /// This is typically used when transferring ownership of the internal fields
    /// for further processing or transformation.
    pub fn into_inner(self) -> Vec<ObjectField> {
        self.0
    }

    /// Constructs a new [`RawObject`] from a list of key-value pairs.
    ///
    /// Each `(RawString, RawValue)` pair is automatically wrapped into
    /// a [`ObjectField::KeyValue`] variant.
    ///
    /// # Example
    /// ```
    /// use hocon_rs::raw::raw_object::RawObject;
    /// use hocon_rs::raw::raw_string::RawString;
    /// use hocon_rs::raw::raw_value::RawValue;
    /// let obj = RawObject::from_entries(vec![
    ///     (RawString::unquoted("a"), RawValue::boolean(true)),
    ///     (RawString::unquoted("b"), RawValue::number(1)),
    /// ]);
    /// ```
    pub fn from_entries(entries: Vec<(RawString, RawValue)>) -> Self {
        let fields = entries
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        Self::new(fields)
    }

    /// Removes the first field in the object (searching from the end of the list)
    /// that matches the given [`Path`].
    ///
    /// - The search order is **reverse**, matching HOCON’s semantics where later
    ///   fields override earlier ones.
    /// - If a nested object or included object contains the target path, removal
    ///   is delegated recursively.
    ///
    /// Returns the removed [`ObjectField`] if found.
    pub fn remove_by_path(&mut self, path: &Path) -> Option<ObjectField> {
        let mut remove_index = None;
        for (index, field) in self.iter_mut().enumerate().rev() {
            match field {
                ObjectField::Inclusion { inclusion, .. } => {
                    if let Some(obj) = &mut inclusion.val {
                        return obj.remove_by_path(path);
                    }
                }
                ObjectField::KeyValue { key, value, .. } => {
                    let k = &key.as_path();
                    if path.starts_with1(k) {
                        match path.sub_path(k.len()) {
                            None => {
                                remove_index = Some(index);
                                break;
                            }
                            Some(sub_path) => {
                                if let RawValue::Object(obj) = value {
                                    return obj.remove_by_path(sub_path);
                                }
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        remove_index.map(|index| self.remove(index))
    }

    /// Removes **all** fields that match the given [`Path`] from this object and its nested objects.
    ///
    /// - Returns a list of removed [`ObjectField`]s, preserving their original order
    ///   (but in reverse relative to the file, consistent with HOCON’s “last one wins” semantics).
    /// - Recursively descends into nested and included objects.
    pub fn remove_all_by_path(&mut self, path: &Path) -> Vec<ObjectField> {
        let mut results = vec![];
        let mut remove_indices = vec![]; // These indices are stored in reverse order
        for (index, field) in self.iter_mut().enumerate().rev() {
            match field {
                ObjectField::Inclusion { inclusion, .. } => {
                    if let Some(obj) = &mut inclusion.val {
                        results.extend(obj.remove_all_by_path(path));
                    }
                }
                ObjectField::KeyValue { key, value, .. } => {
                    let k = &key.as_path();
                    if path.starts_with1(k) {
                        match path.sub_path(k.len()) {
                            None => {
                                remove_indices.push(index);
                            }
                            Some(sub_path) => {
                                if let RawValue::Object(obj) = value {
                                    results.extend(obj.remove_all_by_path(sub_path));
                                }
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        for idx in remove_indices {
            results.push(self.remove(idx));
        }
        results
    }

    /// Retrieves a reference to a [`RawValue`] located at the specified [`Path`].
    ///
    /// - Returns `None` if no matching field exists.
    /// - Traverses nested objects and inclusions recursively.
    /// - Later fields shadow earlier ones (reverse search order).
    pub fn get_by_path(&self, path: &Path) -> Option<&RawValue> {
        for field in self.iter().rev() {
            match field {
                ObjectField::Inclusion { inclusion, .. } => {
                    if let Some(obj) = &inclusion.val {
                        return obj.get_by_path(path);
                    }
                }
                ObjectField::KeyValue { key, value, .. } => {
                    let k = &key.as_path();
                    if path.starts_with1(k) {
                        match path.sub_path(k.len()) {
                            None => return Some(value),
                            Some(sub_path) => {
                                if let RawValue::Object(obj) = value {
                                    return obj.get_by_path(sub_path);
                                }
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        None
    }

    /// Retrieves a mutable reference to a [`RawValue`] located at the specified [`Path`].
    ///
    /// Behaves like [`get_by_path`], but allows in-place modification of the found value.
    pub fn get_by_path_mut(&mut self, path: &Path) -> Option<&mut RawValue> {
        for field in self.iter_mut().rev() {
            match field {
                ObjectField::Inclusion { inclusion, .. } => {
                    if let Some(obj) = &mut inclusion.val {
                        return obj.get_by_path_mut(path);
                    }
                }
                ObjectField::KeyValue { key, value, .. } => {
                    let k = &key.as_path();
                    if path.starts_with1(k) {
                        match path.sub_path(k.len()) {
                            None => return Some(value),
                            Some(sub_path) => {
                                if let RawValue::Object(obj) = value {
                                    return obj.get_by_path_mut(sub_path);
                                }
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        None
    }

    /// Merges two [`RawObject`]s into one.
    ///
    /// - If both objects contain the same key, the field from `right` takes precedence
    ///   and overwrites the one from `left`.
    /// - Fields that exist only in `left` are preserved.
    /// - This behavior follows **HOCON’s rule**: *later definitions override earlier ones*.
    ///
    /// Note: The merge is **syntactic** (on raw fields), not semantic.
    /// Actual resolution (e.g., substitutions, includes) occurs in later phases.
    pub(crate) fn merge(mut left: Self, right: Self) -> Self {
        left.0.extend(right.0);
        left
    }
}

impl Display for RawObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        join(self.iter(), ", ", f)?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl From<Value> for RawValue {
    fn from(val: Value) -> Self {
        match val {
            Value::Object(object) => {
                let len = object.len();
                let fields =
                    object
                        .into_iter()
                        .fold(Vec::with_capacity(len), |mut acc, (key, value)| {
                            let field = ObjectField::key_value(key, value);
                            acc.push(field);
                            acc
                        });
                RawValue::Object(RawObject::new(fields))
            }
            Value::Array(array) => RawValue::array(array.into_iter().map(Into::into).collect()),
            Value::Boolean(boolean) => RawValue::Boolean(boolean),
            Value::Null => RawValue::Null,
            Value::String(string) => RawValue::String(string.into()),
            Value::Number(number) => RawValue::Number(number),
        }
    }
}
