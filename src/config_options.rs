use std::{fmt::Debug, rc::Rc};

use crate::syntax::Syntax;

pub(crate) const MAX_DEPTH: usize = 64;

pub(crate) const MAX_INCLUDE_DEPTH: usize = 64;

pub type CompareFn = Rc<dyn Fn(&Syntax, &Syntax) -> std::cmp::Ordering>;

#[derive(Clone)]
pub struct ConfigOptions {
    pub use_system_environment: bool,
    pub compare: CompareFn,
    pub classpath: Rc<Vec<String>>,
    pub max_depth: usize,
    pub max_include_depth: usize,
}

impl ConfigOptions {
    pub fn new(use_system_env: bool, classpath: Vec<String>) -> Self {
        Self {
            use_system_environment: use_system_env,
            compare: Rc::new(Syntax::cmp),
            classpath: Rc::new(classpath),
            ..Default::default()
        }
    }

    pub fn with_compare<C>(use_system_env: bool, classpath: Vec<String>, compare: C) -> Self
    where
        C: Fn(&Syntax, &Syntax) -> std::cmp::Ordering + 'static,
    {
        Self {
            use_system_environment: use_system_env,
            compare: Rc::new(compare),
            classpath: Rc::new(classpath),
            ..Default::default()
        }
    }
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            use_system_environment: false,
            compare: Rc::new(Syntax::cmp),
            classpath: Default::default(),
            max_depth: MAX_DEPTH,
            max_include_depth: MAX_INCLUDE_DEPTH,
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
            && Rc::ptr_eq(&self.compare, &other.compare)
            && self.classpath == other.classpath
    }
}

impl Eq for ConfigOptions {}
