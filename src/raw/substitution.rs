use crate::raw::raw_string::RawString;
use std::fmt::{Debug, Display, Formatter};

/// Represents a **HOCON substitution expression**.
///
/// In HOCON (Human-Optimized Config Object Notation),
/// substitution expressions allow referencing other configuration values.
/// The syntax looks like:
///
/// ```hocon
/// a = ${b.c}
/// optional = ${?x.y}
/// ```
///
/// This structure is used to represent such expressions in the AST (Abstract Syntax Tree).
///
/// # Fields
/// - [`path`]: the path being referenced (e.g. `"b.c"` or `"x.y"`).
/// - [`optional`]: indicates whether this is an *optional substitution* (`${?...}`).
///
/// # Behavior
/// - If `optional` is `true`, missing values during resolution will not produce an error.
/// - If `optional` is `false`, a missing reference will trigger a substitution error.
///
/// # Examples
///
/// ```rust
/// use hocon_rs::raw::substitution::Substitution;
/// use hocon_rs::raw::raw_string::RawString;
///
/// let normal = Substitution::new(RawString::path_expression(vec![RawString::unquoted("foo"),RawString::unquoted("bar")]), false);
/// let optional = Substitution::new(RawString::path_expression(vec![RawString::unquoted("x"),RawString::unquoted("y")]), true);
///
/// assert_eq!(format!("{}", normal), "${foo.bar}");
/// assert_eq!(format!("{}", optional), "${?x.y}");
/// ```
#[derive(Eq, PartialEq, Hash, Clone, derive_more::Constructor)]
pub struct Substitution {
    /// The referenced path, e.g. `"foo.bar"` or `"config.value"`.
    pub path: RawString,

    /// Indicates whether this substitution is optional (`${?path}`).
    ///
    /// When `true`, unresolved substitutions will not cause an error.
    /// When `false`, missing references will trigger an evaluation failure.
    pub optional: bool,
}

impl Display for Substitution {
    /// Formats the substitution into standard HOCON syntax.
    ///
    /// Examples:
    /// - `${x.y}`
    /// - `${?x.y}`
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "${{")?;
        if self.optional {
            write!(f, "?")?;
        }
        write!(f, "{}", self.path)?;
        write!(f, "}}")?;
        Ok(())
    }
}

impl Debug for Substitution {
    /// Displays the substitution in a debug-friendly format.
    ///
    /// Unlike [`Display`], this version uses the `{:?}` formatter for `path`
    /// so that escaped characters or raw data can be inspected more easily.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "${{")?;
        if self.optional {
            write!(f, "?")?;
        }
        write!(f, "{:?}", self.path)?;
        write!(f, "}}")?;
        Ok(())
    }
}
