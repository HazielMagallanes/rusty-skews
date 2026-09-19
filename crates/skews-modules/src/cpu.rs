//! CPU temperature module (Intel `coretemp` and AMD `k10temp`).
//!
//! ```toml
//! [modules.cpu]
//! # optional: hwmon root for tests or unusual systems
//! # hwmon = "/sys/class/hwmon"
//! ```

use std::path::PathBuf;

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effect, Effects, ModuleId};
use skews_sys_linux::SysPaths;

/// Shows the CPU package temperature in Celsius.
pub struct CpuTemp {
    id: ModuleId,
    paths: SysPaths,
    text: String,
}

impl CpuTemp {
    /// Builds the module from its configuration options.
    pub fn new(options: &toml::Value) -> Result<Self, String> {
        let mut paths = SysPaths::default();

        if let Some(hwmon) = options.get("hwmon") {
            let Some(root) = hwmon.as_str() else {
                return Err(String::from("`hwmon` must be a string path"));
            };
            paths.hwmon = PathBuf::from(root);
        }

        Ok(Self {
            id: ModuleId::from("cpu"),
            paths,
            text: String::new(),
        })
    }

    fn refresh(&mut self) -> Effects {
        let text = match skews_sys_linux::cpu_temp_celsius(&self.paths.hwmon) {
            Some(celsius) => format!("{}°C", celsius.round() as i64),
            None => String::from("--°C"),
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

impl Module for CpuTemp {
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

/// Registers the CPU temperature module.
pub fn register(registry: &mut Registry) {
    registry.register("cpu", |options| {
        CpuTemp::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::CpuTemp;
    use skews_app::{Module, ModuleOutput};
    use std::path::Path;

    fn fixture_options() -> toml::Value {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hwmon-intel");
        let mut table = toml::Table::new();
        table.insert(
            String::from("hwmon"),
            toml::Value::String(root.to_string_lossy().into_owned()),
        );
        toml::Value::Table(table)
    }

    #[test]
    fn renders_celsius_from_coretemp() {
        let mut module = CpuTemp::new(&fixture_options()).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("52°C")));
    }

    #[test]
    fn missing_sensor_renders_placeholder() {
        let mut table = toml::Table::new();
        table.insert(
            String::from("hwmon"),
            toml::Value::String(String::from("/nonexistent/hwmon")),
        );
        let mut module = CpuTemp::new(&toml::Value::Table(table)).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("--°C")));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let mut table = toml::Table::new();
        table.insert(String::from("hwmon"), toml::Value::Integer(3));

        assert!(CpuTemp::new(&toml::Value::Table(table)).is_err());
    }
}
