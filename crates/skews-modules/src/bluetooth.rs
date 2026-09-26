//! Bluetooth module: adapter status on the bar and a device list panel.
//!
//! ```toml
//! [modules.bluetooth]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the panel; the first row toggles the adapter,
//! the remaining rows connect or disconnect devices.

use std::sync::Arc;

use skews_app::{ListItem, Module, ModuleOutput, Msg, PanelContent, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ModuleId};
use skews_services::bluetooth::{AdapterStatus, BluetoothDevice, BluetoothSource, Bluez};

/// Shows Bluetooth status.
pub struct Bluetooth {
    id: ModuleId,
    source: Arc<dyn BluetoothSource>,
    status: Option<AdapterStatus>,
    devices: Vec<BluetoothDevice>,
    last_text: Option<String>,
}

impl Bluetooth {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self::with_source(Arc::new(Bluez)))
    }

    /// Builds the module with an injected source (tests).
    #[must_use]
    pub fn with_source(source: Arc<dyn BluetoothSource>) -> Self {
        Self {
            id: ModuleId::from("bluetooth"),
            source,
            status: None,
            devices: Vec::new(),
            last_text: None,
        }
    }

    fn refresh_status(&mut self) -> Effects {
        self.status = self.source.status().ok();
        self.apply()
    }

    fn refresh_panel(&mut self) -> Effects {
        self.status = self.source.status().ok();
        self.devices = self.source.devices().unwrap_or_default();
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
            Some(status) if !status.powered => String::from("bt off"),
            Some(_) => self
                .devices
                .iter()
                .find(|device| device.connected)
                .map_or_else(|| String::from("bt on"), |device| device.name.clone()),
        }
    }

    fn items(&self) -> Vec<ListItem> {
        let powered = self.status.as_ref().is_some_and(|status| status.powered);
        let mut items = vec![ListItem {
            label: String::from("Bluetooth"),
            detail: if powered {
                String::from("on")
            } else {
                String::from("off")
            },
            active: powered,
        }];

        for device in &self.devices {
            items.push(ListItem {
                label: device.name.clone(),
                detail: if device.connected {
                    String::from("connected")
                } else if device.paired {
                    String::from("paired")
                } else {
                    String::from("known")
                },
                active: device.connected,
            });
        }

        items
    }
}

impl Module for Bluetooth {
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
                    let powered = self.status.as_ref().is_some_and(|status| status.powered);
                    return vec![Effect::Action(Action::SetBluetooth(!powered))];
                }

                match self.devices.get(index - 1) {
                    Some(device) if device.connected => {
                        vec![Effect::Action(Action::DisconnectBluetooth(
                            device.address.clone(),
                        ))]
                    }
                    Some(device) => {
                        vec![Effect::Action(Action::ConnectBluetooth(
                            device.address.clone(),
                        ))]
                    }
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
            title: String::from("Bluetooth"),
            items: self.items(),
        })
    }
}

/// Registers the Bluetooth module.
pub fn register(registry: &mut Registry) {
    registry.register("bluetooth", |options| {
        Bluetooth::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Bluetooth;
    use skews_app::{Module, ModuleOutput, Msg, PanelContent};
    use skews_core::{Action, Effect, InteractionKind, ModuleId};
    use skews_services::bluetooth::{
        AdapterStatus, BluetoothDevice, BluetoothError, BluetoothSource,
    };
    use std::sync::{Arc, Mutex};

    struct Fake {
        status: Mutex<AdapterStatus>,
        devices: Mutex<Vec<BluetoothDevice>>,
    }

    impl BluetoothSource for Fake {
        fn status(&self) -> Result<AdapterStatus, BluetoothError> {
            Ok(self.status.lock().unwrap().clone())
        }

        fn devices(&self) -> Result<Vec<BluetoothDevice>, BluetoothError> {
            Ok(self.devices.lock().unwrap().clone())
        }

        fn set_powered(&self, _powered: bool) -> Result<(), BluetoothError> {
            Ok(())
        }

        fn connect(&self, _address: &str) -> Result<(), BluetoothError> {
            Ok(())
        }

        fn disconnect(&self, _address: &str) -> Result<(), BluetoothError> {
            Ok(())
        }
    }

    fn module(powered: bool, connected: bool) -> Bluetooth {
        Bluetooth::with_source(Arc::new(Fake {
            status: Mutex::new(AdapterStatus {
                powered,
                name: String::from("host"),
            }),
            devices: Mutex::new(vec![
                BluetoothDevice {
                    address: String::from("AA:BB"),
                    name: String::from("Headset"),
                    connected,
                    paired: true,
                },
                BluetoothDevice {
                    address: String::from("CC:DD"),
                    name: String::from("Mouse"),
                    connected: false,
                    paired: true,
                },
            ]),
        }))
    }

    #[test]
    fn bar_shows_state_and_connected_device() {
        let mut off = module(false, false);
        off.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(off.output(), ModuleOutput::Text(String::from("bt off")));

        let mut idle = module(true, false);
        idle.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(idle.output(), ModuleOutput::Text(String::from("bt on")));

        let mut connected = module(true, true);
        connected.update(&Msg::PanelOpened {
            module: ModuleId::from("bluetooth"),
        });
        assert_eq!(
            connected.output(),
            ModuleOutput::Text(String::from("Headset"))
        );
    }

    #[test]
    fn panel_lists_adapter_and_devices() {
        let mut module = module(true, true);
        module.update(&Msg::PanelOpened {
            module: ModuleId::from("bluetooth"),
        });

        let Some(PanelContent::List { title, items }) = module.panel() else {
            panic!("expected a list panel");
        };

        assert_eq!(title, "Bluetooth");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].detail, "on");
        assert_eq!(items[1].detail, "connected");
        assert!(items[1].active);
        assert_eq!(items[2].detail, "paired");
    }

    #[test]
    fn row_selection_toggles_and_connects() {
        let mut module = module(true, true);
        module.update(&Msg::PanelOpened {
            module: ModuleId::from("bluetooth"),
        });

        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("bluetooth"),
                index: 0
            }),
            vec![Effect::Action(Action::SetBluetooth(false))]
        );
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("bluetooth"),
                index: 1
            }),
            vec![Effect::Action(Action::DisconnectBluetooth(String::from(
                "AA:BB"
            )))]
        );
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("bluetooth"),
                index: 2
            }),
            vec![Effect::Action(Action::ConnectBluetooth(String::from(
                "CC:DD"
            )))]
        );
    }

    #[test]
    fn click_toggles_the_panel() {
        let mut module = module(true, false);

        assert_eq!(
            module.update(&Msg::Interaction {
                module: ModuleId::from("bluetooth"),
                kind: InteractionKind::Click,
            }),
            vec![Effect::Action(Action::TogglePanel(ModuleId::from(
                "bluetooth"
            )))]
        );
    }
}
