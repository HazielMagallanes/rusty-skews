//! Media module: MPRIS playback on the bar with transport controls.
//!
//! ```toml
//! [modules.media]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the controls panel, right-click toggles
//! play/pause and scroll up/down skips tracks.

use std::sync::Arc;

use skews_app::{ListItem, Module, ModuleOutput, Msg, PanelContent, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ModuleId};
use skews_services::media::{MediaSource, Mpris, PlaybackStatus, PlayerStatus};

/// Maximum number of characters shown on the bar.
const MAX_BAR_CHARS: usize = 32;

/// Shows the active media player.
pub struct Media {
    id: ModuleId,
    source: Arc<dyn MediaSource>,
    status: Option<PlayerStatus>,
    last_text: Option<String>,
}

impl Media {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self::with_source(Arc::new(Mpris)))
    }

    /// Builds the module with an injected source (tests).
    #[must_use]
    pub fn with_source(source: Arc<dyn MediaSource>) -> Self {
        Self {
            id: ModuleId::from("media"),
            source,
            status: None,
            last_text: None,
        }
    }

    fn refresh(&mut self) -> Effects {
        self.status = self.source.status().ok().flatten();
        self.apply()
    }

    fn apply(&mut self) -> Effects {
        let text = self.bar_text();
        if self.last_text.as_deref() == Some(text.as_str()) {
            return Vec::new();
        }
        self.last_text = Some(text);
        vec![Effect::Redraw]
    }

    fn bar_text(&self) -> String {
        let Some(status) = &self.status else {
            return String::new();
        };

        let icon = match status.status {
            PlaybackStatus::Playing => "▶",
            PlaybackStatus::Paused => "‖",
            PlaybackStatus::Stopped => return String::new(),
        };

        let label = if status.title.is_empty() {
            status.player.clone()
        } else if status.artist.is_empty() {
            status.title.clone()
        } else {
            format!("{} — {}", status.title, status.artist)
        };

        format!("{icon} {}", truncate(&label, MAX_BAR_CHARS))
    }

    fn control(index: usize) -> Option<Action> {
        match index {
            0 => Some(Action::MediaPlayPause),
            1 => Some(Action::MediaNext),
            2 => Some(Action::MediaPrevious),
            _ => None,
        }
    }
}

/// Truncates to `max` characters, appending an ellipsis when cut.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }

    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

impl Module for Media {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => self.refresh(),
            Msg::Interaction { module, kind } if module == &self.id => match kind {
                InteractionKind::Click => {
                    vec![Effect::Action(Action::TogglePanel(self.id.clone()))]
                }
                InteractionKind::SecondaryClick => vec![Effect::Action(Action::MediaPlayPause)],
                InteractionKind::ScrollUp => vec![Effect::Action(Action::MediaNext)],
                InteractionKind::ScrollDown => vec![Effect::Action(Action::MediaPrevious)],
            },
            Msg::ListSelect {
                module,
                index,
                source: skews_core::ListSource::Panel,
            } if module == &self.id => match Self::control(*index) {
                Some(action) => vec![Effect::Action(action)],
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        match &self.last_text {
            Some(text) if !text.is_empty() => ModuleOutput::Text(text.clone()),
            _ => ModuleOutput::Empty,
        }
    }

    fn panel(&self) -> Option<PanelContent> {
        let (title, state) = match &self.status {
            Some(status) => (
                status.player.clone(),
                match status.status {
                    PlaybackStatus::Playing => String::from("Playing"),
                    PlaybackStatus::Paused => String::from("Paused"),
                    PlaybackStatus::Stopped => String::from("Stopped"),
                },
            ),
            None => (String::from("Media"), String::from("No player")),
        };

        Some(PanelContent::List {
            title,
            items: vec![
                ListItem {
                    label: String::from("Play/Pause"),
                    detail: state,
                    active: false,
                },
                ListItem {
                    label: String::from("Next"),
                    detail: String::new(),
                    active: false,
                },
                ListItem {
                    label: String::from("Previous"),
                    detail: String::new(),
                    active: false,
                },
            ],
        })
    }
}

/// Registers the media module.
pub fn register(registry: &mut Registry) {
    registry.register("media", |options| {
        Media::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Media;
    use skews_app::{Module, ModuleOutput, Msg};
    use skews_core::{Action, Effect, InteractionKind, ModuleId};
    use skews_services::media::{MediaError, MediaSource, PlaybackStatus, PlayerStatus};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fake {
        calls: Mutex<Vec<String>>,
        status: Mutex<Option<PlayerStatus>>,
    }

    impl Fake {
        fn with(status: Option<PlayerStatus>) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                status: Mutex::new(status),
            })
        }

        fn record(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_owned());
        }
    }

    impl MediaSource for Fake {
        fn status(&self) -> Result<Option<PlayerStatus>, MediaError> {
            Ok(self.status.lock().unwrap().clone())
        }

        fn play_pause(&self) -> Result<(), MediaError> {
            self.record("play_pause");
            Ok(())
        }

        fn next(&self) -> Result<(), MediaError> {
            self.record("next");
            Ok(())
        }

        fn previous(&self) -> Result<(), MediaError> {
            self.record("previous");
            Ok(())
        }
    }

    fn player(status: PlaybackStatus, title: &str, artist: &str) -> PlayerStatus {
        PlayerStatus {
            player: String::from("spotify"),
            status,
            title: title.to_owned(),
            artist: artist.to_owned(),
        }
    }

    fn module(fake: &Arc<Fake>) -> Media {
        Media::with_source(fake.clone())
    }

    fn tick() -> Msg {
        Msg::Tick { unix_ms: 0 }
    }

    #[test]
    fn playing_track_shows_on_the_bar() {
        let fake = Fake::with(Some(player(PlaybackStatus::Playing, "Song", "Artist")));
        let mut module = module(&fake);
        module.update(&tick());

        assert_eq!(
            module.output(),
            ModuleOutput::Text(String::from("▶ Song — Artist"))
        );
    }

    #[test]
    fn paused_and_stopped_states() {
        let paused = Fake::with(Some(player(PlaybackStatus::Paused, "Song", "Artist")));
        let mut paused_module = module(&paused);
        paused_module.update(&tick());
        assert_eq!(
            paused_module.output(),
            ModuleOutput::Text(String::from("‖ Song — Artist"))
        );

        let stopped = Fake::with(Some(player(PlaybackStatus::Stopped, "Song", "Artist")));
        let mut stopped_module = module(&stopped);
        stopped_module.update(&tick());
        assert_eq!(stopped_module.output(), ModuleOutput::Empty);

        let none = Fake::with(None);
        let mut none_module = module(&none);
        none_module.update(&tick());
        assert_eq!(none_module.output(), ModuleOutput::Empty);
    }

    #[test]
    fn long_titles_are_truncated() {
        let long = "a".repeat(64);
        let fake = Fake::with(Some(player(PlaybackStatus::Playing, &long, "")));
        let mut module = module(&fake);
        module.update(&tick());

        let ModuleOutput::Text(text) = module.output() else {
            panic!("expected text output");
        };
        assert_eq!(text.chars().count(), 34);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn interactions_map_to_transport_actions() {
        let fake = Fake::with(Some(player(PlaybackStatus::Playing, "Song", "Artist")));
        let mut module = module(&fake);

        assert_eq!(
            module.update(&Msg::Interaction {
                module: ModuleId::from("media"),
                kind: InteractionKind::SecondaryClick,
            }),
            vec![Effect::Action(Action::MediaPlayPause)]
        );
        assert_eq!(
            module.update(&Msg::Interaction {
                module: ModuleId::from("media"),
                kind: InteractionKind::ScrollUp,
            }),
            vec![Effect::Action(Action::MediaNext)]
        );
        assert_eq!(
            module.update(&Msg::Interaction {
                module: ModuleId::from("media"),
                kind: InteractionKind::ScrollDown,
            }),
            vec![Effect::Action(Action::MediaPrevious)]
        );
    }

    #[test]
    fn panel_rows_map_to_transport_actions() {
        let fake = Fake::with(None);
        let mut module = module(&fake);

        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("media"),
                index: 0,
                source: skews_core::ListSource::Panel,
            }),
            vec![Effect::Action(Action::MediaPlayPause)]
        );
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("media"),
                index: 1,
                source: skews_core::ListSource::Panel,
            }),
            vec![Effect::Action(Action::MediaNext)]
        );
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("media"),
                index: 2,
                source: skews_core::ListSource::Panel,
            }),
            vec![Effect::Action(Action::MediaPrevious)]
        );
        assert!(
            module
                .update(&Msg::ListSelect {
                    module: ModuleId::from("media"),
                    index: 9,
                    source: skews_core::ListSource::Panel,
                })
                .is_empty()
        );
    }
}
