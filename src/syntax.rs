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

#[cfg(test)]
mod tests {
    use crate::syntax::Syntax;

    #[test]
    fn test_syntax() {
        assert_eq!(Syntax::Hocon.to_string(), "conf");
        assert_eq!(Syntax::Json.to_string(), "json");
        assert_eq!(Syntax::Properties.to_string(), "properties");
    }
}
