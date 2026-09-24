use serde::Serialize;

/// What the focus guard observed while it watched the frontmost application
/// during a background pointer delivery.
///
/// `interventions` counts attempts to hand the frontmost position back to the
/// application that was frontmost before delivery, `restored` is true only
/// when a steal happened and that application was frontmost again when the
/// guard stopped, and `max_steal_ms` is the longest observed stretch during
/// which another application was frontmost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BackgroundFocusGuard {
    pub interventions: u32,
    pub restored: bool,
    pub max_steal_ms: u64,
}
