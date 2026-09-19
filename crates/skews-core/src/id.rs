//! Stable identifiers for modules and related registry keys.

use std::fmt;

/// Stable identifier of a module (bar widget, panel provider, launcher provider).
///
/// Identifiers are compared case-sensitively and are cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(String);

impl ModuleId {
    /// Creates an identifier from any string-like value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ModuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleId;

    #[test]
    fn equality_and_ordering_are_lexicographic() {
        let a = ModuleId::from("battery");
        let b = ModuleId::from("battery");
        let c = ModuleId::from("clock");

        assert_eq!(a, b);
        assert!(a < c);
    }

    #[test]
    fn display_and_accessors_round_trip() {
        let id = ModuleId::new(String::from("workspaces"));

        assert_eq!(id.as_str(), "workspaces");
        assert_eq!(id.to_string(), "workspaces");
    }
}
