//! Volume module: default sink volume and mute, with click/scroll actions.
//!
//! ```toml
//! [modules.volume]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the volume panel, right-click toggles mute and
//! scroll up/down adjusts the volume by 5 %.

use skews_app::{Module, ModuleOutput, Msg, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ModuleId};
use skews_services::audio::{AudioStatus, status};
use std::time::{Duration, Instant};

/// How long the volume OSD stays visible after a change.
const OSD_DURATION: Duration = Duration::from_millis(1500);

type Reader = Box<dyn Fn() -> Option<AudioStatus> + Send>;

/// Shows the default sink volume.
pub struct Volume {
    id: ModuleId,
    reader: Reader,
    current: Option<AudioStatus>,
    output: ModuleOutput,
    osd_until: Option<Instant>,
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
            output: ModuleOutput::Empty,
            osd_until: None,
        }
    }

    fn refresh(&mut self) -> Effects {
        self.current = (self.reader)();

        let output = match self.current {
            Some(status) => ModuleOutput::Level {
                percent: status.volume * 100.0,
                muted: status.muted,
            },
            None => ModuleOutput::Text(String::from("--%")),
        };

        if output == self.output {
            return Vec::new();
        }
        self.output = output;
        vec![Effect::Redraw]
    }
}

impl Module for Volume {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => {
                let mut effects = Vec::new();

                if self.osd_until.is_some_and(|until| Instant::now() >= until) {
                    self.osd_until = None;
                    effects.push(Effect::Redraw);
                }

                effects.extend(self.refresh());
                effects
            }
            Msg::Interaction { module, kind } if module == &self.id => match kind {
                InteractionKind::Click => {
                    vec![Effect::Action(Action::TogglePanel(self.id.clone()))]
                }
                InteractionKind::SecondaryClick => {
                    self.osd_until = Some(Instant::now() + OSD_DURATION);
                    vec![Effect::Action(Action::ToggleMute)]
                }
                InteractionKind::ScrollUp => {
                    self.osd_until = Some(Instant::now() + OSD_DURATION);
                    vec![Effect::Action(Action::AdjustVolume(0.05))]
                }
                InteractionKind::ScrollDown => {
                    self.osd_until = Some(Instant::now() + OSD_DURATION);
                    vec![Effect::Action(Action::AdjustVolume(-0.05))]
                }
            },
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        self.output.clone()
    }

    fn panel(&self) -> Option<skews_app::PanelContent> {
        let (percent, muted) = match self.current {
            Some(status) => (status.volume * 100.0, status.muted),
            None => (0.0, false),
        };

        Some(skews_app::PanelContent::Volume { percent, muted })
    }

    /// The volume OSD: a transient popup after mute/volume changes.
    fn popup(&self) -> Option<skews_app::PanelContent> {
        self.osd_until?;
        self.panel()
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

    fn tick() -> Msg {
        Msg::Tick { unix_ms: 0 }
    }

    #[test]
    fn renders_level_and_mute() {
        let mut loud = module(0.65, false);
        loud.update(&tick());
        assert_eq!(
            loud.output(),
            ModuleOutput::Level {
                percent: 65.0,
                muted: false
            }
        );

        let mut muted = module(0.65, true);
        muted.update(&tick());
        assert_eq!(
            muted.output(),
            ModuleOutput::Level {
                percent: 65.0,
                muted: true
            }
        );
    }

    #[test]
    fn missing_sink_renders_placeholder() {
        let mut module = Volume::with_reader(Box::new(|| None));
        module.update(&tick());

        assert_eq!(module.output(), ModuleOutput::Text(String::from("--%")));
    }

    #[test]
    fn interactions_request_actions() {
        let mut module = module(0.5, false);
        let target = |kind| Msg::Interaction {
            module: ModuleId::from("volume"),
            kind,
        };

        assert_eq!(
            module.update(&target(InteractionKind::Click)),
            vec![Effect::Action(Action::TogglePanel(ModuleId::from(
                "volume"
            )))]
        );
        assert_eq!(
            module.update(&target(InteractionKind::SecondaryClick)),
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
