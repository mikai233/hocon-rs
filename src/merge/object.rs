use tracing::{Level, enabled, instrument, span, trace};

use crate::error::Error;
use crate::merge::array::Array;
use crate::merge::memo::Memo;
use crate::merge::path::RefKey;
use crate::merge::substitution::Substitution;
use crate::path::Key;
use crate::{
    expect_variant,
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

const MAX_SUBSTITUTION_DEPTH: usize = 32;

/// Represents an intermediate state for a HOCON object during parsing and merging.
///
/// This enum distinguishes between two states to optimize the resolution of substitutions:
///
/// - `Merged(BTreeMap<String, V>)`: Indicates that all values within this object and its children
///   have been fully resolved and merged. There are no remaining substitutions, concatenations,
///   or other complex structures that need further processing.
///
/// - `Unmerged(BTreeMap<String, V>)`: Indicates that this object or its children may still
///   contain unresolved values, such as substitutions (`${...}`), concatenations (`Concat`),
///   or additions (`AddAssign`). The resolver must process these pending values before
///   the object is considered complete.
///
/// Separating these states allows the substitution resolver to limit its search to `Unmerged`
/// objects, significantly reducing the scope of traversal and improving performance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Object {
    Merged(BTreeMap<String, V>),
    Unmerged(BTreeMap<String, V>),
}

impl Object {
    pub(crate) fn into_inner(self) -> BTreeMap<String, V> {
        match self {
            Object::Merged(values) | Object::Unmerged(values) => values,
        }
    }

    pub(crate) fn from_raw(parent: Option<&RefPath>, obj: RawObject) -> crate::Result<Self> {
        let mut root = Object::default();
        for field in obj.into_inner().into_iter() {
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
        let expanded_obj =
            Self::new_obj_from_path(&key_path, Value::from_raw(Some(&path), value)?)?;
        self.merge(expanded_obj, parent)?;
        Ok(())
    }

    pub(crate) fn merge(&mut self, other: Self, parent: Option<&RefPath>) -> crate::Result<()> {
        let both_merged = self.is_merged() && other.is_merged();
        let other: BTreeMap<String, V> = other.into();
        for (k, v_right) in other {
            let sub_path = match parent {
                None => RefPath::new(RefKey::Str(&k), None),
                Some(parent_path) => parent_path.join(RefPath::new(RefKey::Str(&k), None)),
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
                        *l = Value::replace(&sub_path, left, r)?;
                        if let Value::Object(obj) = l {
                            obj.resolve_add_assign();
                        }
                    }
                },
                None => {
                    let mut v_right = Value::replace(&sub_path, Value::None, v_right.into_inner())?;
                    if let Value::Object(obj) = &mut v_right {
                        obj.resolve_add_assign();
                    }
                    self.insert(k, RefCell::new(v_right));
                }
            }
        }
        // TODO Can I use try_become_merged directly here?
        if !both_merged {
            self.as_unmerged();
        } else {
            self.try_become_merged();
        }
        Ok(())
    }

    pub(crate) fn resolve_add_assign(&mut self) {
        if self.is_merged() {
            return;
        }
        for v in self.values_mut() {
            v.get_mut().resolve_add_assign();
        }
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        if self.is_merged() {
            return true;
        }
        let all_merged = self.values_mut().all(|v| v.get_mut().try_become_merged());
        if all_merged {
            self.as_merged();
            trace!("{} become merged", self);
        }
        all_merged
    }

    pub(crate) fn as_merged(&mut self) {
        let obj = std::mem::take(self.deref_mut());
        *self = Self::Merged(obj);
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
            trace!(
                "create object from path: `{}` value: `{}`",
                path.join("."),
                value
            );
        }
        if path.is_empty() {
            return Err(Error::InvalidPathExpression("empty"));
        }
        let mut current = value;
        for ele in path.iter().rev() {
            let mut obj = Object::default();
            obj.insert(ele.to_string(), RefCell::new(current));
            current = Value::object(obj);
        }
        if let Value::Object(obj) = current {
            Ok(obj)
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
                    Value::Boolean(_)
                    | Value::Null
                    | Value::None
                    | Value::String(_)
                    | Value::Number(_) => {}
                    Value::Substitution(substitution) => {
                        let mut parent: Path = parent.clone().into();
                        let mut sub = Path::new(Key::String("".to_string()), None);
                        let mut path = (*substitution.path).clone();
                        std::mem::swap(&mut sub, &mut path);
                        parent.push_back(sub);
                        substitution.path = parent.into();
                    }
                    Value::Concat(concat) => {
                        for ele in concat.values_mut() {
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

    /// Traverses the `Value` tree by a given path and executes a callback on the final found value.
    ///
    /// This function recursively searches through nested `Object`s within the `Value` tree
    /// using the provided path expression. Instead of returning the found value directly,
    /// which would violate Rust's borrowing rules due to the `RefCell` wrapper, it invokes
    /// a user-provided callback function on the final value's `RefCell`.
    ///
    /// This approach ensures that the borrow is temporary and isolated to the callback's scope,
    /// allowing safe, recursive traversal without a risk of creating multiple mutable references
    /// to the same data.
    ///
    /// # Arguments
    /// * `path`: The path expression (e.g., `a.b.c`) to navigate the `Value` tree.
    /// * `callback`: A closure that takes a `&RefCell<Value>` and performs an operation. It's called
    ///   only once on the value found at the end of the path.
    ///
    /// # Returns
    /// `Ok(true)` if the value at the given path was found and the callback was successfully executed.
    /// `Ok(false)` if no value was found at the path.
    /// A `crate::Result<()>` on any error during traversal or callback execution.
    #[allow(unused)]
    pub(crate) fn get_by_path<F>(&self, path: &Path, callback: F) -> crate::Result<bool>
    where
        F: FnOnce(&RefCell<Value>) -> crate::Result<()>,
    {
        // A nested helper function to handle the recursive traversal.
        // It takes the current root and the remaining path segments.
        fn get<C>(root: &RefCell<Value>, path: Option<&Path>, callback: C) -> crate::Result<bool>
        where
            C: FnOnce(&RefCell<Value>) -> crate::Result<()>,
        {
            match path {
                // Case 1: The path has more segments to traverse.
                Some(path) => match (&path.first, &*root.borrow()) {
                    (Key::String(key), Value::Object(object)) => {
                        match object.get(key) {
                            Some(next_value) => {
                                // Recursively call `get` on the next value in the path.
                                get(next_value, path.next(), callback)
                            }
                            None => {
                                // The next path segment does not exist.
                                Ok(false)
                            }
                        }
                    }
                    (Key::Index(index), Value::Array(array)) => {
                        match array.get(*index) {
                            Some(next_value) => {
                                // Recursively call `get` on the next value in the path.
                                get(next_value, path.next(), callback)
                            }
                            None => {
                                // The next path segment does not exist.
                                Ok(false)
                            }
                        }
                    }
                    // The current value is not an Object or Array, so we can't continue traversing.
                    (_, _) => Ok(false),
                },

                // Case 2: The end of the path has been reached.
                None => {
                    // Execute the callback on the final value.
                    if matches!(&*root.borrow(), Value::None) {
                        Ok(false)
                    } else {
                        callback(root)?;
                        Ok(true)
                    }
                }
            }
        }

        // Start the recursive traversal from the top-level object.
        if let Key::String(key) = &path.first
            && let Some(value) = self.get(key)
        {
            get(value, path.next(), callback)
        } else {
            Ok(false)
        }
    }

    /// Retrieves a deep `RefCell<Value>` reference from a `HashMap<String, RefCell<Value>>` by following a given `Path`.
    /// This method uses an explicit loop to avoid stack overflow issues that could occur in a recursive implementation.
    ///
    /// # Safety
    /// This method is `unsafe` because it returns a reference derived from a raw pointer (`*const RefCell<Value>`).
    /// The caller must ensure the following to avoid undefined behavior (UB):
    /// 1. **No mutation of the object tree during the reference's lifetime**: While the returned `&RefCell<Value>` is in use,
    ///    the caller must not remove or replace the referenced value in the object tree. For example, for a path `a.b.c`
    ///    (e.g., `{a: {b: {c: 1}}}`), after obtaining a reference to `c`, the caller must not mutate `b` (via `borrow_mut`)
    ///    to remove or replace `c`, as this could invalidate the returned reference.
    /// 2. **No concurrent access to the `HashMap`**: The `HashMap` must not be modified (e.g., via insertion, removal, or mutation
    ///    of other `RefCell`s) while the returned reference is live, as this could lead to dangling pointers or data races.
    /// 3. **Valid path and object structure**: The caller must ensure the `Path` is valid and corresponds to a navigable structure
    ///    in the `HashMap`. Invalid paths or non-object values at intermediate steps will result in `None`, but the caller must
    ///    not assume the returned reference is always valid without proper checks.
    ///
    /// # Potential Undefined Behavior (UB) Points
    /// - **Deref of raw pointer (`*raw`)**: The raw pointer `raw` is dereferenced to obtain a `&RefCell<Value>`. If the underlying
    ///   `RefCell` has been removed or invalidated (e.g., by mutating the `HashMap` or parent `Value::Object`), this dereference
    ///   could lead to UB (e.g., accessing freed memory).
    /// - **Borrowing the `RefCell`**: The `value.borrow()` call assumes the `RefCell` is still valid and not mutably borrowed
    ///   elsewhere. If the caller violates `RefCell` borrowing rules (e.g., by holding a `RefMut` elsewhere), this could trigger
    ///   UB or a panic.
    /// - **Lifetime of returned reference**: The returned `&RefCell<Value>` is tied to the raw pointer's validity. If the `HashMap`
    ///   or its nested objects are modified to remove or replace the referenced `RefCell`, the returned reference becomes dangling,
    ///   leading to UB when used.
    ///
    /// # Parameters
    /// - `path`: A `Path` object representing the sequence of keys to traverse the nested `HashMap` structure.
    ///
    /// # Returns
    /// - `Some(&RefCell<Value>)` if the path resolves to a valid `RefCell<Value>` in the object tree.
    /// - `None` if the path is invalid, a key is missing, or an intermediate value is not a `Value::Object`.
    pub(crate) unsafe fn unsafe_get_by_path(&self, path: &Path) -> Option<&RefCell<Value>> {
        // Attempt to get the first value from the HashMap using the path's first key.
        if let Key::String(key) = &path.first
            && let Some(value) = self.get(key)
        {
            // Initialize the next path segment to traverse.
            let mut next = path.next();
            // Store the current `RefCell<Value>` as a raw pointer to avoid lifetime issues with temporary references.
            let mut raw = value as *const RefCell<Value>;

            // Iterate through the path segments using a loop to avoid recursion.
            loop {
                // Dereference the raw pointer to access the `RefCell<Value>`.
                // UB Risk: If the `RefCell` pointed to by `raw` has been invalidated (e.g., removed from the HashMap or parent
                // object), this dereference causes UB.
                let value = unsafe { &*raw };

                match next {
                    // If there are more path segments, try to navigate deeper.
                    Some(n) => match (&n.first, &*value.borrow()) {
                        // Check if the current value is a `Value::Object` (i.e., a nested HashMap).
                        (Key::String(key), Value::Object(object)) => match object.get(key) {
                            // If the next key exists, update the raw pointer and continue to the next path segment.
                            Some(value) => {
                                raw = value as *const RefCell<Value>;
                                next = n.next();
                            }
                            // If the key is missing, the path is invalid, so return None.
                            None => break None,
                        },
                        (Key::Index(index), Value::Array(array)) => match array.get(*index) {
                            // If the next key exists, update the raw pointer and continue to the next path segment.
                            Some(value) => {
                                raw = value as *const RefCell<Value>;
                                next = n.next();
                            }
                            // If the key is missing, the path is invalid, so return None.
                            None => break None,
                        },
                        // If the current value is not an object, the path cannot be followed, so return None.
                        _ => break None,
                    },
                    // If there are no more path segments, return the current `RefCell<Value>` reference.
                    None => {
                        break if matches!(&*value.borrow(), Value::None) {
                            None
                        } else {
                            Some(value)
                        };
                    }
                }
            }
        } else {
            // If the first key is not found in the HashMap, return None.
            None
        }
    }

    /// Do not call the borrow_mut of Value across the substitute function, it may cause panic.
    #[instrument(level = Level::TRACE, skip_all, fields(path = %path, vlaue = %value.borrow(), mreged = %value.borrow().is_merged())
    )]
    pub(crate) fn substitute_value(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
        memo: &mut Memo,
    ) -> crate::Result<()> {
        memo.substitution_counter += 1;
        if memo.substitution_counter > MAX_SUBSTITUTION_DEPTH {
            return Err(Error::SubstitutionDepthExceeded {
                max_depth: MAX_SUBSTITUTION_DEPTH,
            });
        }
        let value_ref = value.borrow();
        if value_ref.is_merged() {
            memo.substitution_counter -= 1;
            return Ok(());
        }
        match &*value_ref {
            Value::Object(object) => {
                let span = span!(Level::TRACE, "Object");
                let _enter = span.enter();
                for (key, val) in object.iter() {
                    let sub_path = path.join(RefPath::new(RefKey::Str(key), None));
                    self.substitute_value(&sub_path, val, memo)?;
                }
                drop(value_ref);
                // TODO
                if let Ok(mut value) = value.try_borrow_mut() {
                    value.try_become_merged();
                }
                // value.borrow_mut().try_become_merged();
            }
            Value::Array(array) => {
                self.handle_array(path, array, memo)?;
                drop(value_ref);
            }
            Value::Boolean(_) | Value::Null | Value::None | Value::String(_) | Value::Number(_) => {
            }
            Value::Substitution(substitution) => {
                let substitution = substitution.clone();
                drop(value_ref);
                self.handle_substitution(path, value, substitution, memo)?;
            }
            Value::Concat(_) => {
                drop(value_ref);
                self.handle_concat(path, value, memo)?;
            }
            Value::AddAssign(_) => {
                drop(value_ref);
                self.handle_add_assign(path, value, memo)?;
            }
            Value::DelayReplacement(_) => {
                drop(value_ref);
                self.handle_delay_replacement(path, value, memo)?;
            }
        }
        memo.substitution_counter -= 1;
        Ok(())
    }

    fn handle_add_assign(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
        memo: &mut Memo,
    ) -> crate::Result<()> {
        let span = span!(Level::TRACE, "AddAssign");
        let _enter = span.enter();
        let mut value_mut = value.borrow_mut();
        let add_assign = expect_variant!(value_mut, Value::AddAssign, mut);
        let add_assign = std::mem::take(add_assign);
        drop(value_mut);
        let v: RefCell<Value> = RefCell::new(add_assign.into());
        self.substitute_value(path, &v, memo)?;
        let mut v = v.into_inner();
        v.try_become_merged();
        let add_assign = AddAssign::new(Box::new(v));
        *value.borrow_mut() = Value::add_assign(add_assign);
        Ok(())
    }

    fn handle_array(&self, path: &RefPath, array: &Array, memo: &mut Memo) -> crate::Result<()> {
        let span = span!(Level::TRACE, "Array");
        let _enter = span.enter();
        for (index, ele) in array.iter().enumerate() {
            let sub_path = path.join(RefPath::new(RefKey::Index(index), None));
            self.substitute_value(&sub_path, ele, memo)?;
        }
        Ok(())
    }

    fn handle_substitution(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
        substitution: Substitution,
        memo: &mut Memo,
    ) -> crate::Result<()> {
        let span = span!(Level::TRACE, "Substitution");
        let _enter = span.enter();
        match memo.tracker.iter().rposition(|p| p == path) {
            None => {
                memo.tracker.push(path.clone().into());
            }
            Some(i) => {
                return Err(Error::SubstitutionCycle {
                    current: path.to_string(),
                    backtrace: memo.tracker[i..].iter().map(|p| p.to_string()).collect(),
                });
            }
        }
        trace!("substitute: {}", substitution);
        // During the substitution stage, we only modify non-`Value::Object` values (e.g., scalars) via `RefCell::borrow_mut`.
        // This ensures that no `RefCell<Value>` is inserted, removed, or replaced in any `HashMap` within the object tree,
        // guaranteeing that the address of the target `RefCell` remains valid and safe to access.
        // Therefore, the `unsafe` call to `unsafe_get_by_path` does not risk undefined behavior (UB) in this context,
        // as the object tree's structure is not modified during the lifetime of the returned reference.
        let target = unsafe { self.unsafe_get_by_path(&substitution.path) };
        match target {
            Some(target) => {
                if enabled!(Level::TRACE) {
                    trace!("find substitution: {} -> {}", substitution, target.borrow());
                }
                if &*substitution.path == path
                    && matches!(&*target.borrow(), Value::Substitution(_))
                {
                    return if substitution.optional {
                        *target.borrow_mut() = Value::None;
                        Ok(())
                    } else {
                        Err(Error::SubstitutionCycle {
                            current: substitution.to_string(),
                            backtrace: vec![substitution.to_string()],
                        })
                    };
                }
                self.substitute_value(&RefPath::from(&substitution.path), target, memo)?;
                let target_clone = target.borrow().clone();
                if enabled!(Level::TRACE) {
                    trace!("set {} to {}", value.borrow(), target_clone);
                }
                *value.borrow_mut() = target_clone;
            }
            None => match std::env::var(substitution.full_path()) {
                Ok(env_var) => {
                    if enabled!(Level::TRACE) {
                        trace!("set environment variable {} to {}", env_var, value.borrow());
                    }
                    *value.borrow_mut() = Value::string(env_var);
                }
                Err(_) => {
                    if !substitution.optional {
                        return Err(Error::SubstitutionNotFound(substitution.to_string()));
                    } else {
                        *value.borrow_mut() = Value::None;
                    }
                }
            },
        }
        memo.tracker.pop();
        Ok(())
    }

    fn pop_value_from_concat(
        value: &RefCell<Value>,
    ) -> Option<(Option<String>, RefCell<Value>, usize)> {
        let mut value_mut = value.borrow_mut();
        let concat = expect_variant!(value_mut, Value::Concat, mut);
        let len = concat.len();
        let popped = concat.pop_back();
        match &popped {
            Some((_, v)) => {
                if enabled!(Level::TRACE) {
                    trace!("popped {} from {}", v.borrow(), concat);
                }
                if concat.get_values().is_empty() {
                    drop(value_mut);
                    *value.borrow_mut().deref_mut() = Value::None;
                    if enabled!(Level::TRACE) {
                        trace!("concat is empty, set to none");
                    }
                }
            }
            None => {
                trace!("popped None from {}", concat);
            }
        }
        popped.map(|(s, v)| (s, v, len - 1))
    }

    fn handle_concat(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
        memo: &mut Memo,
    ) -> crate::Result<()> {
        let span = span!(Level::TRACE, "Concat");
        let _enter = span.enter();

        match Self::pop_value_from_concat(value) {
            Some((space_last, last, last_index)) => {
                let sub_path = path.join(RefPath::new(RefKey::Index(last_index), None));
                self.substitute_value(&sub_path, &last, memo)?;
                if matches!(&*value.borrow(), Value::Concat(_)) {
                    match Self::pop_value_from_concat(value) {
                        Some((space_second_last, second_last, second_last_index)) => {
                            let sub_path =
                                path.join(RefPath::new(RefKey::Index(second_last_index), None));
                            self.substitute_value(&sub_path, &second_last, memo)?;
                            let last = last.into_inner();
                            let new_val = Value::concatenate(
                                path,
                                second_last.into_inner(),
                                space_last,
                                last,
                            )?;
                            let mut new_val = RefCell::new(new_val);
                            let sub_path =
                                path.join(RefPath::new(RefKey::Str("concatenation"), None));
                            self.substitute_value(&sub_path, &new_val, memo)?;
                            new_val.get_mut().try_become_merged();
                            if enabled!(Level::TRACE) {
                                trace!("push back {} to {}", new_val.get_mut(), value.borrow());
                            }
                            match &mut *value.borrow_mut() {
                                v @ Value::None => {
                                    *v = new_val.into_inner();
                                }
                                Value::Concat(concat) => {
                                    concat.push_back(space_second_last, new_val);
                                }
                                v => {
                                    let left = std::mem::take(v);
                                    *v =
                                        Value::concatenate(path, left, None, new_val.into_inner())?;
                                }
                            }
                            self.substitute_value(path, value, memo)?;
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
                        Value::concatenate(path, second_last, space_last, last.into_inner())?;
                    new_val.try_become_merged();
                    if enabled!(Level::TRACE) {
                        trace!("set {} to {}", value.borrow(), new_val);
                    }
                    *value.borrow_mut() = new_val;
                    self.substitute_value(path, value, memo)?;
                }
            }
            None => {
                if enabled!(Level::TRACE) {
                    trace!("set none to {}", value.borrow());
                }
                *value.borrow_mut() = Value::None;
            }
        }
        Ok(())
    }

    fn pop_value_from_delay_replacement(value: &RefCell<Value>) -> Option<(RefCell<Value>, usize)> {
        let mut value_mut = value.borrow_mut();
        let replacement = expect_variant!(value_mut, Value::DelayReplacement, mut);
        let len = replacement.len();
        let popped = replacement.pop_back();
        match &popped {
            Some(v) => {
                if enabled!(Level::TRACE) {
                    trace!("popped {} from {}", v.borrow(), replacement);
                }
                if replacement.is_empty() {
                    drop(value_mut);
                    *value.borrow_mut().deref_mut() = Value::None;
                    if enabled!(Level::TRACE) {
                        trace!("delay replacement is empty, set to none");
                    }
                }
            }
            None => {
                trace!("popped None from {}", replacement);
            }
        }
        popped.map(|v| (v, len - 1))
    }

    fn handle_delay_replacement(
        &self,
        path: &RefPath,
        value: &RefCell<Value>,
        memo: &mut Memo,
    ) -> crate::Result<()> {
        let span = span!(Level::TRACE, "DelayReplacement");
        let _enter = span.enter();

        match Self::pop_value_from_delay_replacement(value) {
            Some((last, last_index)) => {
                let sub_path = path.join(RefPath::new(RefKey::Index(last_index), None));
                self.substitute_value(&sub_path, &last, memo)?;
                if matches!(&*value.borrow(), Value::DelayReplacement(_)) {
                    match Self::pop_value_from_delay_replacement(value) {
                        Some((second_last, second_last_index)) => {
                            let sub_path =
                                path.join(RefPath::new(RefKey::Index(second_last_index), None));
                            self.substitute_value(&sub_path, &second_last, memo)?;
                            let new_val =
                                Value::replace(path, second_last.into_inner(), last.into_inner())?;
                            let mut new_val = RefCell::new(new_val);
                            let sub_path =
                                path.join(RefPath::new(RefKey::Str("replacement"), None));
                            self.substitute_value(&sub_path, &new_val, memo)?;
                            new_val.get_mut().try_become_merged();
                            if enabled!(Level::TRACE) {
                                trace!("push back {} to {}", new_val.get_mut(), value.borrow());
                            }
                            match &mut *value.borrow_mut() {
                                v @ Value::None => {
                                    *v = new_val.into_inner();
                                }
                                Value::DelayReplacement(re) => {
                                    re.push_back(new_val);
                                }
                                v => {
                                    let left = std::mem::take(v);
                                    *v = Value::replace(path, left, new_val.into_inner())?;
                                }
                            }
                            self.substitute_value(path, value, memo)?;
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
                    let mut new_val = Value::replace(path, second_last, last.into_inner())?;
                    new_val.try_become_merged();
                    if enabled!(Level::TRACE) {
                        trace!("set {} to {}", value.borrow(), new_val);
                    }
                    *value.borrow_mut() = new_val;
                    self.substitute_value(path, value, memo)?;
                }
            }
            None => {
                if enabled!(Level::TRACE) {
                    trace!("set none to {}", value.borrow());
                }
                *value.borrow_mut() = Value::None;
            }
        }
        Ok(())
    }

    pub(crate) fn substitute(&self) -> crate::Result<()> {
        let mut memo = Memo::default();
        for (key, value) in self.iter() {
            let path = RefPath::new(RefKey::Str(key), None);
            self.substitute_value(&path, value, &mut memo)?;
            value.borrow_mut().try_become_merged();
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

impl From<Object> for BTreeMap<String, V> {
    fn from(val: Object) -> Self {
        match val {
            Object::Merged(object) | Object::Unmerged(object) => object,
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut iter = self.iter();
        if let Some((k, v)) = iter.next() {
            write!(f, "{}: {}", k, v.borrow())?;
            for (k, v) in iter {
                write!(f, ", ")?;
                write!(f, "{}: {}", k, v.borrow())?;
            }
        }
        write!(f, "}}")?;
        Ok(())
    }
}
