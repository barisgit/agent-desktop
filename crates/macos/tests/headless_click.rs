//! Integration tests for headless mouse routing via CGEventPostToPid.
//!
//! Tests that actually move the cursor or click on screen are gated
//! behind the `AGENT_DESKTOP_RUN_GUI_TESTS=1` environment variable
//! because they require an interactive macOS session, accessibility
//! permission for the test binary, and a writable UI surface. The
//! unconditional tests use a self-pid target to exercise the routing
//! path without producing visible side effects (`CGEventPostToPid`
//! targeting the test process is silently dropped by the WindowServer
//! since this process has no front-end UI to receive it).

#![cfg(target_os = "macos")]

use agent_desktop_core::InteractionPolicy;
use agent_desktop_core::action::{MouseButton, MouseEvent, MouseEventKind, Point};
use agent_desktop_core::adapter::PlatformAdapter;
use agent_desktop_macos::MacOSAdapter;

fn self_pid() -> i32 {
    std::process::id() as i32
}

fn gui_tests_enabled() -> bool {
    std::env::var("AGENT_DESKTOP_RUN_GUI_TESTS").as_deref() == Ok("1")
}

#[test]
fn mouse_event_with_self_pid_does_not_panic() {
    let adapter = MacOSAdapter::new();
    let result = adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: Point { x: 0.0, y: 0.0 },
            button: MouseButton::Left,
        },
        Some(self_pid()),
    );
    assert!(
        result.is_ok(),
        "headless mouse_event to self pid should succeed: {result:?}"
    );
}

#[test]
fn drag_with_self_pid_does_not_panic() {
    let adapter = MacOSAdapter::new();
    let result = adapter.drag(
        agent_desktop_core::action::DragParams {
            from: Point { x: 0.0, y: 0.0 },
            to: Point { x: 1.0, y: 1.0 },
            duration_ms: Some(0),
            drop_delay_ms: None,
        },
        Some(self_pid()),
    );
    assert!(
        result.is_ok(),
        "headless drag to self pid should succeed: {result:?}"
    );
}

#[test]
fn mouse_event_with_none_target_pid_still_works() {
    if !gui_tests_enabled() {
        eprintln!("skipping: set AGENT_DESKTOP_RUN_GUI_TESTS=1 to enable visible cursor moves");
        return;
    }
    let adapter = MacOSAdapter::new();
    let result = adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: Point { x: 10.0, y: 10.0 },
            button: MouseButton::Left,
        },
        None,
    );
    assert!(
        result.is_ok(),
        "physical mouse_event broadcast should succeed: {result:?}"
    );
}

#[test]
fn headless_click_lookup_resolves_running_app() {
    let adapter = MacOSAdapter::new();
    let apps = match adapter.list_apps() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "skipping: list_apps unavailable (likely no accessibility permission): {e:?}"
            );
            return;
        }
    };
    let finder = apps.iter().find(|a| a.name.eq_ignore_ascii_case("Finder"));
    assert!(
        finder.is_some(),
        "Finder should always be running on macOS; got {} apps",
        apps.len()
    );
}

/// End-to-end headless click smoke. Requires the test binary to have
/// accessibility permission. Set `AGENT_DESKTOP_RUN_GUI_TESTS=1` to
/// enable. Clicks at an offscreen-safe coordinate targeting the test
/// process itself; this exercises the full
/// `mouse_event(Click, Some(pid))` -> `synthesize_mouse_to_pid` path
/// without affecting other apps.
#[test]
fn headless_click_to_self_pid_round_trip() {
    if !gui_tests_enabled() {
        eprintln!("skipping: set AGENT_DESKTOP_RUN_GUI_TESTS=1 to enable end-to-end click");
        return;
    }
    let adapter = MacOSAdapter::new();
    let result = adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Click { count: 1 },
            point: Point { x: 1.0, y: 1.0 },
            button: MouseButton::Left,
        },
        Some(self_pid()),
    );
    assert!(
        result.is_ok(),
        "headless click to self pid should succeed: {result:?}"
    );
    assert!(!InteractionPolicy::headless().allow_cursor_move);
}
