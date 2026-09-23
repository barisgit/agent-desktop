use super::*;
use agent_desktop_core::{Modifier, MouseButton, Point};
use core_graphics::event::CGEventFlags;

const WINDOW_NUMBER: i64 = 9555;

fn event(kind: MouseEventKind, button: MouseButton, modifiers: Vec<Modifier>) -> MouseEvent {
    MouseEvent {
        kind,
        point: Point {
            x: -2850.0,
            y: 160.0,
        },
        button,
        modifiers,
    }
}

fn type_code(event: &CGEvent) -> u32 {
    event.get_type() as u32
}

fn window_fields(event: &CGEvent) -> (i64, i64) {
    (
        event.get_integer_value_field(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER),
        event.get_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
        ),
    )
}

#[test]
fn window_routing_fields_use_the_documented_sdk_numbers() {
    assert_eq!(EventField::MOUSE_EVENT_CLICK_STATE, 1);
    assert_eq!(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER, 91);
    assert_eq!(
        EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
        92
    );
}

#[test]
fn move_builds_one_mouse_moved_tagged_with_the_window() {
    let events = build_events(
        &event(MouseEventKind::Move, MouseButton::Left, Vec::new()),
        WINDOW_NUMBER,
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(type_code(&events[0]), CGEventType::MouseMoved as u32);
    assert_eq!(window_fields(&events[0]), (WINDOW_NUMBER, WINDOW_NUMBER));
    let location = events[0].location();
    assert_eq!((location.x, location.y), (-2850.0, 160.0));
}

#[test]
fn double_right_click_builds_down_up_pairs_with_click_state_and_modifiers() {
    let events = build_events(
        &event(
            MouseEventKind::Click { count: 2 },
            MouseButton::Right,
            vec![Modifier::Shift],
        ),
        WINDOW_NUMBER,
    )
    .unwrap();

    let observed: Vec<(u32, i64)> = events
        .iter()
        .map(|event| {
            (
                type_code(event),
                event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
            )
        })
        .collect();
    let down = CGEventType::RightMouseDown as u32;
    let up = CGEventType::RightMouseUp as u32;
    assert_eq!(observed, vec![(down, 1), (up, 1), (down, 2), (up, 2)]);
    for built in &events {
        assert_eq!(window_fields(built), (WINDOW_NUMBER, WINDOW_NUMBER));
        assert!(built.get_flags().contains(CGEventFlags::CGEventFlagShift));
        assert!(is_button_down(built) == (type_code(built) == down));
    }
}

#[test]
fn wheel_and_standalone_button_state_are_not_built() {
    let wheel = build_events(
        &event(
            MouseEventKind::Wheel {
                delta_x: 0.0,
                delta_y: 1.0,
            },
            MouseButton::Left,
            Vec::new(),
        ),
        WINDOW_NUMBER,
    )
    .err()
    .expect("event must be rejected");
    let down = build_events(
        &event(MouseEventKind::Down, MouseButton::Left, Vec::new()),
        WINDOW_NUMBER,
    )
    .err()
    .expect("event must be rejected");

    assert_eq!(wheel.code, ErrorCode::ActionNotSupported);
    assert_eq!(down.code, ErrorCode::ActionNotSupported);
}

#[test]
fn zero_click_count_is_rejected() {
    let err = build_events(
        &event(
            MouseEventKind::Click { count: 0 },
            MouseButton::Left,
            Vec::new(),
        ),
        WINDOW_NUMBER,
    )
    .err()
    .expect("event must be rejected");

    assert_eq!(err.code, ErrorCode::InvalidArgs);
}

#[test]
fn failures_before_posting_are_reported_as_not_delivered() {
    let window = |id: &str| WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: "Code".to_string(),
        pid: agent_desktop_core::ProcessId::new(std::process::id()),
        process_instance: None,
        bounds: None,
        state: agent_desktop_core::WindowState::default(),
    };
    let cases = [
        (window("not-a-window"), MouseEventKind::Move),
        (
            window("w-9555"),
            MouseEventKind::Wheel {
                delta_x: 0.0,
                delta_y: 1.0,
            },
        ),
    ];

    for (target, kind) in cases {
        let deadline = Deadline::after(1_000).unwrap();
        let err = deliver(
            &target,
            event(kind, MouseButton::Left, Vec::new()),
            deadline,
        )
        .expect_err("must fail before posting");

        assert_eq!(
            err.disposition,
            agent_desktop_core::DeliverySemantics::not_delivered()
        );
    }
}
