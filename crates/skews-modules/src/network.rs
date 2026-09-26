//! Network module: Wi-Fi status on the bar and a selectable network list panel.
//!
//! ```toml
//! [modules.network]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the panel; the panel's first row toggles the
//! Wi-Fi radio, the remaining rows connect to open networks.

use std::sync::Arc;

use skews_app::{ListItem, Module, ModuleOutput, Msg, PanelContent, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ModuleId};
use skews_services::network::{AccessPoint, NetworkManager, NetworkSource, NetworkStatus};

/// Shows the active Wi-Fi network.
pub struct Network {
    id: ModuleId,
    source: Arc<dyn NetworkSource>,
    status: Option<NetworkStatus>,
    points: Vec<AccessPoint>,
    last_text: Option<String>,
}

impl Network {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self::with_source(Arc::new(NetworkManager)))
    }

    /// Builds the module with an injected source (tests).
    #[must_use]
    pub fn with_source(source: Arc<dyn NetworkSource>) -> Self {
        Self {
            id: ModuleId::from("network"),
            source,
            status: None,
            points: Vec::new(),
            last_text: None,
        }
    }

    fn refresh_status(&mut self) -> Effects {
        self.status = self.source.status().ok();
        self.apply()
    }

    fn refresh_panel(&mut self) -> Effects {
        self.status = self.source.status().ok();
        self.points = self.source.access_points().unwrap_or_default();
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
        match &self.status {
            None => String::from("--"),
            Some(status) if !status.wifi_enabled => String::from("wifi off"),
            Some(status) => status
                .ssid
                .clone()
                .unwrap_or_else(|| String::from("offline")),
        }
    }

    fn items(&self) -> Vec<ListItem> {
        let enabled = self
            .status
            .as_ref()
            .is_some_and(|status| status.wifi_enabled);
        let mut items = vec![ListItem {
            label: String::from("Wi-Fi"),
            detail: if enabled {
                String::from("on")
            } else {
                String::from("off")
            },
            active: enabled,
        }];

        for point in &self.points {
            items.push(ListItem {
                label: point.ssid.clone(),
                detail: format!(
                    "{}%{}",
                    point.signal,
                    if point.security { "  secured" } else { "" }
                ),
                active: point.active,
            });
        }

        items
    }
}

impl Module for Network {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => self.refresh_status(),
            Msg::PanelOpened { module } if module == &self.id => self.refresh_panel(),
            Msg::Interaction {
                module,
                kind: InteractionKind::Click,
            } if module == &self.id => {
                vec![Effect::Action(Action::TogglePanel(self.id.clone()))]
            }
            Msg::ListSelect { module, index } if module == &self.id => {
                if *index == 0 {
                    let enabled = self
                        .status
                        .as_ref()
                        .is_some_and(|status| status.wifi_enabled);
                    return vec![Effect::Action(Action::SetWifi(!enabled))];
                }

                match self.points.get(index - 1) {
                    Some(point) => vec![Effect::Action(Action::ConnectWifi(point.ssid.clone()))],
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        match &self.last_text {
            Some(text) => ModuleOutput::Text(text.clone()),
            None => ModuleOutput::Empty,
        }
    }

    fn panel(&self) -> Option<PanelContent> {
        Some(PanelContent::List {
            title: String::from("Wi-Fi"),
            items: self.items(),
        })
    }
}

/// Registers the network module.
pub fn register(registry: &mut Registry) {
    registry.register("network", |options| {
        Network::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Network;
    use skews_app::{Module, ModuleOutput, Msg, PanelContent};
    use skews_core::{Action, Effect, InteractionKind, ModuleId};
    use skews_services::network::{
        AccessPoint, Connectivity, NetworkError, NetworkSource, NetworkStatus,
    };
    use std::sync::Arc;
    use std::sync::Mutex;

    struct Fake {
        status: Mutex<NetworkStatus>,
        points: Mutex<Vec<AccessPoint>>,
    }

    impl NetworkSource for Fake {
        fn status(&self) -> Result<NetworkStatus, NetworkError> {
            Ok(self.status.lock().unwrap().clone())
        }

        fn access_points(&self) -> Result<Vec<AccessPoint>, NetworkError> {
            Ok(self.points.lock().unwrap().clone())
        }

        fn set_wifi_enabled(&self, _enabled: bool) -> Result<(), NetworkError> {
            Ok(())
        }

        fn connect(&self, _ssid: &str) -> Result<(), NetworkError> {
            Ok(())
        }
    }

    fn fake(enabled: bool, ssid: Option<&str>) -> Arc<Fake> {
        Arc::new(Fake {
            status: Mutex::new(NetworkStatus {
                wifi_enabled: enabled,
                ssid: ssid.map(str::to_owned),
                signal: Some(70),
                connectivity: Connectivity::Full,
            }),
            points: Mutex::new(vec![
                AccessPoint {
                    ssid: String::from("Open Net"),
                    signal: 80,
                    security: false,
                    active: false,
                },
                AccessPoint {
                    ssid: String::from("AURA"),
                    signal: 70,
                    security: true,
                    active: true,
                },
            ]),
        })
    }

    fn module(enabled: bool, ssid: Option<&str>) -> Network {
        Network::with_source(fake(enabled, ssid))
    }

    #[test]
    fn bar_shows_ssid_offline_and_off() {
        let mut connected = module(true, Some("AURA"));
        connected.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(connected.output(), ModuleOutput::Text(String::from("AURA")));

        let mut offline = module(true, None);
        offline.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(
            offline.output(),
            ModuleOutput::Text(String::from("offline"))
        );

        let mut disabled = module(false, None);
        disabled.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(
            disabled.output(),
            ModuleOutput::Text(String::from("wifi off"))
        );
    }

    #[test]
    fn panel_lists_toggle_and_access_points() {
        let mut module = module(true, Some("AURA"));
        module.update(&Msg::PanelOpened {
            module: ModuleId::from("network"),
        });

        let Some(PanelContent::List { title, items }) = module.panel() else {
            panic!("expected a list panel");
        };

        assert_eq!(title, "Wi-Fi");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].detail, "on");
        assert_eq!(items[1].label, "Open Net");
        assert!(items[2].active);
        assert!(items[2].detail.contains("secured"));
    }

    #[test]
    fn row_selection_toggles_and_connects() {
        let mut module = module(true, Some("AURA"));
        module.update(&Msg::PanelOpened {
            module: ModuleId::from("network"),
        });

        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("network"),
                index: 0
            }),
            vec![Effect::Action(Action::SetWifi(false))]
        );
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("network"),
                index: 1
            }),
            vec![Effect::Action(Action::ConnectWifi(String::from(
                "Open Net"
            )))]
        );
        assert!(
            module
                .update(&Msg::ListSelect {
                    module: ModuleId::from("network"),
                    index: 9
                })
                .is_empty()
        );
    }

    #[test]
    fn click_toggles_the_panel() {
        let mut module = module(true, Some("AURA"));

        assert_eq!(
            module.update(&Msg::Interaction {
                module: ModuleId::from("network"),
                kind: InteractionKind::Click,
            }),
            vec![Effect::Action(Action::TogglePanel(ModuleId::from(
                "network"
            )))]
        );
    }
}
