use super::test_support::*;
use super::*;
use crate::{DeliverySemantics, ProcessId, refs_test_support::HomeGuard};

fn wheel(dx: f64, dy: f64) -> BackgroundPointerAction {
    BackgroundPointerAction::Wheel {
        dx,
        dy,
        modifiers: vec![Modifier::Shift],
    }
}

#[test]
fn wheel_posts_line_deltas_at_the_point_and_reports_unverified_delivery() {
    let adapter = BackgroundCaptureAdapter::new();

    let value = execute(
        point_args(wheel(0.0, -5.0), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let delivered = adapter.delivered();
    assert_eq!(delivered.len(), 1);
    let (window, event) = &delivered[0];
    assert_eq!(window.id, WINDOW_ID);
    assert!(matches!(
        event.kind,
        MouseEventKind::Wheel {
            delta_x: 0.0,
            delta_y: -5.0
        }
    ));
    assert_eq!((event.point.x, event.point.y), (-2500.0, 300.0));
    assert!(matches!(event.modifiers.as_slice(), [Modifier::Shift]));
    assert_eq!(value["scrolled"], true);
    assert_eq!(value["dy"], -5.0);
    assert_eq!(value["dx"], 0.0);
    assert_eq!(value["background"]["window_id"], WINDOW_ID);
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["disposition"]["retry"], "unsafe");
    assert_eq!(adapter.real_mouse_events(), 0);
}

#[test]
fn ref_scroll_aims_the_wheel_at_the_element_center_of_the_refs_window() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        ref_args(
            BackgroundPointerAction::scroll(crate::Direction::Down, 4),
            snapshot_id,
        ),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let delivered = adapter.delivered();
    assert_eq!(delivered.len(), 1);
    let (_, event) = &delivered[0];
    assert_eq!((event.point.x, event.point.y), (-2850.0, 160.0));
    assert!(matches!(
        event.kind,
        MouseEventKind::Wheel {
            delta_x: 0.0,
            delta_y: -4.0
        }
    ));
    let expected = adapter.expected_windows();
    assert_eq!(expected[0].id, WINDOW_ID);
    assert_eq!(expected[0].pid, ProcessId::new(PID));
}

/// `scroll --direction` follows the `mouse-wheel` line convention: positive
/// `dy` scrolls up and positive `dx` scrolls left.
#[test]
fn scroll_direction_maps_to_mouse_wheel_lines() {
    let lines = |direction| match BackgroundPointerAction::scroll(direction, 3) {
        BackgroundPointerAction::Wheel { dx, dy, modifiers } => {
            assert!(modifiers.is_empty());
            (dx, dy)
        }
        _ => panic!("scroll must map to a wheel"),
    };
    assert_eq!(lines(crate::Direction::Up), (0.0, 3.0));
    assert_eq!(lines(crate::Direction::Down), (0.0, -3.0));
    assert_eq!(lines(crate::Direction::Left), (3.0, 0.0));
    assert_eq!(lines(crate::Direction::Right), (-3.0, 0.0));
}

#[test]
fn zero_or_non_finite_wheel_deltas_are_rejected_before_delivery() {
    let adapter = BackgroundCaptureAdapter::new();

    for (dx, dy) in [(0.0, 0.0), (f64::NAN, 1.0), (0.0, f64::INFINITY)] {
        let err = execute(
            point_args(wheel(dx, dy), -2500.0, 300.0),
            &adapter,
            &CommandContext::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGS");
    }
    assert!(adapter.delivered().is_empty());
}

#[test]
fn ref_scroll_from_a_non_window_surface_is_rejected_before_any_delivery() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot_on(Some(WINDOW_ID), crate::adapter::SnapshotSurface::Sheet);
    let adapter = BackgroundCaptureAdapter::new();

    let err = execute(
        ref_args(
            BackgroundPointerAction::scroll(crate::Direction::Down, 4),
            snapshot_id,
        ),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_NOT_SUPPORTED");
    let AppError::Adapter(error) = err else {
        panic!("expected adapter error");
    };
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(adapter.expected_windows().is_empty());
    assert!(adapter.delivered().is_empty());
}
