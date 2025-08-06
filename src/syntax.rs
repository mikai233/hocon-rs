use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
enum Syntax {
    Conf,
    Json,
    Properties,
}

impl Display for Syntax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Syntax::Conf => write!(f, "conf"),
            Syntax::Json => write!(f, "json"),
            Syntax::Properties => write!(f, "properties"),
        }
    }
}