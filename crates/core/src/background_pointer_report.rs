use crate::ProcessId;

/// Frontmost-application evidence an adapter samples around a background
/// pointer delivery.
///
/// Background delivery promises not to activate the target, but the target
/// process decides for itself how to react to a posted click, so the command
/// reports what it observed instead of claiming a silent success. `None`
/// means the frontmost application could not be read at that moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackgroundPointerReport {
    pub frontmost_pid_before: Option<ProcessId>,
    pub frontmost_pid_after: Option<ProcessId>,
}

impl BackgroundPointerReport {
    /// `"unchanged"`, `"changed"`, or `"unknown"` when either sample is missing.
    pub fn focus_change(&self) -> &'static str {
        match (self.frontmost_pid_before, self.frontmost_pid_after) {
            (Some(before), Some(after)) if before == after => "unchanged",
            (Some(_), Some(_)) => "changed",
            _ => "unknown",
        }
    }
}
