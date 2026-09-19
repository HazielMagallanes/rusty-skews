//! Compile-time module registry (see ADR-0004).
//!
//! Modules are registered by the binary at startup; the kernel resolves
//! configuration ids against this registry and builds modules through their
//! factories.

use std::collections::BTreeMap;

use skews_core::ModuleId;

use crate::module::Module;

type Factory = Box<dyn Fn(&toml::Value) -> Result<Box<dyn Module>, String> + Send + Sync>;

/// Registry of known modules, populated at startup.
#[derive(Default)]
pub struct Registry {
    factories: BTreeMap<ModuleId, Factory>,
}

/// Errors produced while resolving modules.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// The configuration references a module that is not registered.
    #[error("unknown module `{id}` (known modules: {})", known.iter().map(ModuleId::as_str).collect::<Vec<_>>().join(", "))]
    UnknownModule {
        /// Unknown module id.
        id: ModuleId,
        /// Modules the registry knows about, for the error message.
        known: Vec<ModuleId>,
    },
    /// The module factory rejected its options.
    #[error("module `{id}` rejected its options: {detail}")]
    OptionsRejected {
        /// Module id.
        id: ModuleId,
        /// Why the options were rejected.
        detail: String,
    },
}

impl Registry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a module factory under an id.
    pub fn register<F>(&mut self, id: impl Into<ModuleId>, factory: F)
    where
        F: Fn(&toml::Value) -> Result<Box<dyn Module>, String> + Send + Sync + 'static,
    {
        self.factories.insert(id.into(), Box::new(factory));
    }

    /// Returns `true` when the registry knows the module.
    #[must_use]
    pub fn contains(&self, id: &ModuleId) -> bool {
        self.factories.contains_key(id)
    }

    /// Iterates over the registered module ids in lexicographic order.
    pub fn ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.factories.keys()
    }

    /// Builds a module, rejecting unknown ids and bad options.
    pub fn build(
        &self,
        id: &ModuleId,
        options: &toml::Value,
    ) -> Result<Box<dyn Module>, RegistryError> {
        let Some(factory) = self.factories.get(id) else {
            return Err(RegistryError::UnknownModule {
                id: id.clone(),
                known: self.ids().cloned().collect(),
            });
        };

        factory(options).map_err(|detail| RegistryError::OptionsRejected {
            id: id.clone(),
            detail,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Registry, RegistryError};
    use crate::Msg;
    use crate::module::{Module, ModuleOutput};
    use skews_core::{Effects, ModuleId};

    struct Stub(ModuleId);

    impl Module for Stub {
        fn id(&self) -> &ModuleId {
            &self.0
        }

        fn update(&mut self, _msg: &Msg) -> Effects {
            Vec::new()
        }

        fn output(&self) -> ModuleOutput {
            ModuleOutput::Empty
        }
    }

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry.register("clock", |options| {
            if options.get("bad").is_some() {
                return Err(String::from("`bad` is not a valid option"));
            }
            Ok(Box::new(Stub(ModuleId::from("clock"))))
        });
        registry
    }

    #[test]
    fn unknown_ids_report_the_known_set() {
        let error = match registry().build(
            &ModuleId::from("ghost"),
            &toml::Value::Table(toml::Table::new()),
        ) {
            Err(error) => error,
            Ok(_) => panic!("expected the unknown id to be rejected"),
        };

        match error {
            RegistryError::UnknownModule { id, known } => {
                assert_eq!(id.as_str(), "ghost");
                assert_eq!(known, vec![ModuleId::from("clock")]);
            }
            other => panic!("unexpected error: {other}"),
        }
        assert!(registry().ids().any(|id| id.as_str() == "clock"));
        assert!(registry().contains(&ModuleId::from("clock")));
    }

    #[test]
    fn factory_rejections_become_options_errors() {
        let mut table = toml::Table::new();
        table.insert(String::from("bad"), toml::Value::Boolean(true));

        let error = match registry().build(&ModuleId::from("clock"), &toml::Value::Table(table)) {
            Err(error) => error,
            Ok(_) => panic!("expected the options to be rejected"),
        };

        assert!(
            matches!(error, RegistryError::OptionsRejected { detail, .. } if detail.contains("bad"))
        );
    }
}
