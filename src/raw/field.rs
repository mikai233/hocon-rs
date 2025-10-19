use crate::raw::comment::Comment;
use crate::raw::include::Inclusion;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

/// Represents a single field (or element) within a [`RawObject`].
///
/// A field may be:
/// - an inclusion directive (`include`, `include required`, etc.)
/// - a key-value pair
/// - a standalone comment line
///
/// This enum preserves the syntactic structure of HOCON before
/// semantic merging or substitution resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectField {
    /// An `include` directive that brings external content into the object.
    ///
    /// Example:
    /// ```hocon
    /// include "application.conf"
    /// include required("defaults.conf")
    /// ```
    Inclusion {
        /// The inclusion target and its inclusion type.
        inclusion: Inclusion,

        /// An optional comment associated with this inclusion.
        comment: Option<Comment>,
    },

    /// A standard key-value field (e.g., `key = value` or `nested { ... }`).
    ///
    /// Example:
    /// ```hocon
    /// host = "localhost"
    /// database {
    ///   user = "admin"
    /// }
    /// ```
    KeyValue {
        /// The key of the field (can be unquoted, quoted, or path-like).
        key: RawString,

        /// The value associated with the key, which may be any [`RawValue`].
        value: RawValue,

        /// An optional inline or trailing comment.
        comment: Option<Comment>,
    },

    /// A standalone comment line that appears between or after fields.
    ///
    /// Example:
    /// ```hocon
    /// # This is a comment
    /// ```
    NewlineComment(Comment),
}

impl ObjectField {
    pub fn inclusion(inclusion: Inclusion) -> ObjectField {
        ObjectField::Inclusion {
            inclusion,
            comment: None,
        }
    }

    pub fn inclusion_with_comment(
        inclusion: Inclusion,
        comment: impl Into<Comment>,
    ) -> ObjectField {
        ObjectField::Inclusion {
            inclusion,
            comment: Some(comment.into()),
        }
    }

    pub fn key_value(key: impl Into<RawString>, value: impl Into<RawValue>) -> ObjectField {
        ObjectField::KeyValue {
            key: key.into(),
            value: value.into(),
            comment: None,
        }
    }

    pub fn key_value_with_comment(
        key: impl Into<RawString>,
        value: impl Into<RawValue>,
        comment: impl Into<Comment>,
    ) -> ObjectField {
        ObjectField::KeyValue {
            key: key.into(),
            value: value.into(),
            comment: Some(comment.into()),
        }
    }

    pub fn newline_comment(comment: impl Into<Comment>) -> ObjectField {
        ObjectField::NewlineComment(comment.into())
    }

    pub fn set_comment(&mut self, comment: Comment) {
        match self {
            ObjectField::Inclusion { comment: c, .. }
            | ObjectField::KeyValue { comment: c, .. } => *c = Some(comment),
            ObjectField::NewlineComment(c) => *c = comment,
        }
    }
}

impl Display for ObjectField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectField::Inclusion { inclusion, comment } => {
                write!(f, "{}", inclusion)?;
                if let Some(comment) = comment {
                    write!(f, " {}", comment)?;
                }
            }
            ObjectField::KeyValue {
                key,
                value,
                comment,
            } => {
                write!(f, "{}: {}", key, value)?;
                if let Some(comment) = comment {
                    write!(f, " {}", comment)?;
                }
            }
            ObjectField::NewlineComment(c) => {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}
