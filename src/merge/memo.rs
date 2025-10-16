use crate::path::Path;

/// Tracks recursive substitutions during HOCON value resolution.
///
/// This structure serves as a recursion guard to detect and prevent
/// infinite loops or excessive nesting depth when resolving substitution
/// expressions (e.g. `${a.b.c}`).
///
/// During value expansion, each visited `Path` is pushed into `tracker`.
/// Before resolving a new substitution, the parser checks whether the same
/// path already exists in the tracker.
/// If it does, it indicates a **cyclic reference** such as:
///
/// ```hocon
/// a = ${b}
/// b = ${a}
/// ```
///
/// Additionally, the `substitution_counter` tracks the total number of
/// performed substitutions to prevent stack overflow in deeply nested
/// structures, for example:
///
/// ```hocon
/// a = { b = [{ c = "hello" }] }
/// ```
///
/// In this case, the traversal path would be represented as `a.b.0.c`.
///
/// # Fields
/// - `tracker`: A stack of paths representing the current substitution
///   resolution chain. Used to detect recursion.
/// - `substitution_counter`: Counts the total number of performed
///   substitutions, used for recursion depth control.
#[derive(Debug, Default)]
pub(crate) struct Memo {
    /// Stack of currently active substitution paths.
    /// Used to detect cyclic references like `${a}` → `${b}` → `${a}`.
    pub(crate) tracker: Vec<Path>,

    /// Counter to track the number of performed substitutions.
    /// Helps limit recursion depth to avoid stack overflow.
    pub(crate) substitution_counter: usize,
}
