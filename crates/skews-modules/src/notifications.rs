//! Notifications module: bar chip, notification center panel and popups.
//!
//! ```toml
//! [modules.notifications]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the center panel; the panel's first row toggles
//! Do Not Disturb, the remaining rows dismiss notifications. Popup rows dismiss
//! too, and popups auto-hide when there is nothing left to show.

use skews_app::{ListItem, Module, ModuleOutput, Msg, PanelContent, Registry};
use skews_core::{Action, Effect, Effects, InteractionKind, ListSource, ModuleId, Notification};

/// Shows notification state and hosts the center panel.
pub struct Notifications {
    id: ModuleId,
    active: Vec<Notification>,
    dnd: bool,
    last_text: Option<String>,
}

impl Notifications {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self {
            id: ModuleId::from("notifications"),
            active: Vec::new(),
            dnd: false,
            last_text: None,
        })
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
        if self.dnd {
            return String::from("󰂛");
        }
        match self.active.len() {
            0 => String::new(),
            count => format!("󰂚 {count}"),
        }
    }

    fn row(&self, notification: &Notification) -> ListItem {
        ListItem {
            label: notification.label().to_owned(),
            detail: notification.app.clone(),
            active: false,
        }
    }

    fn push(&mut self, notification: Notification) {
        if let Some(existing) = self
            .active
            .iter_mut()
            .find(|item| item.id == notification.id)
        {
            *existing = notification;
            return;
        }
        self.active.insert(0, notification);
    }
}

impl Module for Notifications {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Notification(notification) => {
                self.push(notification.clone());
                self.apply()
            }
            Msg::NotificationClosed(id) => {
                let before = self.active.len();
                self.active.retain(|item| item.id != *id);
                if self.active.len() == before {
                    Vec::new()
                } else {
                    self.apply()
                }
            }
            Msg::Interaction {
                module,
                kind: InteractionKind::Click,
            } if module == &self.id => {
                vec![Effect::Action(Action::TogglePanel(self.id.clone()))]
            }
            Msg::ListSelect {
                module,
                index,
                source,
            } if module == &self.id => {
                let index = match source {
                    ListSource::Panel => *index,
                    ListSource::Popup => index + 1,
                };

                if index == 0 {
                    self.dnd = !self.dnd;
                    return self.apply();
                }

                match self.active.get(index - 1) {
                    Some(notification) => {
                        vec![Effect::Action(Action::DismissNotification(notification.id))]
                    }
                    None => Vec::new(),
                }
            }
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
        let mut items = vec![ListItem {
            label: String::from("Do Not Disturb"),
            detail: if self.dnd {
                String::from("on")
            } else {
                String::from("off")
            },
            active: self.dnd,
        }];
        items.extend(
            self.active
                .iter()
                .map(|notification| self.row(notification)),
        );

        Some(PanelContent::List {
            title: String::from("Notifications"),
            items,
        })
    }

    fn popup(&self) -> Option<PanelContent> {
        if self.dnd || self.active.is_empty() {
            return None;
        }

        let items = self
            .active
            .iter()
            .map(|notification| self.row(notification))
            .collect();
        Some(PanelContent::List {
            title: String::from("Notifications"),
            items,
        })
    }
}

/// Registers the notifications module.
pub fn register(registry: &mut Registry) {
    registry.register("notifications", |options| {
        Notifications::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Notifications;
    use skews_app::{Module, ModuleOutput, Msg, PanelContent};
    use skews_core::{Action, Effect, ListSource, ModuleId, Notification};

    fn notification(id: u32, summary: &str) -> Notification {
        Notification {
            id,
            app: String::from("Discord"),
            summary: summary.to_owned(),
            body: String::from("hello"),
            timeout_ms: 8000,
        }
    }

    fn module() -> Notifications {
        Notifications::new(&toml::Value::Table(toml::Table::new())).unwrap()
    }

    #[test]
    fn bar_counts_notifications_and_shows_dnd() {
        let mut module = module();
        assert_eq!(module.output(), ModuleOutput::Empty);

        module.update(&Msg::Notification(notification(1, "One")));
        module.update(&Msg::Notification(notification(2, "Two")));
        assert_eq!(module.output(), ModuleOutput::Text(String::from("󰂚 2")));

        module.update(&Msg::ListSelect {
            module: ModuleId::from("notifications"),
            index: 0,
            source: ListSource::Panel,
        });
        assert_eq!(module.output(), ModuleOutput::Text(String::from("󰂛")));
    }

    #[test]
    fn popup_hides_under_dnd_or_when_empty() {
        let mut module = module();
        assert!(module.popup().is_none());

        module.update(&Msg::Notification(notification(1, "One")));
        assert!(
            matches!(module.popup(), Some(PanelContent::List { items, .. }) if items.len() == 1)
        );

        module.update(&Msg::ListSelect {
            module: ModuleId::from("notifications"),
            index: 0,
            source: ListSource::Panel,
        });
        assert!(module.popup().is_none());
    }

    #[test]
    fn popup_selection_offsets_to_panel_indices() {
        let mut module = module();
        module.update(&Msg::Notification(notification(7, "Seven")));

        // Popup index 0 is the first notification (panel index 1).
        assert_eq!(
            module.update(&Msg::ListSelect {
                module: ModuleId::from("notifications"),
                index: 0,
                source: ListSource::Popup,
            }),
            vec![Effect::Action(Action::DismissNotification(7))]
        );
    }

    #[test]
    fn closed_notifications_are_removed() {
        let mut module = module();
        module.update(&Msg::Notification(notification(1, "One")));
        module.update(&Msg::Notification(notification(2, "Two")));

        module.update(&Msg::NotificationClosed(1));

        assert_eq!(module.output(), ModuleOutput::Text(String::from("󰂚 1")));
    }

    #[test]
    fn replacing_a_notification_keeps_one_row() {
        let mut module = module();
        module.update(&Msg::Notification(notification(1, "One")));
        module.update(&Msg::Notification(notification(1, "Updated")));

        assert_eq!(module.output(), ModuleOutput::Text(String::from("󰂚 1")));
    }
}
