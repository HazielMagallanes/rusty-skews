//! Battery module: charge percentage and charging state.
//!
//! ```toml
//! [modules.battery]
//! # optional: power-supply root for tests or unusual systems
//! # root = "/sys/class/power_supply"
//! ```

use std::path::PathBuf;

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effect, Effects, ModuleId};
use skews_sys_linux::battery_status;

/// Shows the battery percentage, with a `+` suffix while charging.
pub struct Battery {
    id: ModuleId,
    root: PathBuf,
    text: String,
}

impl Battery {
    /// Builds the module from its configuration options.
    pub fn new(options: &toml::Value) -> Result<Self, String> {
        let root = match options.get("root") {
            Some(value) => {
                let Some(path) = value.as_str() else {
                    return Err(String::from("`root` must be a string path"));
                };
                PathBuf::from(path)
            }
            None => PathBuf::from("/sys/class/power_supply"),
        };

        Ok(Self {
            id: ModuleId::from("battery"),
            root,
            text: String::new(),
        })
    }

    fn refresh(&mut self) -> Effects {
        let text = match battery_status(&self.root) {
            Some(status) if status.charging => format!("{}%+", status.percent),
            Some(status) => format!("{}%", status.percent),
            None => String::from("--%"),
        };
        self.apply(text)
    }

    fn apply(&mut self, text: String) -> Effects {
        if text == self.text {
            return Vec::new();
        }
        self.text = text;
        vec![Effect::Redraw]
    }
}

impl Module for Battery {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => self.refresh(),
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        if self.text.is_empty() {
            ModuleOutput::Empty
        } else {
            ModuleOutput::Text(self.text.clone())
        }
    }
}

/// Registers the battery module.
pub fn register(registry: &mut Registry) {
    registry.register("battery", |options| {
        Battery::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Battery;
    use skews_app::{Module, ModuleOutput};
    use std::path::Path;

    fn options(fixture: &str) -> toml::Value {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let mut table = toml::Table::new();
        table.insert(
            String::from("root"),
            toml::Value::String(root.to_string_lossy().into_owned()),
        );
        toml::Value::Table(table)
    }

    #[test]
    fn renders_discharging_percentage() {
        let mut module = Battery::new(&options("power-supply")).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("94%")));
    }

    #[test]
    fn marks_charging_batteries() {
        let mut module = Battery::new(&options("power-supply-charging")).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("41%+")));
    }

    #[test]
    fn missing_battery_renders_placeholder() {
        let mut module = Battery::new(&options("power-supply-none")).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("--%")));
    }
}
