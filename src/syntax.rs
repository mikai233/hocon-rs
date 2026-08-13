use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum Syntax {
    Hocon,
    Json,
    Properties,
}

impl Display for Syntax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Syntax::Hocon => write!(f, "conf"),
            Syntax::Json => write!(f, "json"),
            Syntax::Properties => write!(f, "properties"),
        }
    }
}

impl Syntax {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Syntax::Hocon => "conf",
            Syntax::Json => "json",
            Syntax::Properties => "properties",
        }
    }

    pub(crate) fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "conf" => Some(Self::Hocon),
            "json" => Some(Self::Json),
            "properties" => Some(Self::Properties),
            _ => None,
        }
    }

    pub(crate) fn enabled() -> impl Iterator<Item = Self> {
        [
            Some(Self::Hocon),
            Some(Self::Json),
            cfg!(feature = "properties").then_some(Self::Properties),
        ]
        .into_iter()
        .flatten()
    }

    #[cfg(feature = "urls_includes")]
    pub(crate) fn from_content_type(content_type: &str) -> Option<Self> {
        match content_type {
            "application/hocon" => Some(Self::Hocon),
            "application/json" => Some(Self::Json),
            "text/x-java-properties" => Some(Self::Properties),
            _ => None,
        }
    }
}
