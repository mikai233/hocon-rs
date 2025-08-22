use crate::raw::comment::Comment;
use crate::raw::include::Inclusion;
use crate::raw::raw_string::RawString;
use crate::raw::raw_value::RawValue;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectField {
    Inclusion {
        inclusion: Inclusion,
        comment: Option<Comment>,
    },
    KeyValue {
        key: RawString,
        value: RawValue,
        comment: Option<Comment>,
    },
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
