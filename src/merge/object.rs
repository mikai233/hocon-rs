use tracing::{enabled, instrument, span, trace, Level};

use crate::{
    merge::{add_assign::AddAssign, path::RefPath, value::Value},
    path::Path,
    raw::{field::ObjectField, raw_object::RawObject, raw_string::RawString, raw_value::RawValue},
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::Display,
    ops::{Deref, DerefMut},
};

type V = RefCell<Value>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Object {
    Merged(BTreeMap<String, V>),
    Unmerged(BTreeMap<String, V>),
}

impl Object {
    pub(crate) fn from_raw(parent: Option<&RefPath>, obj: RawObject) -> crate::Result<Self> {
        let mut root = Object::default();
        for field in obj.0.into_iter() {
            root.put_field(parent, field)?;
        }
        Ok(root)
    }

    fn put_field(&mut self, parent: Option<&RefPath>, field: ObjectField) -> crate::Result<()> {
        match field {
            ObjectField::Inclusion { inclusion, .. } => {
                if let Some(include_obj) = inclusion.val {
                    let mut include_obj = Self::from_raw(parent, *include_obj)?;
                    include_obj.fixup_substitution(parent)?;
                    self.merge(include_obj, parent)?;
                }
            }
            ObjectField::KeyValue { key, value, .. } => self.put_kv(parent, key, value)?,
            ObjectField::NewlineComment(_) => {}
        }
        Ok(())
    }

    fn put_kv(
        &mut self,
        parent: Option<&RefPath>,
        key: RawString,
        value: RawValue,
    ) -> crate::Result<()> {
        let key_path = key.as_path();
        let path = match parent {
            Some(parent) => parent.join(RefPath::from_slice(&key_path)?),
            None => RefPath::from_slice(&key_path)?,
        };
        let mut expanded_obj =
            Self::new_obj_from_path(&key_path, Value::from_raw(Some(&path), value)?)?;
        expanded_obj.fixup_substitution(parent)?;
        self.merge(expanded_obj, parent)?;
        Ok(())
    }

    pub(crate) fn merge(&mut self, other: Self, parent: Option<&RefPath>) -> crate::Result<()> {
        let both_merged = self.is_merged() && other.is_merged();
        let other: BTreeMap<String, V> = other.into();
        for (k, v_right) in other {
            let sub_path = match parent {
                None => RefPath::new(&k, None),
                Some(parent_path) => parent_path.join(RefPath::new(&k, None)),
            };
            match self.get_mut(&k) {
                Some(v_left) => match (v_left.get_mut(), v_right.into_inner()) {
                    (Value::Object(left_obj), Value::Object(right_obj)) => {
                        left_obj.merge(right_obj, parent)?;
                    }
                    (l, r) => {
                        let left = std::mem::take(l);
                        // Even if the value ends up merged after replacement,
                        // we still treat it as unmerged, to avoid complicating the merge-check logic.
                        *l = Value::replacement(&sub_path, left, r)?;
                    }
                },
                None => {
                    let v_right = Value::replacement(&sub_path, Value::Null, v_right.into_inner())?;
                    self.insert(k, RefCell::new(v_right));
                    // if let Value::Object(v_right_obj) = v_right {
                    //     let mut obj = Object::default();
                    //     obj.merge(v_right_obj, parent)?;
                    //     self.insert(k, RefCell::new(Value::Object(obj)));
                    // } else {
                    //     self.insert(k, RefCell::new(v_right));
                    // }
                }
            }
        }
        if !both_merged {
            self.as_unmerged();
        } else {
            self.try_become_merged();
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
            trace!("{} become merged", self);
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
        if enabled!(Level::TRACE) {
            trace!("create object from path: `{}` value: `{}`", path.join("."), value);
        }
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

    fn fixup_substitution(&mut self, parent: Option<&RefPath>) -> crate::Result<()> {
        if let Some(parent) = parent {
            for (_, val) in self.iter_mut() {
                match val.get_mut() {
                    Value::Object(obj) => {
                        obj.fixup_substitution(Some(parent))?;
                    }
                    Value::Array(array) => {
                        for ele in array.iter_mut() {
                            if let Value::Object(obj) = ele.get_mut() {
                                obj.fixup_substitution(Some(parent))?;
                            }
                        }
                    }
                    Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
                    Value::Substitution(substitution) => {
                        let mut parent: Path = parent.clone().into();
                        let mut sub = Path::new("".to_string(), None);
                        let mut path = (*substitution.path).clone();
                        std::mem::swap(&mut sub, &mut path);
                        parent.push_back(sub);
                        substitution.path = parent.into();
                    }
                    Value::Concat(concat) => {
                        for ele in concat.iter_mut() {
                            if let Value::Object(obj) = ele.get_mut() {
                                obj.fixup_substitution(Some(parent))?;
                            }
                        }
                    }
                    Value::AddAssign(add_assign) => {
                        if let Value::Object(obj) = &mut ***add_assign {
                            obj.fixup_substitution(Some(parent))?;
                        }
                    }
                    Value::DelayReplacement(delay_replacement) => {
                        for ele in delay_replacement.iter_mut() {
                            if let Value::Object(obj) = ele.get_mut() {
                                obj.fixup_substitution(Some(parent))?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn get_by_path<F>(&self, path: &Path, callback: F) -> crate::Result<bool>
    where
        F: FnOnce(&RefCell<Value>) -> crate::Result<()>,
    {
        fn inner<C>(root: &RefCell<Value>, path: Option<&Path>, callback: C) -> crate::Result<bool>
        where
            C: FnOnce(&RefCell<Value>) -> crate::Result<()>,
        {
            let mut success = false;
            match path {
                Some(path) => match &*root.borrow() {
                    Value::Object(object) => {
                        if let Some(root) = object.get(&path.first) {
                            success = inner(root, path.next(), callback)?;
                        }
                    }
                    _ => {
                        success = false;
                    }
                },
                None => {
                    callback(root)?;
                    success = true;
                }
            }
            Ok(success)
        }
        if let Some(value) = self.get(&path.first) {
            inner(value, path.next(), callback)
        } else {
            Ok(false)
        }
    }

    /// Do not call the borrow_mut of Value across the substitute function, it may cause panic.
    #[instrument(level = Level::TRACE, skip_all, fields(path = %path, vlaue = %value.borrow(), mreged = %value.borrow().is_merged())
    )]
    pub(crate) fn substitute_value(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
    ) -> crate::Result<()> {
        let borrowed = value.borrow();
        if borrowed.is_merged() {
            return Ok(());
        }
        match &*borrowed {
            Value::Object(object) => {
                let span = span!(Level::TRACE, "Object");
                let _enter = span.enter();
                for (key, val) in object.iter() {
                    let sub_path = path.join(RefPath::new(&key, None));
                    self.substitute_value(&sub_path, val)?;
                }
                drop(borrowed);
                value.borrow_mut().try_become_merged();
            }
            Value::Array(array) => {
                let span = span!(Level::TRACE, "Array");
                let _enter = span.enter();
                for (index, ele) in array.iter().enumerate() {
                    //TODO
                    let string_index = index.to_string();
                    let sub_path = path.join(RefPath::new(&string_index, None));
                    self.substitute_value(&sub_path, ele)?;
                }
                drop(borrowed);
            }
            Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {}
            Value::Substitution(substitution) => {
                let span = span!(Level::TRACE, "Substitution");
                let _enter = span.enter();
                let substitution = substitution.clone();
                drop(borrowed);
                trace!("substitute: {}", substitution);
                let success = self.get_by_path(&substitution.path, |target| {
                    if enabled!(Level::TRACE) {
                        trace!("find substitution: {} -> {}", substitution, target.borrow());
                    }
                    if &*substitution.path == path
                        && matches!(&*target.borrow(), Value::Substitution(_))
                    {
                        return if substitution.optional {
                            *target.borrow_mut() = Value::Null;
                            Ok(())
                        } else {
                            Err(crate::error::Error::CycleSubstitution(
                                substitution.to_string(),
                            ))
                        };
                    }
                    self.substitute_value(&RefPath::from(&substitution.path), target)?;
                    let target_clone = target.borrow().clone();
                    if enabled!(Level::TRACE) {
                        trace!("set {} to {}", value.borrow(), target_clone);
                    }
                    *value.borrow_mut() = target_clone;
                    Ok(())
                })?;
                if !success {
                    match std::env::var(substitution.full_path()) {
                        Ok(env_var) => {
                            if enabled!(Level::TRACE) {
                                trace!(
                                    "set environment variable {} to {}",
                                    env_var,
                                    value.borrow()
                                );
                            }
                            *value.borrow_mut() = Value::string(env_var);
                        }
                        Err(_) => {
                            if !substitution.optional {
                                return Err(crate::error::Error::SubstitutionNotFound(
                                    substitution.to_string(),
                                ));
                            } else {
                                *value.borrow_mut() = Value::Null;
                            }
                        }
                    }
                }
            }
            Value::Concat(_) => {
                let span = span!(Level::TRACE, "Concat");
                let _enter = span.enter();
                drop(borrowed);
                fn pop_value(value: &RefCell<Value>) -> Option<RefCell<Value>> {
                    let mut borrowed = value.borrow_mut();
                    let concat = borrowed.as_concat_mut();
                    let popped = concat.pop_back();
                    match &popped {
                        Some(v) => {
                            if enabled!(Level::TRACE) {
                                trace!("popped {} from {}", v.borrow(), concat);
                            }
                        }
                        None => {
                            trace!("popped None from {}", concat);
                        }
                    }
                    popped
                }
                match pop_value(value) {
                    Some(last) => {
                        self.substitute_value(path, &last)?;
                        if matches!(&*value.borrow(), Value::Concat(_)) {
                            match pop_value(value) {
                                Some(second_last) => {
                                    self.substitute_value(path, &second_last)?;
                                    let new_val = Value::concatenate(
                                        path,
                                        second_last.into_inner(),
                                        last.into_inner(),
                                    )?;
                                    let new_val = RefCell::new(new_val);
                                    self.substitute_value(path, &new_val)?;
                                    let mut new_val = new_val.into_inner();
                                    if enabled!(Level::TRACE) {
                                        trace!("set {} to {}", value.borrow(), new_val);
                                    }
                                    new_val.try_become_merged();
                                    *value.borrow_mut() = new_val;
                                }
                                None => {
                                    let mut last = last.into_inner();
                                    last.try_become_merged();
                                    if enabled!(Level::TRACE) {
                                        trace!("set {} to {}", last, value.borrow());
                                    }
                                    *value.borrow_mut() = last;
                                }
                            }
                        } else {
                            let second_last = std::mem::take(&mut *value.borrow_mut());
                            let mut new_val =
                                Value::concatenate(path, second_last, last.into_inner())?;
                            new_val.try_become_merged();
                            if enabled!(Level::TRACE) {
                                trace!("set {} to {}", value.borrow(), new_val);
                            }
                            *value.borrow_mut() = new_val;
                            self.substitute_value(path, value)?;
                        }
                    }
                    None => {
                        if enabled!(Level::TRACE) {
                            trace!("set null to {}", value.borrow());
                        }
                        *value.borrow_mut() = Value::Null;
                    }
                }
            }
            Value::AddAssign(_) => {
                let span = span!(Level::TRACE, "AddAssign");
                let _enter = span.enter();
                drop(borrowed);
                let add_assign = std::mem::take(value.borrow_mut().as_add_assign_mut());
                let v: RefCell<Value> = RefCell::new(add_assign.into());
                self.substitute_value(path, &v)?;
                let mut v = v.into_inner();
                v.try_become_merged();
                let add_assign = AddAssign::new(Box::new(v));
                *value.borrow_mut() = Value::add_assign(add_assign);
            }
            Value::DelayReplacement(_) => {
                let span = span!(Level::TRACE, "DelayReplacement");
                let _enter = span.enter();
                drop(borrowed);
                fn pop_value(value: &RefCell<Value>) -> Option<RefCell<Value>> {
                    let mut borrowed = value.borrow_mut();
                    let de = borrowed.as_delay_replacement_mut();
                    let popped = de.pop_back();
                    match &popped {
                        Some(v) => {
                            if enabled!(Level::TRACE) {
                                trace!("popped {} from {}", v.borrow(), de);
                            }
                        }
                        None => {
                            trace!("popped None from {}", de);
                        }
                    }
                    popped
                }
                match pop_value(value) {
                    Some(last) => {
                        self.substitute_value(path, &last)?;
                        if matches!(&*value.borrow(), Value::DelayReplacement(_)) {
                            match pop_value(value) {
                                Some(second_last) => {
                                    self.substitute_value(path, &second_last)?;
                                    let new_val = Value::replacement(
                                        path,
                                        second_last.into_inner(),
                                        last.into_inner(),
                                    )?;
                                    let new_val = RefCell::new(new_val);
                                    self.substitute_value(path, &new_val)?;
                                    let mut new_val = new_val.into_inner();
                                    new_val.try_become_merged();
                                    if enabled!(Level::TRACE) {
                                        trace!("set {} to {}", value.borrow(), new_val);
                                    }
                                    *value.borrow_mut() = new_val;
                                }
                                None => {
                                    let mut last = last.into_inner();
                                    last.try_become_merged();
                                    if enabled!(Level::TRACE) {
                                        trace!("set {} to {}", last, value.borrow());
                                    }
                                    *value.borrow_mut() = last;
                                }
                            }
                        } else {
                            let second_last = std::mem::take(&mut *value.borrow_mut());
                            let mut new_val =
                                Value::replacement(path, second_last, last.into_inner())?;
                            new_val.try_become_merged();
                            if enabled!(Level::TRACE) {
                                trace!("set {} to {}", value.borrow(), new_val);
                            }
                            *value.borrow_mut() = new_val;
                            self.substitute_value(path, value)?;
                        }
                    }
                    None => {
                        if enabled!(Level::TRACE) {
                            trace!("set null to {}", value.borrow());
                        }
                        *value.borrow_mut() = Value::Null;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn substitute(&self) -> crate::Result<()> {
        for (key, value) in self.iter() {
            let path = RefPath::new(key, None);
            self.substitute_value(&path, value)?;
        }
        Ok(())
    }
}

impl Default for Object {
    fn default() -> Self {
        Object::Unmerged(BTreeMap::new())
    }
}

impl Deref for Object {
    type Target = BTreeMap<String, V>;

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

impl Into<BTreeMap<String, V>> for Object {
    fn into(self) -> BTreeMap<String, V> {
        match self {
            Object::Merged(object) | Object::Unmerged(object) => object,
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let last_index = self.len().saturating_sub(1);
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

#[cfg(test)]
mod tests {
    use ahash::HashMap;
    use serde::{Deserialize, Serialize};
    use tracing::info;

    use crate::from_value;
    use crate::merge::object::Object;
    use crate::parser::{load_conf, parse};

    #[derive(Debug, Serialize, Deserialize)]
    struct Test {
        a: HashMap<String, String>,
        b: Vec<crate::value::Value>,
    }

    #[test]
    fn test_sub() -> crate::Result<()> {
        let conf = load_conf("object6")?;
        let (remainder, object) = parse(conf.as_str()).unwrap();
        info!("raw:{object}");
        let mut obj = Object::from_raw(None, object)?;
        info!("before:{obj}");
        obj.substitute()?;
        info!("after:{obj}");
        let v: crate::value::Value = crate::merge::value::Value::Object(obj).try_into()?;
        let v: Test = from_value(v)?;
        info!("done:{v:?}");
        Ok(())
    }

    #[test]
    fn test_object7() -> crate::Result<()> {
        let conf = load_conf("object7")?;
        let (remainder, object) = parse(conf.as_str()).unwrap();
        info!("raw:{object}");
        let mut obj = Object::from_raw(None, object)?;
        info!("before:{obj}");
        // obj.substitute()?;
        info!("after:{obj}");
        // let v: crate::value::Value = crate::merge::value::Value::Object(obj).try_into()?;
        Ok(())
    }
}
