//! Placeholder modules for ids referenced by the default configuration.
//!
//! These keep configuration validation green while their real implementations
//! land in later M1 slices: `workspaces` needs the Hyprland IPC client and
//! `battery` needs the power-supply reader.

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effects, ModuleId};

struct Stub {
    id: ModuleId,
}

impl Module for Stub {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, _msg: &Msg) -> Effects {
        Vec::new()
    }

    fn output(&self) -> ModuleOutput {
        ModuleOutput::Empty
    }
}

fn register_stub(registry: &mut Registry, id: &'static str) {
    registry.register(id, move |_options| {
        Ok(Box::new(Stub {
            id: ModuleId::from(id),
        }) as Box<dyn Module>)
    });
}

/// Registers the placeholder modules.
pub fn register_all(registry: &mut Registry) {
    register_stub(registry, "workspaces");
    register_stub(registry, "battery");
}

#[cfg(test)]
mod tests {
    use super::register_all;
    use skews_app::Registry;
    use skews_core::ModuleId;

    #[test]
    fn stubs_are_registered_and_buildable() {
        let mut registry = Registry::new();
        register_all(&mut registry);

        for id in ["workspaces", "battery"] {
            assert!(registry.contains(&ModuleId::from(id)));
            assert!(
                registry
                    .build(&ModuleId::from(id), &toml::Value::Table(toml::Table::new()))
                    .is_ok()
            );
        }
    }
}
