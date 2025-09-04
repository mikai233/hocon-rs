use crate::join;
use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::{path::Path, value::Value};
use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deref, DerefMut, Constructor)]
pub struct RawObject(pub Vec<ObjectField>);

impl RawObject {
    pub fn from_entries<I>(entries: Vec<(RawString, RawValue)>) -> Self
    where
        I: IntoIterator<Item = (RawString, RawValue)>,
    {
        let fields = entries
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        Self::new(fields)
    }

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

    /// Removes all object fields from the given path, preserving their original
    /// order but reversed relative to the file.
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

    /// Merges two `RawObject`s into one.
    ///
    /// - If both objects contain the same key, the field from `right` takes precedence
    ///   and overwrites the one from `left`.
    /// - Fields that only exist in `left` are preserved.
    /// - This follows HOCON’s rule that later definitions of the same key override
    ///   earlier ones.
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
