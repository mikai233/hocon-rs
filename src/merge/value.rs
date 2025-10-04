use tracing::trace;

use crate::{
    error::Error,
    merge::{
        add_assign::AddAssign, array::Array, concat::Concat, delay_replacement::DelayReplacement,
        object::Object, path::RefPath, substitution::Substitution,
    },
};
use std::fmt::Write;
use std::{cell::RefCell, fmt::Display};

#[macro_export(local_inner_macros)]
macro_rules! expect_variant {
    // Match tuple-style enum variants, e.g. MyEnum::Foo(x)
    ($expr:expr, $variant:path) => {{
        match $expr {
            $variant(var) => var,
            other => std::panic!(
                "expected variant `{}`, got `{:?}`",
                std::stringify!($variant),
                other,
            ),
        }
    }};

    // Match struct-style enum variants, e.g. MyEnum::Foo { a, b }
    ($expr:expr, $variant:path, $($ident:ident),+ ) => {{
        match $expr {
            $variant { $($ident),+ } => ($($ident),+),
            other => std::panic!(
                "expected variant `{}`, got `{:?}`",
                std::stringify!($variant),
                other,
            ),
        }
    }};
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Value {
    Object(Object),
    Array(Array),
    Boolean(bool),
    Null,
    #[default]
    None,
    String(String),
    Number(serde_json::Number),
    Substitution(Substitution),
    Concat(Concat),
    AddAssign(AddAssign),
    DelayReplacement(DelayReplacement),
}

impl Value {
    pub(crate) fn object(o: impl Into<Object>) -> Value {
        Value::Object(o.into())
    }

    pub(crate) fn array(a: impl Into<Array>) -> Value {
        Value::Array(a.into())
    }

    pub(crate) fn string(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }

    pub(crate) fn number(n: serde_json::Number) -> Value {
        Value::Number(n)
    }

    pub(crate) fn substitution(s: impl Into<Substitution>) -> Value {
        Value::Substitution(s.into())
    }

    pub(crate) fn concat(c: impl Into<Concat>) -> Value {
        Value::Concat(c.into())
    }

    pub(crate) fn add_assign(a: impl Into<AddAssign>) -> Value {
        Value::AddAssign(a.into())
    }

    pub(crate) fn delay_replacement<I>(value: I) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        let d = DelayReplacement::from_iter(value);
        Value::DelayReplacement(d.flatten())
    }

    pub(crate) fn ty(&self) -> &'static str {
        match self {
            Value::Object(_) => "object",
            Value::Array(_) => "array",
            Value::Boolean(_) => "boolean",
            Value::Null => "null",
            Value::None => "none",
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Substitution(_) => "substitution",
            Value::Concat(_) => "concat",
            Value::AddAssign(_) => "add_assign",
            Value::DelayReplacement(_) => "delay_replacement",
        }
    }

    pub(crate) fn try_become_merged(&mut self) -> bool {
        match self {
            Value::Object(object) => object.try_become_merged(),
            Value::Array(array) => array.try_become_merged(),
            Value::Boolean(_) | Value::Null | Value::None | Value::String(_) | Value::Number(_) => {
                true
            }
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayReplacement(_) => false,
        }
    }

    /// Replaces the left `Value` with the right `Value` at the specified path, following HOCON replacement rules.
    ///
    /// In HOCON, replacement behavior depends on the types of the left and right values:
    /// - **Objects**: Merges the right object into the left, resolving conflicts based on the path.
    /// - **Arrays**: Replaces with the right value unless the right is an `AddAssign`, which appends to the array.
    /// - **Primitives (Boolean, String, Number, Null, None)**: Replaces the left with the right value, unless the right
    ///   is a `Substitution` or `Concat`, which defers resolution.
    /// - **AddAssign**: Cannot appear as the left value, as it is expanded to an array during prior object merging.
    /// - **Substitution, Concat, DelayReplacement**: Defers resolution by wrapping both values in a `DelayReplacement`.
    ///
    /// If the right value is an `AddAssign` (e.g., `a += 1`), it is either appended to an array (if the left is an array)
    /// or expanded to an array `[1]` (if the left is `None`). If the right value is a `Substitution` or unresolved `Concat`,
    /// replacement is deferred because the final value cannot be determined yet. This ensures correct handling of
    /// dependencies in HOCON configurations.
    ///
    /// # Parameters
    /// - `path`: The `RefPath` at which the replacement occurs, used for error reporting.
    /// - `left`: The original `Value` to be replaced.
    /// - `right`: The new `Value` to replace or combine with the left.
    ///
    /// # Returns
    /// - `Ok(Value)`: The resulting `Value` after replacement or combination.
    /// - `Err(Error::ConcatenateDifferentType)`: If the left and right types are incompatible (e.g., left is an object
    ///   and right is an `AddAssign`).
    ///
    /// # Notes
    /// - `AddAssign` cannot be the left value because prior object merging expands it to an array.
    /// - Deferred replacements (`DelayReplacement`) are used when the right value involves unresolved `Substitution`
    ///   or `Concat` to preserve dependencies for later resolution.
    /// - Trace logs are emitted for debugging the replacement operation and result.
    pub(crate) fn replace(path: &RefPath, left: Value, right: Value) -> crate::Result<Value> {
        // Log the replacement operation for debugging.
        trace!("replace: `{}`: `{}` <- `{}`", path, left, right);

        let new_val = match left {
            // Handle replacement when the left value is an object.
            Value::Object(mut obj_left) => match right {
                // Merge the right object into the left, respecting the path for conflict resolution.
                Value::Object(right) => {
                    obj_left.merge(right, Some(path))?;
                    Value::object(obj_left)
                }
                // Replace the left object with any primitive or array value.
                Value::Array(_)
                | Value::Boolean(_)
                | Value::Null
                | Value::None
                | Value::String(_)
                | Value::Number(_) => right,
                // Defer replacement if the right is a substitution, wrapping both values.
                Value::Substitution(_) => {
                    let left = Value::object(obj_left);
                    Value::delay_replacement([left, right])
                }
                // Attempt to resolve the right concat and merge or defer based on the result.
                Value::Concat(concat) => {
                    let try_resolved = concat.try_resolve(path)?;
                    match try_resolved {
                        // Merge resolved object into the left object.
                        Value::Object(object) => {
                            obj_left.merge(object, Some(path))?;
                            Value::object(obj_left)
                        }
                        // Defer if the concat resolves to another concat, prepending the left object.
                        Value::Concat(mut concat) => {
                            let left = Value::object(obj_left);
                            concat.push_front(RefCell::new(left), None);
                            Value::concat(concat)
                        }
                        // Use the resolved value directly if it’s not an object or concat.
                        other => other,
                    }
                }
                // Objects cannot be replaced with AddAssign values.
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "object",
                        right_type: right.ty(),
                    });
                }
                // Prepend the left object to an existing delayed replacement.
                Value::DelayReplacement(mut delay_merge) => {
                    let left = Value::object(obj_left);
                    delay_merge.push_front(RefCell::new(left));
                    Value::DelayReplacement(delay_merge)
                }
            },
            // Handle replacement when the left value is an array.
            Value::Array(mut array_left) => match right {
                // Defer replacement for substitutions or delayed replacements.
                Value::Substitution(_) | Value::DelayReplacement(_) => {
                    Value::delay_replacement([Value::array(array_left), right])
                }
                // Attempt to resolve the right concat and handle the result.
                Value::Concat(concat) => {
                    let right = concat.try_resolve(path)?;
                    match right {
                        // Concatenate arrays if the concat resolves to an array.
                        Value::Array(array) => {
                            let left = Value::Array(array_left);
                            let right = Value::Array(array);
                            Self::concatenate(path, left, None, right)?
                        }
                        // Defer if the concat resolves to another concat.
                        Value::Concat(concat) => {
                            let left = Value::Array(array_left);
                            let right = Value::Concat(concat);
                            Value::delay_replacement([left, right])
                        }
                        // Replace with the resolved value otherwise.
                        right => right,
                    }
                }
                // Append the AddAssign value to the array, preserving merge state.
                Value::AddAssign(add_assign) => {
                    let inner: Value = add_assign.into();
                    let unmerged = inner.is_unmerged();
                    array_left.push(RefCell::new(inner));
                    if unmerged {
                        array_left.as_unmerged()
                    }
                    Value::array(array_left)
                }
                // Replace the left array with any other right value.
                right => right,
            },
            // Handle replacement when the left value is null.
            Value::Null => match right {
                // Null cannot be replaced with AddAssign.
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "null",
                        right_type: right.ty(),
                    });
                }
                // Replace null with any other value.
                other => other,
            },
            // Handle replacement when the left value is none.
            Value::None => match right {
                // Expand AddAssign to an array with the resolved value.
                Value::AddAssign(add_assign) => {
                    let value = add_assign.try_resolve(path)?;
                    let array = if value.is_merged() {
                        Array::Merged(vec![RefCell::new(value)])
                    } else {
                        Array::Unmerged(vec![RefCell::new(value)])
                    };
                    Value::Array(array)
                }
                // Replace none with any other right value.
                right => right,
            },
            // Handle replacement for primitive left values (boolean, string, number).
            Value::Boolean(_) | Value::String(_) | Value::Number(_) => match right {
                // Defer replacement if the right is a substitution.
                Value::Substitution(_) => Value::delay_replacement([left, right]),
                // Attempt to resolve the right concat and handle the result.
                Value::Concat(concat) => {
                    let right = concat.try_resolve(path)?;
                    match right {
                        // Defer if the concat resolves to another concat.
                        Value::Concat(_) => Value::delay_replacement([left, right]),
                        // AddAssign is invalid after concat resolution.
                        Value::AddAssign(_) => {
                            return Err(Error::ConcatenateDifferentType {
                                path: path.to_string(),
                                left_type: left.ty(),
                                right_type: "add_assign",
                            });
                        }
                        // Replace with the resolved value otherwise.
                        other => other,
                    }
                }
                // Primitives cannot be replaced with AddAssign.
                Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: left.ty(),
                        right_type: right.ty(),
                    });
                }
                // Replace with any other right value.
                other => other,
            },
            // AddAssign cannot be the left value due to prior expansion during object merging.
            Value::AddAssign(_) => {
                return Err(Error::ConcatenateDifferentType {
                    path: path.to_string(),
                    left_type: left.ty(),
                    right_type: right.ty(),
                });
            }
            // Defer replacement for left substitution, concat, or delayed replacement.
            Value::Substitution(_) | Value::Concat(_) | Value::DelayReplacement(_) => {
                Value::delay_replacement([left, right])
            }
        };

        // Log the result of the replacement for debugging.
        trace!("replace result: `{path}`=`{new_val}`");

        Ok(new_val)
    }

    /// Concatenates two HOCON `Value`s according to the HOCON specification, producing a new `Value`.
    ///
    /// In HOCON, concatenation combines two values at a given path, with specific behavior depending on the
    /// types of the left and right values:
    /// - **Objects**: Merges the right object into the left object, resolving conflicts based on the path.
    /// - **Arrays**: Concatenates the right array onto the left array.
    /// - **Strings, Numbers, Booleans, Null**: Concatenates as strings, optionally inserting a separator (`space`).
    /// - **Substitutions, Concatenations, Delayed Replacements**: Wraps the values in a `Concat` structure for deferred evaluation.
    /// - **None**: Handles special cases, such as returning the right value or creating a string with the separator.
    /// - **AddAssign**: Not supported for concatenation, as it represents an assignment operation, not a value.
    ///
    /// The function ensures type compatibility, raising an error if the left and right values cannot be concatenated
    /// (e.g., attempting to concatenate an object with a string). The `space` parameter is used when concatenating
    /// primitive types (e.g., strings, numbers) to insert a separator between them, as per HOCON’s concatenation rules.
    ///
    /// # Parameters
    /// - `path`: The `RefPath` at which the concatenation is occurring, used for error reporting.
    /// - `left`: The left `Value` to concatenate.
    /// - `space`: An optional `String` separator to insert between concatenated values (e.g., a space or empty string).
    /// - `right`: The right `Value` to concatenate.
    ///
    /// # Returns
    /// - `Ok(Value)`: The resulting concatenated `Value`, which may be an object, array, string, or `Concat` structure.
    /// - `Err(Error::ConcatenateDifferentType)`: If the left and right types are incompatible for concatenation.
    ///
    /// # Notes
    /// - The function logs trace messages to indicate the input values and the result.
    /// - The result is guaranteed not to be a `DelayReplacement`, `Substitution`, or `AddAssign`, as these are either
    ///   wrapped in a `Concat` or resolved to a concrete value.
    /// - The `space` parameter is only relevant for concatenating primitive types or when creating a `Concat` structure.
    pub(crate) fn concatenate(
        path: &RefPath,
        left: Value,
        space: Option<String>,
        right: Value,
    ) -> crate::Result<Value> {
        trace!("concatenate: `{}`: `{}` <- `{}`", path, left, right);

        let val = match left {
            // Handle object concatenation.
            Value::Object(mut left_obj) => match right {
                // If right is None, return the left object unchanged.
                Value::None => Value::object(left_obj),
                // Merge right object into left object, respecting the path for conflict resolution.
                Value::Object(right_obj) => {
                    left_obj.merge(right_obj, Some(path))?;
                    Value::object(left_obj)
                }
                // Objects cannot be concatenated with arrays, primitives, or AddAssign.
                Value::Null
                | Value::Array(_)
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Number(_)
                | Value::AddAssign(_) => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "object",
                        right_type: right.ty(),
                    });
                }
                // For substitutions or delayed replacements, wrap in a Concat structure.
                Value::Substitution(_) | Value::DelayReplacement(_) => {
                    let left = Value::object(left_obj);
                    Value::concat(Concat::two(left, space, right))
                }
                // If right is a Concat, prepend the left object to it.
                Value::Concat(mut concat) => {
                    let left = Value::object(left_obj);
                    concat.push_front(RefCell::new(left), space);
                    Value::concat(concat)
                }
            },
            // Handle array concatenation.
            Value::Array(mut left_array) => {
                if let Value::Array(right_array) = right {
                    // Extend left array with right array's elements.
                    left_array.extend(right_array.into_inner());
                    Value::array(left_array)
                } else {
                    // Arrays can only be concatenated with other arrays.
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: "array",
                        right_type: right.ty(),
                    });
                }
            }
            // Handle None as the left value.
            Value::None => match space {
                // If a separator is provided, handle concatenation with primitives or None.
                Some(space) => match right {
                    // For primitives, create a string starting with the separator.
                    Value::Null | Value::Boolean(_) | Value::String(_) | Value::Number(_) => {
                        let mut s = String::new();
                        s.push_str(&space);
                        write!(&mut s, "{right}").unwrap();
                        Value::string(s)
                    }
                    // If right is None, return the separator as a string.
                    Value::None => Value::string(space),
                    // For substitutions, wrap in a Concat structure.
                    Value::Substitution(_) => Value::concat(Concat::two(left, Some(space), right)),
                    // Otherwise, return the right value unchanged.
                    right => right,
                },
                // Without a separator, return the right value.
                _ => right,
            },
            // Handle concatenation of primitive types (null, boolean, string, number).
            Value::Null | Value::Boolean(_) | Value::String(_) | Value::Number(_) => match right {
                // Concatenate primitives into a single string, inserting the separator if provided.
                Value::Boolean(_) | Value::Null | Value::String(_) | Value::Number(_) => {
                    let mut s = String::new();
                    write!(&mut s, "{left}").unwrap();
                    if let Some(space) = &space {
                        s.push_str(space);
                    }
                    write!(&mut s, "{right}").unwrap();
                    Value::string(s)
                }
                // If right is None, append the separator (if any) to the left value as a string.
                Value::None => {
                    let mut s = String::new();
                    write!(&mut s, "{left}").unwrap();
                    if let Some(space) = &space {
                        s.push_str(space);
                    }
                    Value::string(s)
                }
                // For substitutions, wrap in a Concat structure.
                Value::Substitution(_) => Value::concat(Concat::two(left, space, right)),
                // Primitives cannot be concatenated with objects, arrays, or AddAssign.
                _ => {
                    return Err(Error::ConcatenateDifferentType {
                        path: path.to_string(),
                        left_type: left.ty(),
                        right_type: right.ty(),
                    });
                }
            },
            // For substitutions or delayed replacements, wrap both values in a Concat structure.
            Value::Substitution(_) | Value::DelayReplacement(_) => {
                Value::concat(Concat::two(left, space, right))
            }
            // If left is a Concat, append the right value to it.
            Value::Concat(mut concat) => {
                concat.push_back(space, RefCell::new(right));
                Value::concat(concat)
            }
            // AddAssign is not a valid type for concatenation.
            Value::AddAssign(_) => {
                return Err(Error::ConcatenateDifferentType {
                    path: path.to_string(),
                    left_type: left.ty(),
                    right_type: right.ty(),
                });
            }
        };

        trace!("concatenate result: `{path}`=`{val}`");

        debug_assert!(!matches!(
            val,
            Value::DelayReplacement(_) | Value::Substitution(_) | Value::AddAssign(_)
        ));

        Ok(val)
    }

    pub(crate) fn is_merged(&self) -> bool {
        match self {
            Value::Object(object) => object.is_merged(),
            Value::Array(array) => array.is_merged(),
            Value::Boolean(_) | Value::String(_) | Value::Number(_) | Value::Null | Value::None => {
                true
            }
            Value::Substitution(_)
            | Value::Concat(_)
            | Value::AddAssign(_)
            | Value::DelayReplacement(_) => false,
        }
    }

    pub(crate) fn is_unmerged(&self) -> bool {
        !self.is_merged()
    }

    /// Resolves `AddAssign` values in the current `Value` by converting them to arrays, as per HOCON rules.
    ///
    /// In HOCON, `AddAssign` (e.g., `a += 1` following `a = []`) represents a value to be appended to an array
    /// at the specified key. After all `include` directives and substitutions (e.g., `${x}`) have been resolved,
    /// any remaining `AddAssign` values are guaranteed to be standalone and cannot reference data higher in the
    /// configuration hierarchy (e.g., `a += 1` does not depend on prior values of `a`). Thus, each `AddAssign` is
    /// transformed into an array containing its single value (e.g., `a += 1` becomes `a = [1]`).
    ///
    /// If the current `Value` is an object, this function delegates to the object's `resolve_add_assign` method
    /// to recursively process nested values. If the current `Value` is an `AddAssign`, it extracts the inner value
    /// and replaces itself with an `Array` containing that value. After transformation, it attempts to merge the
    /// resulting array with existing values via `try_become_merged`, adhering to HOCON's merging rules.
    ///
    /// # Context
    /// - This function is called after all substitutions and includes are resolved, ensuring that `AddAssign`
    ///   values are standalone and ready for direct conversion to arrays.
    /// - The transformation supports HOCON's array concatenation semantics, where `a += value` appends `value`
    ///   to an array at key `a`.
    ///
    /// # Notes
    /// - Non-object and non-`AddAssign` values are ignored, as they do not require resolution.
    /// - The `try_become_merged` call ensures the resulting array is merged with any existing values at the
    ///   same key, as required by HOCON.
    pub(crate) fn resolve_add_assign(&mut self) {
        if let Value::Object(object) = self {
            // Delegate to the object's `resolve_add_assign` method to process nested values recursively.
            object.resolve_add_assign();
        } else if let Value::AddAssign(add_assign) = self {
            // Extract the inner value from the AddAssign, replacing it with an empty box to avoid ownership issues.
            let val = std::mem::take(&mut add_assign.0);
            // Transform the AddAssign into an Array containing the single standalone value.
            *self = Value::Array(Array::new(vec![RefCell::new(*val)]));
            // Attempt to merge the resulting array with existing values at the same key, if applicable.
            self.try_become_merged();
        }
    }

    pub(crate) fn resolve(&mut self) -> crate::Result<()> {
        if let Value::Object(object) = self {
            object.substitute()?;
        }
        self.resolve_add_assign();
        self.try_become_merged();
        Ok(())
    }

    pub(crate) fn from_raw(
        parent: Option<&RefPath>,
        raw: crate::raw::raw_value::RawValue,
    ) -> crate::Result<Self> {
        let mut value = match raw {
            crate::raw::raw_value::RawValue::Object(raw_object) => {
                let object = Object::from_raw(parent, raw_object)?;
                Value::object(object)
            }
            crate::raw::raw_value::RawValue::Array(raw_array) => {
                let array = Array::from_raw(parent, raw_array)?;
                Value::array(array)
            }
            crate::raw::raw_value::RawValue::Boolean(b) => Value::Boolean(b),
            crate::raw::raw_value::RawValue::Null => Value::Null,
            crate::raw::raw_value::RawValue::String(raw_string) => {
                Value::string(raw_string.to_string())
            }
            crate::raw::raw_value::RawValue::Number(number) => Value::number(number),
            crate::raw::raw_value::RawValue::Substitution(substitution) => {
                Value::substitution(substitution)
            }
            crate::raw::raw_value::RawValue::Concat(concat) => {
                let concat = Concat::from_raw(parent, concat)?;
                Value::concat(concat)
            }
            crate::raw::raw_value::RawValue::AddAssign(add_assign) => {
                let add_assign = AddAssign::from_raw(parent, add_assign)?;
                Value::add_assign(add_assign)
            }
        };
        value.try_become_merged();
        Ok(value)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Object(object) => write!(f, "{object}"),
            Value::Array(array) => write!(f, "{array}"),
            Value::Boolean(boolean) => write!(f, "{boolean}"),
            Value::None => write!(f, "none"),
            Value::Null => write!(f, "null"),
            Value::String(string) => write!(f, "{string}"),
            Value::Number(number) => write!(f, "{number}"),
            Value::Substitution(substitution) => write!(f, "{substitution}"),
            Value::Concat(concat) => write!(f, "{concat}"),
            Value::AddAssign(add_assign) => write!(f, "{add_assign}"),
            Value::DelayReplacement(delay_merge) => write!(f, "{delay_merge}"),
        }
    }
}
