use agent_desktop_core::{
    AdapterError, BackgroundPointerReport, Deadline, ErrorCode, MouseEvent, MouseEventKind,
    WindowInfo,
};
use core_graphics::event::{CGEvent, CGEventType, EventField};
use core_graphics::geometry::CGPoint;

use crate::actions::DeliveryTracker;
use crate::input::mouse::{
    create_event_with_source, down_type, ensure_budget, event_flags, event_source, sleep_bounded,
    standalone_state_error, to_cg_button, up_type, validate_point,
};

const BUTTON_HOLD: std::time::Duration = std::time::Duration::from_millis(10);
const MULTI_CLICK_GAP: std::time::Duration = std::time::Duration::from_millis(30);
const FOCUS_SETTLE: std::time::Duration = std::time::Duration::from_millis(100);

/// Posts a mouse event to the process that owns `window` with
/// `CGEventPostToPid`, never through the HID tap, so the system cursor stays
/// put and the window server performs no activation.
///
/// A pid-targeted event carries no window-server hit test, so AppKit relies on
/// `kCGMouseEventWindowUnderMousePointer` (field 91) and
/// `kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent` (field 92) to
/// route it to the right `NSWindow`; both are set to the exact window number.
/// The location stays in global CoreGraphics coordinates. Offscreen or covered
/// windows are fine because nothing is hit-tested against the screen.
///
/// Chromium/Electron caveats (from `render_widget_host_view_cocoa.mm`): the
/// web view ignores `mouseMoved` unless its window is still main or key in its
/// own app, and it refuses first-mouse clicks in an inactive window unless the
/// app opted into `acceptFirstMouse`. Hover therefore works only while the
/// target window remains its app's main window, and a background click may be
/// swallowed. Callers must observe the effect with a fresh snapshot.
pub(crate) fn deliver(
    window: &WindowInfo,
    event: MouseEvent,
    deadline: Deadline,
) -> Result<BackgroundPointerReport, AdapterError> {
    let (pid, events) =
        prepare(window, &event).map_err(|error| DeliveryTracker::default().annotate(error))?;

    let frontmost_pid_before = frontmost_pid(deadline);
    let mut delivery = DeliveryTracker::default();
    ensure_budget(deadline, delivery)?;
    for (index, background_event) in events.iter().enumerate() {
        background_event.post_to_pid(pid);
        delivery.mark_delivered();
        if index + 1 == events.len() {
            break;
        }
        let pause = if is_button_down(background_event) {
            BUTTON_HOLD
        } else {
            MULTI_CLICK_GAP
        };
        std::thread::sleep(pause.min(deadline.remaining()));
    }

    let _ = sleep_bounded(deadline, FOCUS_SETTLE, delivery);
    Ok(BackgroundPointerReport {
        frontmost_pid_before,
        frontmost_pid_after: frontmost_pid(deadline),
    })
}

fn prepare(
    window: &WindowInfo,
    event: &MouseEvent,
) -> Result<(libc::pid_t, Vec<CGEvent>), AdapterError> {
    validate_point(&event.point)?;
    let window_number =
        crate::system::window_resolve::parse_window_number(&window.id).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("'{}' is not a window id", window.id),
            )
        })?;
    let pid = crate::system::process_identity::to_pid_t(window.pid)?;
    Ok((pid, build_events(event, window_number)?))
}

/// Builds, without posting, the pid-targeted events for `event`: one
/// `mouseMoved` for a move, or a down/up pair per click carrying the click
/// state. Kept separate from posting so the event fields are unit-testable.
pub(crate) fn build_events(
    event: &MouseEvent,
    window_number: i64,
) -> Result<Vec<CGEvent>, AdapterError> {
    let source = event_source()?;
    let point = CGPoint::new(event.point.x, event.point.y);
    let button = to_cg_button(&event.button);
    let flags = event_flags(&event.modifiers);
    let create = |event_type: CGEventType, click_state: i64| {
        let created = create_event_with_source(&source, event_type, point, button, flags)?;
        created.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_state);
        created.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
            window_number,
        );
        created.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
            window_number,
        );
        Ok::<_, AdapterError>(created)
    };
    match event.kind {
        MouseEventKind::Move => Ok(vec![create(CGEventType::MouseMoved, 0)?]),
        MouseEventKind::Click { count } => {
            agent_desktop_core::validate_mouse_click_count(count)?;
            let mut events = Vec::new();
            for click_state in 1..=i64::from(count) {
                events.push(create(down_type(&event.button), click_state)?);
                events.push(create(up_type(&event.button), click_state)?);
            }
            Ok(events)
        }
        MouseEventKind::Down | MouseEventKind::Up => Err(standalone_state_error()),
        MouseEventKind::Wheel { .. } => Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Background pointer delivery supports move and click only",
        )),
    }
}

fn is_button_down(event: &CGEvent) -> bool {
    matches!(
        event.get_type(),
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown | CGEventType::OtherMouseDown
    )
}

/// A failed read becomes `None` so the command reports the focus change as
/// unknown instead of failing after events were already delivered.
fn frontmost_pid(deadline: Deadline) -> Option<agent_desktop_core::ProcessId> {
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    let pid = crate::system::window_inventory::focused_application_pid(instant).ok()??;
    crate::system::process_identity::from_pid_t(pid).ok()
}

#[cfg(test)]
#[path = "mouse_background_tests.rs"]
mod tests;
