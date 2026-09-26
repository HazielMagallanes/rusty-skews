//! `org.freedesktop.Notifications` server over D-Bus.
//!
//! Implements the freedesktop notification protocol as a blocking zbus server.
//! The server owns the well-known bus name while the shell runs; when another
//! daemon (for example quickshell) holds it, [`spawn`] fails cleanly and the
//! shell keeps running without notifications.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use skews_core::Notification;
use thiserror::Error;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::OwnedValue;

/// Events produced by the notification server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationEvent {
    /// A client posted a notification.
    Posted(Notification),
    /// A client closed a notification.
    Closed(u32),
}

/// Errors produced while starting or driving the server.
#[derive(Debug, Error)]
pub enum NotificationError {
    /// The D-Bus call failed (including the name being taken).
    #[error("notification server failed: {0}")]
    Dbus(#[from] zbus::Error),
}

/// Keeps the notification server alive for the shell's lifetime.
pub struct NotificationServer {
    connection: Connection,
}

const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";

/// Starts the notification server, forwarding events to `on_event`.
pub fn spawn<F>(on_event: F) -> Result<NotificationServer, NotificationError>
where
    F: Fn(NotificationEvent) + Send + Sync + 'static,
{
    let iface = NotificationsInterface {
        next_id: AtomicU32::new(1),
        on_event: Arc::new(on_event),
    };

    let connection = Builder::session()?
        .name(INTERFACE)?
        .serve_at(PATH, iface)?
        .build()?;

    Ok(NotificationServer { connection })
}

impl NotificationServer {
    /// Emits `NotificationClosed` for a notification dismissed by the shell.
    ///
    /// Reason `2` is `NotificationClosedReason::Dismissed`.
    pub fn close(&self, id: u32) -> Result<(), NotificationError> {
        self.connection.emit_signal(
            None::<&str>,
            PATH,
            INTERFACE,
            "NotificationClosed",
            &(id, 2_u32),
        )?;
        Ok(())
    }
}

struct NotificationsInterface {
    next_id: AtomicU32,
    on_event: Arc<dyn Fn(NotificationEvent) + Send + Sync>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationsInterface {
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let _ = (app_icon, actions, hints);

        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };

        (self.on_event)(NotificationEvent::Posted(Notification {
            id,
            app: app_name,
            summary,
            body,
            timeout_ms: expire_timeout,
        }));

        id
    }

    fn close_notification(&self, id: u32) {
        (self.on_event)(NotificationEvent::Closed(id));
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec![String::from("body"), String::from("body-markup")]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            String::from("rusty-skews"),
            String::from("rusty-skews"),
            String::from(env!("CARGO_PKG_VERSION")),
            String::from("1.2"),
        )
    }
}

#[cfg(test)]
mod tests {
    use skews_core::Notification;

    #[test]
    fn label_falls_back_to_app_name() {
        let mut notification = Notification {
            id: 1,
            app: String::from("Discord"),
            summary: String::new(),
            body: String::new(),
            timeout_ms: -1,
        };
        assert_eq!(notification.label(), "Discord");

        notification.summary = String::from("New message");
        assert_eq!(notification.label(), "New message");
    }
}
