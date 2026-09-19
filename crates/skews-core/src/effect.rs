//! Effects produced by the kernel's `update` step.
//!
//! The kernel follows the *effects-as-data* model: state transitions are pure
//! and return a list of side-effect descriptions that the runtime executes in
//! order. This keeps behavior unit-testable without a display or event loop.

/// Severity of a [`Effect::Log`] message.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// Fine-grained diagnostics.
    Debug,
    /// Normal lifecycle information.
    Info,
    /// Something that deserves attention but is recoverable.
    Warn,
    /// Something that failed or will degrade behavior.
    Error,
}

/// A side effect requested by the kernel.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Request a redraw of every mapped surface.
    Redraw,
    /// Emit a log line through the shell's tracing pipeline.
    Log {
        /// Severity of the message.
        level: LogLevel,
        /// Human-readable message.
        message: String,
    },
}

/// An ordered list of effects to execute after an update.
pub type Effects = Vec<Effect>;

#[cfg(test)]
mod tests {
    use super::{Effect, Effects, LogLevel};

    #[test]
    fn effects_are_comparable_and_ordered() {
        let effects: Effects = vec![
            Effect::Log {
                level: LogLevel::Info,
                message: String::from("hello"),
            },
            Effect::Redraw,
        ];

        assert_eq!(
            effects[0],
            Effect::Log {
                level: LogLevel::Info,
                message: String::from("hello")
            }
        );
        assert_eq!(effects[1], Effect::Redraw);
    }
}
