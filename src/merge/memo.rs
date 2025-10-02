use crate::path::Path;

#[derive(Debug, Default)]
pub(crate) struct Memo {
    pub(crate) tracker: Vec<Path>,
    pub(crate) substitution_counter: usize,
}
