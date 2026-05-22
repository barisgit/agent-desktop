pub mod client;
/// Serializes lifecycle tests because std::env mutation is process-global.
#[cfg(all(test, unix))]
pub(crate) mod lifecycle;
pub mod protocol;
pub mod suppress;
pub mod transport;

pub use client::{
    is_enabled, notify_error, notify_key_combo, notify_key_text, notify_mouse, notify_scroll,
    notify_synthetic_click, set_color, set_visible, target_clear, target_set, thinking_set,
};
pub use suppress::{SuppressClearGuard, clear_suppress};
