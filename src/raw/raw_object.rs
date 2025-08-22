use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use crate::{path::Path, value::Value};
use derive_more::{Constructor, Deref, DerefMut};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Deref, DerefMut, Constructor)]
pub struct RawObject(pub Vec<ObjectField>);

impl RawObject {
    pub fn from_iter<I>(fields: I) -> Self
    where
        I: IntoIterator<Item = ObjectField>,
    {
        Self(fields.into_iter().collect())
    }

    pub fn key_value<I>(fields: I) -> Self
    where
        I: IntoIterator<Item = (RawString, RawValue)>,
    {
        let kvs = fields
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v));
        Self::from_iter(kvs)
    }

    fn remove_by_path(&mut self, path: &Path) -> Option<ObjectField> {
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
        match remove_index {
            None => None,
            Some(index) => Some(self.remove(index)),
        }
    }

    /// Removes all object fields from the given path, preserving their original
    /// order but reversed relative to the file.
    fn remove_all_by_path(&mut self, path: &Path) -> Vec<ObjectField> {
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

    fn get_by_path(&self, path: &Path) -> Option<&RawValue> {
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

    fn get_by_path_mut(&mut self, path: &Path) -> Option<&mut RawValue> {
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

    pub(crate) fn merge(mut left: Self, right: Self) -> Self {
        left.0.extend(right.0);
        left
    }
}

impl Display for RawObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let joined = self.iter().map(|v| format!("{}", v)).join(", ");
        write!(f, "{{{}}}", joined)
    }
}

// TODO make sure the key is valid
impl From<Vec<(String, RawValue)>> for RawObject {
    fn from(value: Vec<(String, RawValue)>) -> Self {
        let fields = value
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(RawString::QuotedString(k), v));
        Self::from_iter(fields)
    }
}

impl Into<RawValue> for Value {
    fn into(self) -> RawValue {
        match self {
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
            Value::Array(array) => RawValue::array(array.into_iter().map(Into::into)),
            Value::Boolean(boolean) => RawValue::Boolean(boolean),
            Value::Null => RawValue::Null,
            Value::String(string) => RawValue::String(string.into()),
            Value::Number(number) => RawValue::Number(number),
        }
    }
}
