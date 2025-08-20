use std::{fmt::Debug, sync::Arc};

use crate::syntax::Syntax;
use derive_more::Constructor;

#[derive(Debug, Clone, Eq, PartialEq, Constructor)]
pub struct ConfigOptions {
    pub max_include_depth: u8,
    pub use_system_environment: bool,
    pub override_options: OverrideOptions,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            max_include_depth: 50,
            use_system_environment: true,
            override_options: OverrideOptions::default(),
        }
    }
}

#[derive(Clone)]
pub struct OverrideOptions {
    pub allow_override: bool,
    pub compare: Arc<Box<dyn Fn(&Syntax, &Syntax) -> std::cmp::Ordering>>,
}

impl OverrideOptions {
    fn new<C>(allow_override: bool, compare: C) -> Self
    where
        C: Fn(&Syntax, &Syntax) -> std::cmp::Ordering + 'static,
    {
        Self {
            allow_override,
            compare: Arc::new(Box::new(compare)),
        }
    }
}

impl Default for OverrideOptions {
    fn default() -> Self {
        Self {
            allow_override: true,
            compare: Arc::new(Box::new(Syntax::cmp)),
        }
    }
}

impl Debug for OverrideOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverrideOptions")
            .field("allow_override", &self.allow_override)
            .finish_non_exhaustive()
    }
}

impl PartialEq for OverrideOptions {
    fn eq(&self, other: &Self) -> bool {
        self.allow_override == other.allow_override && Arc::ptr_eq(&self.compare, &other.compare)
    }
}

impl Eq for OverrideOptions {}
