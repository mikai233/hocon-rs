use std::{fmt::Debug, rc::Rc};

use crate::syntax::Syntax;

#[derive(Clone)]
pub struct ConfigOptions {
    pub use_system_environment: bool,
    pub compare: Rc<Box<dyn Fn(&Syntax, &Syntax) -> std::cmp::Ordering>>,
    pub classpath: Rc<Vec<String>>,
}

impl ConfigOptions {
    pub fn new(use_system_env: bool, classpath: Vec<String>) -> Self {
        Self {
            use_system_environment: use_system_env,
            compare: Rc::new(Box::new(Syntax::cmp)),
            classpath: Rc::new(classpath),
        }
    }

    pub fn with_compare<C>(use_system_env: bool, classpath: Vec<String>, compare: C) -> Self
    where
        C: Fn(&Syntax, &Syntax) -> std::cmp::Ordering + 'static,
    {
        Self {
            use_system_environment: use_system_env,
            compare: Rc::new(Box::new(compare)),
            classpath: Rc::new(classpath),
        }
    }
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            use_system_environment: false,
            compare: Rc::new(Box::new(Syntax::cmp)),
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
            && Rc::ptr_eq(&self.compare, &other.compare)
            && self.classpath == other.classpath
    }
}

impl Eq for ConfigOptions {}
