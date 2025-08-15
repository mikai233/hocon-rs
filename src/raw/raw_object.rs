use crate::path::Path;
use crate::raw::field::ObjectField;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use itertools::Itertools;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq)]
pub enum RawObject {
    Merged(Vec<ObjectField>),
    Unmerged(Vec<ObjectField>),
    MergedSubstitution {
        path: Vec<Path>,
        fields: Vec<ObjectField>,
    },
    UnmergedSubstitution {
        path: Vec<Path>,
        fields: Vec<ObjectField>,
    },
}

impl RawObject {
    pub fn new<I>(fields: I) -> Self
    where
        I: IntoIterator<Item=ObjectField>,
    {
        Self::Unmerged(fields.into_iter().collect())
    }

    pub fn key_value<I>(fields: I) -> Self
    where
        I: IntoIterator<Item=(RawString, RawValue)>,
    {
        let kvs = fields
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        Self::Unmerged(kvs)
    }

    pub fn merge_object(o1: Self, o2: Self, path: &Path) -> Self {
        let mut substitutions = vec![];
        let mut fields = Vec::with_capacity(o1.len() + o2.len());
        let mut extract = |o: RawObject| {
            match o {
                RawObject::Merged(v) |
                RawObject::Unmerged(v) => {
                    fields.extend(v);
                }
                RawObject::MergedSubstitution { path, fields: f } |
                RawObject::UnmergedSubstitution { path, fields: f } => {
                    substitutions.extend(path);
                    fields.extend(f);
                }
            }
        };
        extract(o1);
        extract(o2);
        let object = RawObject::UnmergedSubstitution {
            path: substitutions,
            fields,
        };
        object.merge(path)
    }

    pub fn merge(self, path: &Path) -> RawObject {
        unimplemented!()
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
                            Some(sub_path) => if let RawValue::Object(obj) = value {
                                return obj.remove_by_path(sub_path)
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        match remove_index {
            None => None,
            Some(index) => {
                Some(self.remove(index))
            }
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
                            Some(sub_path) => if let RawValue::Object(obj) = value {
                                results.extend(obj.remove_all_by_path(sub_path));
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
                            None => {
                                return Some(value)
                            }
                            Some(sub_path) => if let RawValue::Object(obj) = value {
                                return obj.get_by_path(sub_path)
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
                            None => {
                                return Some(value)
                            }
                            Some(sub_path) => if let RawValue::Object(obj) = value {
                                return obj.get_by_path_mut(sub_path)
                            }
                        }
                    }
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        None
    }

    pub fn to_merged(self) -> Self {
        match self {
            RawObject::Merged(v) |
            RawObject::Unmerged(v) => RawObject::Merged(v),
            RawObject::MergedSubstitution { path, fields } |
            RawObject::UnmergedSubstitution { path, fields } => RawObject::MergedSubstitution { path, fields },
        }
    }

    pub fn to_unmerged(self) -> Self {
        match self {
            RawObject::Merged(v) |
            RawObject::Unmerged(v) => RawObject::Unmerged(v),
            RawObject::MergedSubstitution { path, fields } |
            RawObject::UnmergedSubstitution { path, fields } => RawObject::UnmergedSubstitution { path, fields },
        }
    }

    pub fn to_substitution(self) -> Self {
        match self {
            RawObject::Merged(v) => RawObject::MergedSubstitution {
                path: vec![],
                fields: v,
            },
            RawObject::Unmerged(v) => RawObject::UnmergedSubstitution {
                path: vec![],
                fields: v,
            },
            _ => self,
        }
    }

    pub fn is_merged(&self) -> bool {
        matches!(self, RawObject::Merged(_))
    }

    pub fn is_unmerged(&self) -> bool {
        matches!(self, RawObject::Unmerged(_))
    }

    pub fn is_merged_substitution(&self) -> bool {
        matches!(self, RawObject::MergedSubstitution{..})
    }

    pub fn is_unmerged_substitution(&self) -> bool {
        matches!(self, RawObject::UnmergedSubstitution{..})
    }
}

impl Default for RawObject {
    fn default() -> Self {
        RawObject::Unmerged(vec![])
    }
}

impl Deref for RawObject {
    type Target = Vec<ObjectField>;

    fn deref(&self) -> &Self::Target {
        match self {
            RawObject::Merged(o) |
            RawObject::Unmerged(o) |
            RawObject::MergedSubstitution { fields: o, .. } |
            RawObject::UnmergedSubstitution { fields: o, .. } => o,
        }
    }
}

impl DerefMut for RawObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            RawObject::Merged(o) |
            RawObject::Unmerged(o) |
            RawObject::MergedSubstitution { fields: o, .. } |
            RawObject::UnmergedSubstitution { fields: o, .. } => o,
        }
    }
}

impl Display for RawObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let joined = self.iter()
            .map(|v| format!("{}", v))
            .join(", ");
        write!(f, "{{{}}}", joined)
    }
}

// TODO make sure the key is valid
impl From<Vec<(String, RawValue)>> for RawObject {
    fn from(value: Vec<(String, RawValue)>) -> Self {
        let fields = value.into_iter().map(|(k, v)| ObjectField::key_value(RawString::QuotedString(k), v)).collect();
        Self::Unmerged(fields)
    }
}
