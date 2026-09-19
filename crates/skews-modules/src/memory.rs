//! Memory usage module.
//!
//! ```toml
//! [modules.memory]
//! # optional: meminfo path for tests
//! # meminfo = "/proc/meminfo"
//! ```

use std::path::PathBuf;

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effect, Effects, ModuleId};
use skews_sys_linux::SysPaths;

/// Shows the used memory percentage.
pub struct Memory {
    id: ModuleId,
    paths: SysPaths,
    text: String,
}

impl Memory {
    /// Builds the module from its configuration options.
    pub fn new(options: &toml::Value) -> Result<Self, String> {
        let mut paths = SysPaths::default();

        if let Some(meminfo) = options.get("meminfo") {
            let Some(path) = meminfo.as_str() else {
                return Err(String::from("`meminfo` must be a string path"));
            };
            paths.meminfo = PathBuf::from(path);
        }

        Ok(Self {
            id: ModuleId::from("memory"),
            paths,
            text: String::new(),
        })
    }

    fn refresh(&mut self) -> Effects {
        let text = match skews_sys_linux::memory_used_percent(&self.paths.meminfo) {
            Some(percent) => format!("{}%", percent.round() as i64),
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

impl Module for Memory {
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

/// Registers the memory module.
pub fn register(registry: &mut Registry) {
    registry.register("memory", |options| {
        Memory::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Memory;
    use skews_app::{Module, ModuleOutput};
    use std::path::Path;

    fn fixture_options() -> toml::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meminfo");
        let mut table = toml::Table::new();
        table.insert(
            String::from("meminfo"),
            toml::Value::String(path.to_string_lossy().into_owned()),
        );
        toml::Value::Table(table)
    }

    #[test]
    fn renders_used_percentage() {
        let mut module = Memory::new(&fixture_options()).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("75%")));
    }

    #[test]
    fn missing_file_renders_placeholder() {
        let mut table = toml::Table::new();
        table.insert(
            String::from("meminfo"),
            toml::Value::String(String::from("/nonexistent/meminfo")),
        );
        let mut module = Memory::new(&toml::Value::Table(table)).unwrap();

        module.update(&skews_app::Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("--%")));
    }
}
