//! Volume module: default sink volume and mute, with click/scroll actions.
//!
//! ```toml
//! [modules.volume]
//! # no options yet
//! ```
//!
//! Interactions: click toggles mute, scroll up/down adjusts the volume by 5 %.

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ModuleId};
use skews_services::audio::{AudioStatus, status};

type Reader = Box<dyn Fn() -> Option<AudioStatus> + Send>;

/// Shows the default sink volume.
pub struct Volume {
    id: ModuleId,
    reader: Reader,
    current: Option<AudioStatus>,
    text: String,
}

impl Volume {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self::with_reader(Box::new(|| status().ok())))
    }

    /// Builds the module with an injected reader (tests).
    #[must_use]
    pub fn with_reader(reader: Reader) -> Self {
        Self {
            id: ModuleId::from("volume"),
            reader,
            current: None,
            text: String::new(),
        }
    }

    fn refresh(&mut self) -> Effects {
        let current = (self.reader)();
        self.current = current;

        let text = match current {
            Some(status) if status.muted => String::from("MUTED"),
            Some(status) => format!("{}%", (status.volume * 100.0).round() as i32),
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

impl Module for Volume {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => self.refresh(),
            Msg::Interaction { module, kind } if module == &self.id => match kind {
                InteractionKind::Click => vec![Effect::Action(Action::ToggleMute)],
                InteractionKind::ScrollUp => {
                    vec![Effect::Action(Action::AdjustVolume(0.05))]
                }
                InteractionKind::ScrollDown => {
                    vec![Effect::Action(Action::AdjustVolume(-0.05))]
                }
            },
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

/// Registers the volume module.
pub fn register(registry: &mut Registry) {
    registry.register("volume", |options| {
        Volume::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Volume;
    use skews_app::{Module, ModuleOutput, Msg};
    use skews_core::{Action, Effect, InteractionKind, ModuleId};
    use skews_services::audio::AudioStatus;

    fn module(volume: f32, muted: bool) -> Volume {
        Volume::with_reader(Box::new(move || Some(AudioStatus { volume, muted })))
    }

    #[test]
    fn renders_percentage_and_mute() {
        let mut loud = module(0.65, false);
        loud.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(loud.output(), ModuleOutput::Text(String::from("65%")));

        let mut muted = module(0.65, true);
        muted.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(muted.output(), ModuleOutput::Text(String::from("MUTED")));
    }

    #[test]
    fn missing_sink_renders_placeholder() {
        let mut module = Volume::with_reader(Box::new(|| None));
        module.update(&Msg::Tick { unix_ms: 0 });

        assert_eq!(module.output(), ModuleOutput::Text(String::from("--%")));
    }

    #[test]
    fn interactions_request_audio_actions() {
        let mut module = module(0.5, false);
        let target = |kind| Msg::Interaction {
            module: ModuleId::from("volume"),
            kind,
        };

        assert_eq!(
            module.update(&target(InteractionKind::Click)),
            vec![Effect::Action(Action::ToggleMute)]
        );
        assert_eq!(
            module.update(&target(InteractionKind::ScrollUp)),
            vec![Effect::Action(Action::AdjustVolume(0.05))]
        );
        assert_eq!(
            module.update(&target(InteractionKind::ScrollDown)),
            vec![Effect::Action(Action::AdjustVolume(-0.05))]
        );
    }

    #[test]
    fn ignores_interactions_for_other_modules() {
        let mut module = module(0.5, false);

        let effects = module.update(&Msg::Interaction {
            module: ModuleId::from("clock"),
            kind: InteractionKind::Click,
        });

        assert!(effects.is_empty());
    }
}
