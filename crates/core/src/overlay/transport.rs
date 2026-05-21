#[cfg(unix)]
/// Overlay socket connection type for Unix targets.
pub type Conn = std::os::unix::net::UnixStream;

#[cfg(not(unix))]
/// Uninhabited transport marker for targets without Unix sockets.
pub enum Never {}

#[cfg(not(unix))]
/// Overlay socket connection type for non-Unix targets.
pub type Conn = Never;

/// Reports whether this target can connect to the overlay transport.
pub const fn is_enabled_at_compile_time() -> bool {
    cfg!(unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(unix))]
    #[test]
    fn non_unix_is_disabled() {
        assert!(!is_enabled_at_compile_time());
        assert!(!crate::overlay::is_enabled());
    }

    #[cfg(unix)]
    #[test]
    fn is_enabled_at_compile_time_is_true_on_unix() {
        assert!(is_enabled_at_compile_time());
    }
}
