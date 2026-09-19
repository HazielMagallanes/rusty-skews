//! Clock module: renders the current local time.
//!
//! The format is a `chrono` strftime pattern, configurable per module:
//!
//! ```toml
//! [modules.clock]
//! format = "%H:%M"
//! ```

use chrono::{DateTime, Local};
use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Effect, Effects, ModuleId};

/// Default strftime pattern (hours and minutes).
pub const DEFAULT_FORMAT: &str = "%H:%M";

/// Renders the current local time.
pub struct Clock {
    id: ModuleId,
    format: String,
    text: String,
}

impl Clock {
    /// Builds the module from its configuration options.
    pub fn new(options: &toml::Value) -> Result<Self, String> {
        let format = options
            .get("format")
            .and_then(toml::Value::as_str)
            .unwrap_or(DEFAULT_FORMAT)
            .to_owned();

        if format.is_empty() {
            return Err(String::from("`format` must not be empty"));
        }
        if has_invalid_specifiers(&format) {
            return Err(format!(
                "`format` is not a valid strftime pattern: {format:?}"
            ));
        }

        Ok(Self {
            id: ModuleId::from("clock"),
            format,
            text: String::new(),
        })
    }

    /// Renders the configured format for a given instant (pure, testable).
    #[must_use]
    pub fn render_at(&self, instant: DateTime<Local>) -> String {
        instant.format(&self.format).to_string()
    }

    fn refresh(&mut self) -> Effects {
        self.apply(self.render_at(Local::now()))
    }

    fn apply(&mut self, text: String) -> Effects {
        if text == self.text {
            return Vec::new();
        }
        self.text = text;
        vec![Effect::Redraw]
    }
}

impl Module for Clock {
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

/// Registers the clock module.
pub fn register(registry: &mut Registry) {
    registry.register("clock", |options| {
        Clock::new(options).map(|clock| Box::new(clock) as Box<dyn Module>)
    });
}

fn has_invalid_specifiers(format: &str) -> bool {
    chrono::format::StrftimeItems::new(format)
        .any(|item| matches!(item, chrono::format::Item::Error))
}

#[cfg(test)]
mod tests {
    use super::{Clock, DEFAULT_FORMAT};
    use chrono::{Local, TimeZone};
    use skews_app::{Module, ModuleOutput, Msg};
    use skews_core::Effect;

    fn options(format: Option<&str>) -> toml::Value {
        let mut table = toml::Table::new();
        if let Some(format) = format {
            table.insert(
                String::from("format"),
                toml::Value::String(format.to_owned()),
            );
        }
        toml::Value::Table(table)
    }

    #[test]
    fn default_format_is_hours_and_minutes() {
        let clock = Clock::new(&options(None)).unwrap();
        let instant = Local.with_ymd_and_hms(2026, 9, 19, 7, 5, 0).unwrap();

        assert_eq!(clock.render_at(instant), "07:05");
        assert_eq!(DEFAULT_FORMAT, "%H:%M");
    }

    #[test]
    fn custom_format_is_honored() {
        let clock = Clock::new(&options(Some("%Y-%m-%d"))).unwrap();
        let instant = Local.with_ymd_and_hms(2026, 9, 19, 7, 5, 0).unwrap();

        assert_eq!(clock.render_at(instant), "2026-09-19");
    }

    #[test]
    fn invalid_options_are_rejected() {
        assert!(Clock::new(&options(Some(""))).is_err());
        assert!(Clock::new(&options(Some("%Q"))).is_err());
    }

    #[test]
    fn tick_redraws_only_when_the_text_changes() {
        let mut clock = Clock::new(&options(Some("%H:%M"))).unwrap();

        assert_eq!(clock.output(), ModuleOutput::Empty);
        assert_eq!(clock.apply(String::from("10:00")), vec![Effect::Redraw]);
        assert!(clock.apply(String::from("10:00")).is_empty());
        assert_eq!(clock.apply(String::from("10:01")), vec![Effect::Redraw]);
        assert_eq!(clock.output(), ModuleOutput::Text(String::from("10:01")));
    }

    #[test]
    fn tick_message_refreshes_the_clock() {
        let mut clock = Clock::new(&options(Some("%H:%M"))).unwrap();

        let effects = clock.update(&Msg::Tick { unix_ms: 0 });

        assert_eq!(effects, vec![Effect::Redraw]);
        assert!(matches!(clock.output(), ModuleOutput::Text(_)));
    }
}
