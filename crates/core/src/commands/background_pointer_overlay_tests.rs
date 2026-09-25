use super::test_support::*;
use super::*;
use crate::commands::background_cue::test_support::TravelChange;
use crate::{
    CursorOverlayConfig, CursorOverlayControl, CursorPhase, DeliverySemantics, ProcessId,
    refs_test_support::HomeGuard,
};

fn overlay_context() -> CommandContext {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CommandContext::default().with_cursor_overlay_session("test-session", config)
}

fn cue(control: &CursorOverlayControl) -> &crate::CursorOverlayInstruction {
    control.instruction().expect("present control")
}

fn bound_window() -> (ProcessId, String) {
    (ProcessId::new(PID), WINDOW_ID.to_string())
}

fn disposition(err: AppError) -> DeliverySemantics {
    let AppError::Adapter(error) = err else {
        panic!("expected adapter error");
    };
    error.disposition
}

fn point() -> Point {
    Point {
        x: -2500.0,
        y: 300.0,
    }
}

#[test]
fn point_click_travels_to_the_point_before_delivery_then_clicks_after_it() {
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &overlay_context(),
    )
    .unwrap();

    let overlay = adapter.overlay.controls();
    assert_eq!(overlay.len(), 2);

    let (delivered_before_travel, travel) = &overlay[0];
    assert_eq!(*delivered_before_travel, 0);
    assert_eq!(cue(travel).phase(), CursorPhase::Travel);
    assert_eq!(cue(travel).destination(), &point());
    assert_eq!(cue(travel).window(), Some(&bound_window()));
    assert!(!cue(travel).is_click());

    let (delivered_before_effect, effect) = &overlay[1];
    assert_eq!(*delivered_before_effect, 1);
    assert_eq!(cue(effect).phase(), CursorPhase::Effect);
    assert_eq!(cue(effect).destination(), &point());
    assert_eq!(cue(effect).window(), Some(&bound_window()));
    assert!(cue(effect).is_click());
    assert_eq!(cue(effect).target(), None);
}

#[test]
fn ref_hover_presents_at_the_element_center_and_highlights_its_bounds() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        ref_args(BackgroundPointerAction::Hover, snapshot_id),
        &adapter,
        &overlay_context(),
    )
    .unwrap();

    let overlay = adapter.overlay.controls();
    assert_eq!(overlay.len(), 2);
    let center = Point {
        x: -2850.0,
        y: 160.0,
    };
    assert_eq!(cue(&overlay[0].1).destination(), &center);
    assert_eq!(cue(&overlay[0].1).window(), Some(&bound_window()));
    let effect = cue(&overlay[1].1);
    assert_eq!(effect.destination(), &center);
    assert_eq!(effect.target(), Some(&adapter.element_bounds));
    assert!(!effect.is_click());
}

#[test]
fn background_wheel_travels_to_the_point_and_plays_a_non_click_effect() {
    let adapter = BackgroundCaptureAdapter::new();
    let wheel = BackgroundPointerAction::Wheel {
        dx: 0.0,
        dy: -5.0,
        modifiers: Vec::new(),
    };

    execute(
        point_args(wheel, -2500.0, 300.0),
        &adapter,
        &overlay_context(),
    )
    .unwrap();

    let overlay = adapter.overlay.controls();
    let phases: Vec<_> = overlay
        .iter()
        .map(|(delivered, control)| (*delivered, cue(control).phase()))
        .collect();
    assert_eq!(
        phases,
        [(0, CursorPhase::Travel), (1, CursorPhase::Effect)],
        "travel before the wheel, effect after it"
    );
    assert!(!cue(&overlay[1].1).is_click());
    assert_eq!(cue(&overlay[1].1).window(), Some(&bound_window()));
}

#[test]
fn disabled_overlay_presents_nothing_and_checks_the_window_once() {
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert!(adapter.overlay.controls().is_empty());
    assert_eq!(adapter.delivered().len(), 1);
    assert_eq!(adapter.expected_windows().len(), 1);
}

#[test]
fn presentation_failure_never_changes_the_delivery_result() {
    let plain = BackgroundCaptureAdapter::new();
    let expected = execute(
        point_args(left_click(2), -2500.0, 300.0),
        &plain,
        &CommandContext::default(),
    )
    .unwrap();

    let mut failing = BackgroundCaptureAdapter::new();
    failing.overlay.fail = true;
    let value = execute(
        point_args(left_click(2), -2500.0, 300.0),
        &failing,
        &overlay_context(),
    )
    .unwrap();

    assert_eq!(value, expected);
    assert_eq!(failing.delivered().len(), 1);
}

#[test]
fn undelivered_event_travels_but_shows_no_effect() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.fail_delivery = true;

    let err = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &overlay_context(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    let overlay = adapter.overlay.controls();
    assert_eq!(overlay.len(), 1);
    assert_eq!(cue(&overlay[0].1).phase(), CursorPhase::Travel);
}

#[test]
fn rejected_point_presents_nothing() {
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(left_click(1), 10.0, 10.0),
        &adapter,
        &overlay_context(),
    )
    .unwrap_err();

    assert!(adapter.overlay.controls().is_empty());
}

/// The travel can wait up to the arrival timeout. A window that moves in
/// that time puts other content under the global point, so nothing is
/// posted and the refusal is safe to retry.
#[test]
fn a_window_that_moves_during_travel_receives_nothing() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.overlay.on_travel = TravelChange::WindowMoves;

    let err = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &overlay_context(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(disposition(err), DeliverySemantics::not_delivered());
    assert!(adapter.delivered().is_empty());
    assert_eq!(
        adapter.expected_windows().len(),
        2,
        "the window is checked again after the travel"
    );
    let overlay = adapter.overlay.controls();
    assert_eq!(overlay.len(), 1, "no effect for an event never posted");
    assert_eq!(cue(&overlay[0].1).phase(), CursorPhase::Travel);
}

/// A ref's element can move inside a window that stays put, for example when
/// its content scrolls. The point aimed at its old center is refused.
#[test]
fn a_ref_element_that_moves_during_travel_receives_nothing() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_in(Some("test-session"), Some(WINDOW_ID));
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.overlay.on_travel = TravelChange::ElementMoves;

    let err = execute(
        ref_args(left_click(1), snapshot_id),
        &adapter,
        &overlay_context(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(disposition(err), DeliverySemantics::not_delivered());
    assert!(adapter.delivered().is_empty());
    assert_eq!(adapter.overlay.controls().len(), 1);
}

#[test]
fn multi_agent_cues_carry_the_calling_agent_id() {
    let config = CursorOverlayConfig::enabled(None, 6)
        .expect("valid config")
        .with_multi_agent(true);
    let context = CommandContext::default()
        .with_agent_id(Some("agent-a".into()))
        .unwrap()
        .with_cursor_overlay_session("test-session", config);
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(BackgroundPointerAction::Move, -2500.0, 300.0),
        &adapter,
        &context,
    )
    .unwrap();

    let overlay = adapter.overlay.controls();
    assert_eq!(overlay.len(), 2);
    assert!(
        overlay
            .iter()
            .all(|(_, control)| control.agent_id() == Some("agent-a"))
    );
}
