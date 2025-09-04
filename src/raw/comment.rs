use derive_more::{Constructor, Deref, DerefMut};
use std::fmt::{Display, Formatter};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CommentType {
    DoubleSlash,
    Hash,
}

impl Display for CommentType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommentType::DoubleSlash => write!(f, "//"),
            CommentType::Hash => write!(f, "#"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Constructor, Deref, DerefMut)]
pub struct Comment {
    #[deref]
    #[deref_mut]
    pub content: String,
    pub ty: CommentType,
}

impl Comment {
    pub fn double_slash(comment: impl Into<String>) -> Comment {
        Comment::new(comment.into(), CommentType::DoubleSlash)
    }

    pub fn hash(comment: impl Into<String>) -> Comment {
        Comment::new(comment.into(), CommentType::Hash)
    }
}

impl Display for Comment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.ty, self.content)
    }
}

impl From<&str> for Comment {
    fn from(val: &str) -> Self {
        Comment::new(String::from(val), CommentType::DoubleSlash)
    }
}

impl From<String> for Comment {
    fn from(val: String) -> Self {
        Comment::new(val, CommentType::Hash)
    }
}
