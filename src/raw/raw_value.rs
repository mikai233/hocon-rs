use crate::Result;
use crate::raw::add_assign::AddAssign;
use crate::raw::concat::Concat;
use crate::raw::field::ObjectField;
use crate::raw::include::Inclusion;
use crate::raw::raw_array::RawArray;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_string::RawString;
use crate::raw::substitution::Substitution;
use serde_json::Number;
use std::fmt::{Display, Formatter};

/// Type constants used to represent HOCON value kinds.
pub const RAW_OBJECT_TYPE: &str = "object";
pub const RAW_ARRAY_TYPE: &str = "array";
pub const RAW_BOOLEAN_TYPE: &str = "boolean";
pub const RAW_NULL_TYPE: &str = "null";
pub const RAW_QUOTED_STRING_TYPE: &str = "quoted_string";
pub const RAW_UNQUOTED_STRING_TYPE: &str = "unquoted_string";
pub const RAW_MULTILINE_STRING_TYPE: &str = "multiline_string";
pub const RAW_CONCAT_STRING_TYPE: &str = "concat_string";
pub const RAW_NUMBER_TYPE: &str = "number";
pub const RAW_SUBSTITUTION_TYPE: &str = "substitution";
pub const RAW_CONCAT_TYPE: &str = "concat";
pub const RAW_ADD_ASSIGN_TYPE: &str = "add_assign";

/// Represents any possible raw value in a HOCON configuration file.
///
/// This is the core enum used to model parsed HOCON data before
/// evaluation and resolution (e.g., substitutions or concatenations).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RawValue {
    /// A key-value object or an include statement.
    Object(RawObject),

    /// An ordered array of values.
    Array(RawArray),

    /// A boolean value (`true` or `false`).
    Boolean(bool),

    /// The `null` literal.
    Null,

    /// A HOCON string (quoted, unquoted, multiline, or path expression).
    String(RawString),

    /// A numeric value (integer or floating-point).
    Number(Number),

    /// A substitution expression like `${path.to.value}`.
    Substitution(Substitution),

    /// A concatenation of multiple values, e.g. `"foo" bar 123`.
    Concat(Concat),

    /// A value assigned via `+=` (HOCON add-assign syntax).
    AddAssign(AddAssign),
}

impl RawValue {
    /// Returns a string that identifies the type of this value.
    pub fn ty(&self) -> &'static str {
        match self {
            RawValue::Object(_) => RAW_OBJECT_TYPE,
            RawValue::Array(_) => RAW_ARRAY_TYPE,
            RawValue::Boolean(_) => RAW_BOOLEAN_TYPE,
            RawValue::Null => RAW_NULL_TYPE,
            RawValue::String(s) => s.ty(),
            RawValue::Number(_) => RAW_NUMBER_TYPE,
            RawValue::Substitution(_) => RAW_SUBSTITUTION_TYPE,
            RawValue::Concat(_) => RAW_CONCAT_TYPE,
            RawValue::AddAssign(_) => RAW_ADD_ASSIGN_TYPE,
        }
    }

    /// Returns `true` if the value is a simple literal (boolean, null, string, or number),
    /// or an `AddAssign` containing a simple value.
    pub fn is_simple_value(&self) -> bool {
        matches!(
            self,
            RawValue::Boolean(_) | RawValue::Null | RawValue::String(_) | RawValue::Number(_)
        ) || matches!(self, RawValue::AddAssign(r) if r.is_simple_value())
    }

    /// Creates a new object value containing an inclusion field (e.g., `include "file.conf"`).
    pub fn inclusion(inclusion: Inclusion) -> RawValue {
        let field = ObjectField::inclusion(inclusion);
        RawValue::Object(RawObject::new(vec![field]))
    }

    /// Constructs a new object from a list of key-value pairs.
    pub fn object(values: Vec<(RawString, RawValue)>) -> RawValue {
        let fields = values
            .into_iter()
            .map(|(k, v)| ObjectField::key_value(k, v))
            .collect();
        RawValue::Object(RawObject::new(fields))
    }

    /// Constructs a new array from a list of values.
    pub fn array(values: Vec<RawValue>) -> RawValue {
        RawValue::Array(RawArray::new(values))
    }

    /// Constructs a new boolean value.
    pub fn boolean(b: bool) -> RawValue {
        RawValue::Boolean(b)
    }

    /// Returns a null value.
    pub fn null() -> RawValue {
        RawValue::Null
    }

    /// Constructs a quoted string (e.g., `"hello world"`).
    pub fn quoted_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::quoted(s))
    }

    /// Constructs an unquoted string (e.g., `fooBar123`).
    pub fn unquoted_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::unquoted(s))
    }

    /// Constructs a multiline string (using triple quotes).
    pub fn multiline_string(s: impl Into<String>) -> RawValue {
        RawValue::String(RawString::multiline(s))
    }

    /// Constructs a path expression string (e.g., `a.b.c`).
    pub fn path_expression(paths: Vec<RawString>) -> RawValue {
        RawValue::String(RawString::path_expression(paths))
    }

    /// Constructs a numeric value.
    pub fn number(n: impl Into<Number>) -> RawValue {
        RawValue::Number(n.into())
    }

    /// Constructs a substitution expression (e.g., `${foo.bar}`).
    pub fn substitution(s: Substitution) -> RawValue {
        RawValue::Substitution(s)
    }

    /// Constructs a concatenated value consisting of multiple elements and optional spaces.
    /// Returns an error if invalid concatenation rules are violated.
    pub fn concat(values: Vec<RawValue>, spaces: Vec<Option<String>>) -> Result<RawValue> {
        Ok(RawValue::Concat(Concat::new(values, spaces)?))
    }

    /// Constructs an additive assignment value (e.g., `key += value`).
    pub fn add_assign(v: RawValue) -> RawValue {
        RawValue::AddAssign(AddAssign::new(v.into()))
    }
}

impl Display for RawValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RawValue::Object(object) => write!(f, "{}", object),
            RawValue::Array(array) => write!(f, "{}", array),
            RawValue::Boolean(boolean) => write!(f, "{}", boolean),
            RawValue::Null => write!(f, "null"),
            RawValue::String(string) => write!(f, "{}", string),
            RawValue::Number(number) => write!(f, "{}", number),
            RawValue::Substitution(substitution) => write!(f, "{}", substitution),
            RawValue::Concat(concat) => write!(f, "{}", concat),
            RawValue::AddAssign(add_assign) => write!(f, "{}", add_assign),
        }
    }
}

impl TryInto<RawArray> for RawValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<RawArray> {
        match self {
            RawValue::Array(a) => Ok(a),
            other => Err(crate::error::Error::InvalidConversion {
                from: other.ty(),
                to: RAW_ARRAY_TYPE,
            }),
        }
    }
}

impl TryInto<RawObject> for RawValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<RawObject> {
        match self {
            RawValue::Object(o) => Ok(o),
            other => Err(crate::error::Error::InvalidConversion {
                from: other.ty(),
                to: RAW_OBJECT_TYPE,
            }),
        }
    }
}

impl From<serde_json::Value> for RawValue {
    fn from(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::Null => RawValue::Null,
            serde_json::Value::Bool(boolean) => RawValue::Boolean(boolean),
            serde_json::Value::Number(number) => RawValue::Number(number),
            serde_json::Value::String(string) => RawValue::String(string.into()),
            serde_json::Value::Array(values) => {
                RawValue::array(values.into_iter().map(Into::into).collect())
            }
            serde_json::Value::Object(map) => {
                let fields = map
                    .into_iter()
                    .map(|(key, value)| ObjectField::key_value(key, value))
                    .collect();
                let raw_object = RawObject::new(fields);
                RawValue::Object(raw_object)
            }
        }
    }
}
