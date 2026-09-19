//! Workspaces module: shows the active Hyprland workspace.
//!
//! The module consumes typed compositor events; the initial workspace is
//! queried by the runtime at startup and delivered as a [`Msg::Compositor`]
//! message.

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effect, Effects, ModuleId};
use skews_ipc_hyprland::HyprEvent;

/// Shows the active workspace name.
pub struct Workspaces {
    id: ModuleId,
    active: Option<String>,
}

impl Workspaces {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self {
            id: ModuleId::from("workspaces"),
            active: None,
        })
    }

    fn apply(&mut self, name: String) -> Effects {
        if self.active.as_deref() == Some(name.as_str()) {
            return Vec::new();
        }
        self.active = Some(name);
        vec![Effect::Redraw]
    }
}

impl Module for Workspaces {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Compositor(HyprEvent::Workspace { name }) => self.apply(name.clone()),
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        match &self.active {
            Some(name) => ModuleOutput::Text(name.clone()),
            None => ModuleOutput::Empty,
        }
    }
}

/// Registers the workspaces module.
pub fn register(registry: &mut Registry) {
    registry.register("workspaces", |options| {
        Workspaces::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Workspaces;
    use skews_app::{Module, ModuleOutput, Msg};
    use skews_core::Effect;
    use skews_ipc_hyprland::HyprEvent;

    #[test]
    fn tracks_the_active_workspace() {
        let mut module = Workspaces::new(&toml::Value::Table(toml::Table::new())).unwrap();

        assert_eq!(module.output(), ModuleOutput::Empty);

        let effects = module.update(&Msg::Compositor(HyprEvent::Workspace {
            name: String::from("2"),
        }));
        assert_eq!(effects, vec![Effect::Redraw]);
        assert_eq!(module.output(), ModuleOutput::Text(String::from("2")));

        // Same workspace again: no redraw.
        assert!(
            module
                .update(&Msg::Compositor(HyprEvent::Workspace {
                    name: String::from("2")
                }))
                .is_empty()
        );

        module.update(&Msg::Compositor(HyprEvent::Workspace {
            name: String::from("web"),
        }));
        assert_eq!(module.output(), ModuleOutput::Text(String::from("web")));
    }

    #[test]
    fn ignores_unrelated_events() {
        let mut module = Workspaces::new(&toml::Value::Table(toml::Table::new())).unwrap();

        let effects = module.update(&Msg::Compositor(HyprEvent::ActiveWindow {
            class: String::from("kitty"),
            title: String::from("~"),
        }));

        assert!(effects.is_empty());
        assert_eq!(module.output(), ModuleOutput::Empty);
    }
}
