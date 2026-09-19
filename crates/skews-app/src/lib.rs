//! Shell kernel: single-writer state, effects-as-data updates and the
//! compile-time module registry.
//!
//! The kernel follows ADR-0003: every input becomes a [`Msg`], `update`
//! mutates state synchronously and returns [`Effects`] that the runtime
//! executes. Nothing here touches Wayland, the GPU or the filesystem, which
//! keeps behavior unit-testable without a display.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use skews_config::Config;
use skews_core::{Effect, Effects, LogLevel, ModuleId};

mod module;
mod registry;

pub use module::{Module, ModuleOutput};
pub use registry::{Registry, RegistryError};

/// Messages delivered to the kernel.
#[derive(Debug)]
pub enum Msg {
    /// A validated configuration is ready to be applied.
    ConfigLoaded(Box<Config>),
    /// Periodic tick for modules that need wall-clock time or sensors.
    Tick {
        /// Milliseconds since the Unix epoch, as reported by the runtime.
        unix_ms: u128,
    },
    /// A typed compositor event.
    Compositor(skews_ipc_hyprland::HyprEvent),
}

/// Errors produced while building or updating the kernel.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    /// The configuration is structurally invalid.
    #[error(transparent)]
    Config(#[from] skews_config::ConfigError),
    /// The configuration references a module the registry does not know.
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

/// A module placed in a bar region, with its current output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSlot {
    /// Module id as written in the configuration.
    pub id: ModuleId,
    /// Current output of the module.
    pub output: ModuleOutput,
}

/// The bar as composed by the kernel: regions in configuration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarPlan {
    /// Bar height in pixels.
    pub height: u32,
    /// Left region slots.
    pub left: Vec<ModuleSlot>,
    /// Center region slots.
    pub center: Vec<ModuleSlot>,
    /// Right region slots.
    pub right: Vec<ModuleSlot>,
}

/// Kernel state visible to the runtime.
#[derive(Debug, Clone)]
pub struct ShellState {
    /// Bar geometry and composition.
    pub bar: skews_config::BarConfig,
    /// Monotonic counter of applied configurations (starts at 1).
    pub config_revision: u64,
}

/// The shell kernel: owns state and modules, exposes `update` and views.
pub struct Shell {
    state: ShellState,
    registry: Registry,
    modules: BTreeMap<ModuleId, Box<dyn Module>>,
}

impl Shell {
    /// Builds the kernel from a validated configuration and a registry.
    ///
    /// Fails when the configuration references unknown modules or when a
    /// module rejects its options.
    pub fn new(config: Config, registry: Registry) -> Result<Self, ShellError> {
        let modules = Self::build_modules(&config, &registry)?;
        let state = ShellState {
            bar: config.bar,
            config_revision: 1,
        };

        Ok(Self {
            state,
            registry,
            modules,
        })
    }

    /// Returns the current state.
    #[must_use]
    pub fn state(&self) -> &ShellState {
        &self.state
    }

    /// Returns the registry used by this shell.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Applies a message and returns the effects to execute.
    pub fn update(&mut self, msg: Msg) -> Effects {
        match msg {
            Msg::ConfigLoaded(config) => self.apply_config(*config),
            Msg::Tick { .. } | Msg::Compositor(_) => self.forward(&msg),
        }
    }

    /// Composes the bar plan from configuration order and module outputs.
    #[must_use]
    pub fn bar_plan(&self) -> BarPlan {
        let slot = |id: &String| {
            let module_id = ModuleId::from(id.as_str());
            ModuleSlot {
                id: module_id.clone(),
                output: self
                    .modules
                    .get(&module_id)
                    .map_or(ModuleOutput::Empty, |module| module.output()),
            }
        };

        BarPlan {
            height: self.state.bar.height,
            left: self.state.bar.left.iter().map(slot).collect(),
            center: self.state.bar.center.iter().map(slot).collect(),
            right: self.state.bar.right.iter().map(slot).collect(),
        }
    }

    fn apply_config(&mut self, config: Config) -> Effects {
        match Self::build_modules(&config, &self.registry) {
            Ok(modules) => {
                self.modules = modules;
                self.state.bar = config.bar;
                self.state.config_revision += 1;
                vec![
                    Effect::Log {
                        level: LogLevel::Info,
                        message: format!(
                            "configuration applied (revision {})",
                            self.state.config_revision
                        ),
                    },
                    Effect::Redraw,
                ]
            }
            Err(error) => vec![Effect::Log {
                level: LogLevel::Warn,
                message: format!("configuration rejected, keeping the previous one: {error}"),
            }],
        }
    }

    fn forward(&mut self, msg: &Msg) -> Effects {
        let mut effects = Vec::new();
        for module in self.modules.values_mut() {
            effects.extend(module.update(msg));
        }
        effects
    }

    fn build_modules(
        config: &Config,
        registry: &Registry,
    ) -> Result<BTreeMap<ModuleId, Box<dyn Module>>, ShellError> {
        let mut modules = BTreeMap::new();

        for (region, ids) in config.bar.regions() {
            for id in ids {
                let module_id = ModuleId::from(id.as_str());
                if modules.contains_key(&module_id) {
                    // Duplicates are rejected by `Config::validate`; keep a
                    // defensive error for programmatic construction.
                    return Err(ShellError::Config(skews_config::ConfigError::Validation(
                        format!("module `{id}` is listed twice (region `{region}`)"),
                    )));
                }

                let options = config
                    .modules
                    .get(id.as_str())
                    .cloned()
                    .unwrap_or(toml::Value::Table(toml::Table::new()));

                let module = registry.build(&module_id, &options)?;
                modules.insert(module_id, module);
            }
        }

        Ok(modules)
    }
}

#[cfg(test)]
mod tests {
    use super::{Msg, Shell, ShellError};
    use crate::registry::Registry;
    use crate::{Module, ModuleOutput};
    use skews_config::Config;
    use skews_core::{Effect, LogLevel, ModuleId};

    struct TestClock {
        id: ModuleId,
        ticks: u64,
    }

    impl Module for TestClock {
        fn id(&self) -> &ModuleId {
            &self.id
        }

        fn update(&mut self, msg: &Msg) -> skews_core::Effects {
            if matches!(msg, Msg::Tick { .. }) {
                self.ticks += 1;
                vec![Effect::Redraw]
            } else {
                Vec::new()
            }
        }

        fn output(&self) -> ModuleOutput {
            ModuleOutput::Text(format!("ticks={}", self.ticks))
        }
    }

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry.register("clock", |_options| {
            Ok(Box::new(TestClock {
                id: ModuleId::from("clock"),
                ticks: 0,
            }))
        });
        registry.register("battery", |_options| {
            Ok(Box::new(TestClock {
                id: ModuleId::from("battery"),
                ticks: 0,
            }))
        });
        registry.register("cpu", |_options| {
            Ok(Box::new(TestClock {
                id: ModuleId::from("cpu"),
                ticks: 0,
            }))
        });
        registry
    }

    fn config(left: &[&str], center: &[&str], right: &[&str], height: u32) -> Config {
        let quoted = |items: &[&str]| {
            items
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Config::from_toml_str(&format!(
            "[bar]\nheight = {height}\nleft = [{}]\ncenter = [{}]\nright = [{}]\n",
            quoted(left),
            quoted(center),
            quoted(right)
        ))
        .expect("test config is valid")
    }

    #[test]
    fn unknown_modules_are_rejected_with_known_ids() {
        let error = match Shell::new(config(&["nope"], &[], &[], 32), registry()) {
            Err(error) => error,
            Ok(_) => panic!("expected the unknown module to be rejected"),
        };

        match error {
            ShellError::Registry(crate::RegistryError::UnknownModule { id, known }) => {
                assert_eq!(id.as_str(), "nope");
                assert!(known.contains(&ModuleId::from("clock")));
                assert!(known.contains(&ModuleId::from("battery")));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn bar_plan_follows_configuration_order() {
        let shell =
            Shell::new(config(&["clock"], &[], &["battery", "cpu"], 40), registry()).unwrap();

        let plan = shell.bar_plan();
        assert_eq!(plan.height, 40);
        assert_eq!(plan.left.len(), 1);
        assert_eq!(plan.left[0].id.as_str(), "clock");
        assert_eq!(plan.right[0].id.as_str(), "battery");
        assert_eq!(plan.right[1].id.as_str(), "cpu");
        assert_eq!(shell.state().config_revision, 1);
    }

    #[test]
    fn ticks_reach_modules_and_aggregate_effects() {
        let mut shell = Shell::new(config(&["clock"], &[], &[], 32), registry()).unwrap();

        let effects = shell.update(Msg::Tick { unix_ms: 42 });

        assert_eq!(effects, vec![Effect::Redraw]);
        assert_eq!(
            shell.bar_plan().left[0].output,
            ModuleOutput::Text(String::from("ticks=1"))
        );
    }

    #[test]
    fn compositor_events_are_forwarded_without_effects_by_default() {
        let mut shell = Shell::new(config(&["clock"], &[], &[], 32), registry()).unwrap();

        let effects = shell.update(Msg::Compositor(skews_ipc_hyprland::HyprEvent::Workspace {
            name: String::from("2"),
        }));

        assert!(effects.is_empty());
    }

    #[test]
    fn config_reload_swaps_layout_and_bumps_revision() {
        let mut shell = Shell::new(config(&["clock"], &[], &[], 32), registry()).unwrap();

        let effects = shell.update(Msg::ConfigLoaded(Box::new(config(
            &[],
            &["battery"],
            &[],
            48,
        ))));

        assert!(matches!(
            effects[0],
            Effect::Log {
                level: LogLevel::Info,
                ..
            }
        ));
        assert_eq!(effects[1], Effect::Redraw);
        assert_eq!(shell.state().config_revision, 2);
        assert_eq!(shell.state().bar.height, 48);
        assert!(shell.bar_plan().left.is_empty());
        assert_eq!(shell.bar_plan().center[0].id.as_str(), "battery");
    }

    #[test]
    fn invalid_reload_keeps_previous_state() {
        let mut shell = Shell::new(config(&["clock"], &[], &[], 32), registry()).unwrap();

        let effects = shell.update(Msg::ConfigLoaded(Box::new(config(
            &["ghost"],
            &[],
            &[],
            32,
        ))));

        assert!(matches!(
            effects[0],
            Effect::Log {
                level: LogLevel::Warn,
                ..
            }
        ));
        assert_eq!(shell.state().config_revision, 1);
        assert_eq!(shell.bar_plan().left[0].id.as_str(), "clock");
    }
}
