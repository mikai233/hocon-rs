use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::Display,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crate::{
    merge::vlaue::Value,
    path::Path,
    raw::{field::ObjectField, raw_object::RawObject, raw_string::RawString, raw_value::RawValue},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Object {
    Merged(BTreeMap<String, RefCell<Value>>),
    Unmerged(BTreeMap<String, RefCell<Value>>),
}

impl Object {
    pub(crate) fn new(obj: RawObject) -> crate::Result<Self> {
        let mut root = Object::default();
        for ele in obj.0.into_iter() {
            root.put_field(None, ele)?;
        }
        Ok(root)
    }

    fn merge_inclusion(root: &mut Object, parent_path: Option<Path>, obj: RawObject) {}

    fn put_field(&mut self, parent_path: Option<Path>, field: ObjectField) -> crate::Result<()> {
        match field {
            ObjectField::Inclusion { inclusion, .. } => {
                if let Some(include_obj) = inclusion.val {
                    let mut include_obj = Self::new(*include_obj)?;
                    include_obj.fixup_substitution(parent_path.as_ref())?;
                    self.merge(include_obj)?;
                }
            }
            ObjectField::KeyValue { key, value, .. } => self.put_kv(parent_path, key, value)?,
            ObjectField::NewlineComment(_) => {}
        }
        Ok(())
    }

    fn put_kv(
        &mut self,
        mut parent_path: Option<Path>,
        key: RawString,
        value: RawValue,
    ) -> crate::Result<()> {
        let sub_path = Path::from_iter(key.as_path().iter());
        match &mut parent_path {
            Some(p) => {
                if let Some(sub_path) = sub_path {
                    p.push_back(sub_path);
                }
            }
            None => parent_path = sub_path,
        }
        let mut expanded_obj = Self::new_obj_from_path(&key.as_path(), value.into())?;
        expanded_obj.fixup_substitution(parent_path.as_ref())?;
        self.merge(expanded_obj)?;
        Ok(())
    }

    pub(crate) fn merge(&mut self, other: Self) -> crate::Result<()> {
        let both_merged = self.is_merged() && other.is_merged();
        let other: BTreeMap<String, RefCell<Value>> = other.into();
        for (k, v_right) in other {
            match self.get_mut(&k) {
                Some(v_left) => match (v_left.get_mut(), v_right.into_inner()) {
                    (Value::Object(left_obj), Value::Object(right_obj)) => {
                        left_obj.merge(right_obj)?;
                    }
                    (l, r) => {
                        let left = std::mem::take(l);
                        // Even if the value ends up merged after replacement,
                        // we still treat it as unmerged, to avoid complicating the merge-check logic.
                        *l = Value::replacement(left, r)?;
                    }
                },
                None => {
                    self.insert(k, v_right);
                }
            }
        }
        if !both_merged {
            self.as_unmerged();
        }
        Ok(())
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        let mut all_merged = false;
        for val in self.values_mut() {
            let val = val.get_mut();
            if !val.is_merged() {
                all_merged = false;
                break;
            }
            if let Value::Object(obj) = val
                && !obj.try_become_merged()
            {
                all_merged = false;
                break;
            }
        }
        if all_merged {
            self.as_merged();
        }
        all_merged
    }

    pub(crate) fn into_merged(self) -> Self {
        Self::Merged(self.into())
    }

    pub(crate) fn as_merged(&mut self) {
        let obj = std::mem::take(self.deref_mut());
        *self = Self::Merged(obj);
    }

    pub(crate) fn into_unmerged(self) -> Self {
        Self::Unmerged(self.into())
    }

    pub(crate) fn as_unmerged(&mut self) {
        let obj = std::mem::take(self.deref_mut());
        *self = Self::Unmerged(obj);
    }

    pub(crate) fn is_merged(&self) -> bool {
        matches!(self, Self::Merged(_))
    }

    pub(crate) fn is_unmerged(&self) -> bool {
        matches!(self, Self::Unmerged(_))
    }

    fn new_obj_from_path(path: &[&str], value: Value) -> crate::Result<Object> {
        if path.is_empty() {
            return Err(crate::error::Error::InvalidPathExpression("empty"));
        }
        let mut current = value;
        for ele in path.iter().rev() {
            let mut obj = Object::default();
            obj.insert(ele.to_string(), RefCell::new(current));
            current = Value::object(obj);
        }
        if let Value::Object(obj) = current {
            return Ok(obj);
        } else {
            unreachable!("`current` should always be Object")
        }
    }

    fn fixup_substitution(&mut self, parent_path: Option<&Path>) -> crate::Result<()> {
        if let Some(path) = parent_path {
            for (_, val) in self.iter_mut() {
                let val = val.get_mut();
                match val {
                    Value::Object(obj) => {
                        obj.fixup_substitution(Some(path))?;
                    }
                    Value::Array(array) => {
                        for ele in array.iter_mut() {
                            if let Value::Object(obj) = ele {
                                obj.fixup_substitution(Some(path))?;
                            }
                        }
                    }
                    Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
                    Value::Substitution(substitution) => {
                        let mut fixed_path = path.clone();
                        let mut temp = Path::new("".to_string(), None);
                        std::mem::swap(&mut temp, &mut substitution.path);
                        fixed_path.push_back(temp);
                        substitution.path = fixed_path;
                    }
                    Value::Concat(concat) => {
                        for ele in concat.iter_mut() {
                            if let Value::Object(obj) = ele {
                                obj.fixup_substitution(Some(path))?;
                            }
                        }
                    }
                    Value::AddAssign(add_assign) => {
                        if let Value::Object(obj) = &mut ***add_assign {
                            obj.fixup_substitution(Some(path))?;
                        }
                    }
                    Value::DelayMerge(delay_merge) => {
                        for ele in delay_merge.iter_mut() {
                            if let Value::Object(obj) = ele {
                                obj.fixup_substitution(Some(path))?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn substitute(root: &Object, current: &Value) -> crate::Result<()> {
        match current {
            Value::Object(object) => {
                if object.is_unmerged() {
                    for val in object.values() {
                        Self::substitute(root, &*val.borrow())?;
                    }
                }
            }
            Value::Array(array) => {
                for ele in array.iter() {
                    Self::substitute(root, ele)?;
                }
            }
            Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
            Value::Substitution(substitution) => todo!(),
            Value::Concat(concat) => {
                for ele in concat.iter() {
                    Self::substitute(root, ele)?;
                }
                let val = concat.clone().reslove()?;
            }
            Value::AddAssign(add_assign) => todo!(),
            Value::DelayMerge(delay_merge) => todo!(),
        }
        Ok(())
    }
}

impl TryInto<Object> for RawObject {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<Object, Self::Error> {
        Object::new(self)
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

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let last_index = self.len() - 1;
        for (index, (k, v)) in self.iter().enumerate() {
            write!(f, "{} : {}", k, v.borrow())?;
            if index != last_index {
                write!(f, ", ")?;
            }
        }
        write!(f, "}}")?;
        Ok(())
    }
}
