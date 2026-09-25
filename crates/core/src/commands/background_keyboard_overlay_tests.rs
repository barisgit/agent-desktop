use super::test_support::*;
use super::*;
use crate::commands::background_cue::test_support::TravelChange;
use crate::{CursorOverlayConfig, CursorPhase, Point, ProcessId, refs_test_support::HomeGuard};

fn overlay_context() -> CommandContext {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CommandContext::default().with_cursor_overlay_session("test-session", config)
}

#[test]
fn background_type_travels_to_the_ref_before_focus_and_keys_then_highlights_it() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let adapter = KeyboardCaptureAdapter::new();

    execute(type_args("hi", snapshot_id), &adapter, &overlay_context()).unwrap();

    assert_eq!(
        adapter.calls(),
        vec![
            "bounds",
            "overlay:Some(Travel)",
            "action:focus",
            "background_keys",
            "overlay:Some(Effect)",
        ]
    );
    let overlay = adapter.overlay.controls();
    let center = Point { x: 250.0, y: 220.0 };
    let window = (ProcessId::new(PID), WINDOW_ID.to_string());
    for (_, control) in &overlay {
        let instruction = control.instruction().unwrap();
        assert_eq!(instruction.destination(), &center);
        assert_eq!(instruction.window(), Some(&window));
        assert!(!instruction.is_click());
    }
    let effect = overlay[1].1.instruction().unwrap();
    assert_eq!(effect.phase(), CursorPhase::Effect);
    assert_eq!(effect.target(), Some(&element_bounds()));
}

#[test]
fn background_type_without_overlay_reads_no_bounds_and_presents_nothing() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let adapter = KeyboardCaptureAdapter::new();

    execute(
        type_args("hi", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.calls(), vec!["action:focus", "background_keys"]);
}

#[test]
fn background_press_presents_no_cursor() {
    let adapter = KeyboardCaptureAdapter::new();

    execute(press_args("cmd+s", false), &adapter, &overlay_context()).unwrap();

    assert_eq!(adapter.calls(), vec!["background_keys"]);
    assert!(adapter.overlay.controls().is_empty());
}

#[test]
fn background_window_type_presents_no_cursor() {
    let adapter = KeyboardCaptureAdapter::new();

    execute(window_type_args("hi"), &adapter, &overlay_context()).unwrap();

    assert_eq!(adapter.calls(), vec!["background_keys"]);
    assert!(adapter.overlay.controls().is_empty());
}

#[test]
fn presentation_failure_never_changes_the_typing_result() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let plain = KeyboardCaptureAdapter::new();
    let expected = execute(
        type_args("hi", snapshot_id.clone()),
        &plain,
        &overlay_context(),
    )
    .unwrap();

    let mut failing = KeyboardCaptureAdapter::new();
    failing.overlay.fail = true;
    let value = execute(type_args("hi", snapshot_id), &failing, &overlay_context()).unwrap();

    assert_eq!(value, expected);
    assert_eq!(failing.delivered().len(), 1);
}

/// A window replaced while the overlay travels must not receive the keys:
/// the window is checked again after the travel, before focus and delivery.
#[test]
fn a_window_replaced_during_travel_receives_no_keys() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.overlay.on_travel = TravelChange::WindowReplaced;

    let err = execute(type_args("hi", snapshot_id), &adapter, &overlay_context()).unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert!(adapter.delivered().is_empty());
    assert_eq!(adapter.calls(), vec!["bounds", "overlay:Some(Travel)"]);
}

/// The bounds read only serves presentation, so it gets a small slice of the
/// deadline instead of the whole remaining budget.
#[test]
fn the_presentation_bounds_read_gets_a_small_capped_budget() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let adapter = KeyboardCaptureAdapter::new();

    execute(type_args("hi", snapshot_id), &adapter, &overlay_context()).unwrap();

    let budgets = adapter.recorded.lock().unwrap().bounds_budgets.clone();
    assert_eq!(budgets.len(), 1);
    assert!(budgets[0] <= Duration::from_millis(CUE_BOUNDS_MAX_MS));
    assert!(budgets[0] > Duration::ZERO);
}

/// With a short `--timeout-ms`, what is left after reserving the delivery
/// allowance and the overlay's arrival wait is too little for presentation,
/// so the cue is skipped and the keys still go out.
#[test]
fn a_budget_without_room_for_presentation_skips_the_cue_and_still_types() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let adapter = KeyboardCaptureAdapter::new();
    let mut args = type_args("hi", snapshot_id);
    args.timeout_ms = Some(500);

    execute(args, &adapter, &overlay_context()).unwrap();

    assert_eq!(adapter.calls(), vec!["action:focus", "background_keys"]);
    assert!(adapter.overlay.controls().is_empty());
    assert_eq!(adapter.delivered().len(), 1);
}
