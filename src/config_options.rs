use std::{fmt::Debug, sync::Arc};

use crate::syntax::Syntax;
use derive_more::Constructor;

#[derive(Clone, Constructor)]
pub struct ConfigOptions {
    pub use_system_environment: bool,
    pub compare: Arc<Box<dyn Fn(&Syntax, &Syntax) -> std::cmp::Ordering>>,
    pub classpath: Vec<String>,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            use_system_environment: false,
            compare: Arc::new(Box::new(Syntax::cmp)),
            classpath: Default::default(),
        }
    }
}

impl Debug for ConfigOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigOptions")
            .field("use_system_environment", &self.use_system_environment)
            .field("classpath", &self.classpath)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ConfigOptions {
    fn eq(&self, other: &Self) -> bool {
        self.use_system_environment == other.use_system_environment
            && Arc::ptr_eq(&self.compare, &other.compare)
            && self.classpath == other.classpath
    }
}

impl Eq for ConfigOptions {}
