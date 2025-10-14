use std::{
    fmt::{Display, Formatter, Write},
    rc::Rc,
};

use derive_more::Constructor;

use crate::path::{Key, Path};

/// Represents a **HOCON substitution reference** in the merge phase.
///
/// This structure is used after parsing, during the **merge and resolution**
/// stages of the HOCON configuration lifecycle. It corresponds to a substitution
/// expression such as:
///
/// ```hocon
/// database.url = ${connection.base}
/// optional     = ${?system.env}
/// ```
///
/// Unlike [`crate::raw::substitution::Substitution`], which stores raw string
/// data directly from the source file, this version holds a fully parsed and
/// reference-counted [`Path`] object that can be used for efficient value
/// resolution and merging.
///
/// # Fields
///
/// - [`path`]: A shared reference-counted [`Path`] representing the lookup path
///   (e.g. `"connection.base"` or `"system.env"`).
/// - [`optional`]: Whether this substitution is optional (`${?...}` syntax).
///
/// # Behavior
///
/// - When `optional` is `true`, missing paths do not produce an error; the
///   substitution is silently ignored.
/// - When `optional` is `false`, missing paths cause resolution failure.
///
/// # See also
/// - [`crate::merge::value::Value`] — where this type is used during resolution.
/// - [`Path`] — underlying path structure for configuration lookups.
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Constructor)]
pub(crate) struct Substitution {
    /// The resolved configuration path this substitution points to.
    ///
    /// Stored as a reference-counted [`Rc<Path>`] to allow sharing between
    /// different configuration nodes during merge operations.
    pub(crate) path: Rc<Path>,

    /// Indicates whether this substitution is optional (`${?path}`).
    ///
    /// Optional substitutions will not raise errors if the referenced path
    /// cannot be found during resolution.
    pub(crate) optional: bool,
}

impl Substitution {
    /// Returns the full string representation of this substitution’s path.
    ///
    /// The result is a flattened version of the path (e.g. `"foo.bar.0.name"`),
    /// reconstructed from the internal [`Path`] structure.
    pub(crate) fn full_path(&self) -> String {
        self.path.iter().fold(String::new(), |mut acc, next| {
            match &next.first {
                Key::String(s) => {
                    acc.push_str(s);
                }
                Key::Index(i) => {
                    write!(&mut acc, "{i}").unwrap();
                }
            }
            if next.remainder.is_some() {
                acc.push('.');
            }
            acc
        })
    }
}

impl Display for Substitution {
    /// Formats this substitution into valid HOCON syntax.
    ///
    /// Example outputs:
    /// - `${database.url}`
    /// - `${?system.env}`
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

impl From<crate::raw::substitution::Substitution> for Substitution {
    /// Converts a raw substitution node from the parser into a merge-phase
    /// substitution with a resolved [`Path`].
    ///
    /// This conversion is part of the parsing pipeline where raw syntax trees
    /// are transformed into semantic configuration structures.
    fn from(value: crate::raw::substitution::Substitution) -> Self {
        let path = value.path.into_path().into();
        Self::new(path, value.optional)
    }
}
