//! Notification payloads shared by the service, kernel and modules.

/// A notification delivered by the system notification bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Bus-assigned id (used to close it later).
    pub id: u32,
    /// Application name.
    pub app: String,
    /// Summary line.
    pub summary: String,
    /// Body text (markup is rendered as plain text for now).
    pub body: String,
    /// Requested timeout in milliseconds (`-1` = server default, `0` = never).
    pub timeout_ms: i32,
}

impl Notification {
    /// Best-effort display label: the summary, or the app name when empty.
    #[must_use]
    pub fn label(&self) -> &str {
        if self.summary.trim().is_empty() {
            &self.app
        } else {
            &self.summary
        }
    }
}
