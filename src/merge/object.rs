use std::{
    cell::RefCell,
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use crate::{
    merge::vlaue::Value,
    path::Path,
    raw::{raw_object::RawObject, raw_string::RawString, raw_value::RawValue},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Object {
    Merged(BTreeMap<String, RefCell<Value>>),
    Unmerged(BTreeMap<String, RefCell<Value>>),
}

impl Object {
    pub(crate) fn new(obj: RawObject) -> Self {
        let mut root = Object::default();
        for ele in obj.0.into_iter() {
            let obj = match ele {
                crate::raw::field::ObjectField::Inclusion { inclusion, .. } => {
                    if let Some(obj) = inclusion.val {
                        Self::merge_inclusion(&mut root, None, *obj);
                    }
                }
                crate::raw::field::ObjectField::KeyValue { key, value, .. } => {
                    Self::merge_key_value(&mut root, None, key, value);
                }
                crate::raw::field::ObjectField::NewlineComment(comment) => {}
            };
        }
        todo!()
    }

    fn merge_inclusion(root: &mut Object, parent_path: Option<Path>, obj: RawObject) {}

    fn merge_key_value(
        root: &mut Object,
        parent_path: Option<Path>,
        key: RawString,
        value: RawValue,
    ) {
        let mut current = root;
    }

    pub(crate) fn merge(&mut self, other: Self) -> crate::Result<()> {
        let other: BTreeMap<String, RefCell<Value>> = other.into();
        for (k, v_right) in other {
            match self.get_mut(&k) {
                Some(v_left) => match (v_left.get_mut(), v_right.into_inner()) {
                    (Value::Object(left_obj), Value::Object(right_obj)) => {
                        left_obj.merge(right_obj)?;
                    }
                    (l, r) => {
                        let left = std::mem::take(l);
                        *l = Value::replacement(left, r)?;
                    }
                },
                None => {
                    self.insert(k, v_right);
                }
            }
        }
        Ok(())
    }
}

impl From<RawObject> for Object {
    fn from(value: RawObject) -> Self {
        todo!()
    }
}

impl Default for Object {
    fn default() -> Self {
        Object::Unmerged(BTreeMap::new())
    }
}

impl Deref for Object {
    type Target = BTreeMap<String, RefCell<Value>>;

    fn deref(&self) -> &Self::Target {
        match self {
            Object::Merged(obj) | Object::Unmerged(obj) => obj,
        }
    }
}

impl DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Object::Merged(obj) | Object::Unmerged(obj) => obj,
        }
    }
}

impl Into<BTreeMap<String, RefCell<Value>>> for Object {
    fn into(self) -> BTreeMap<String, RefCell<Value>> {
        match self {
            Object::Merged(object) | Object::Unmerged(object) => object,
        }
    }
}
