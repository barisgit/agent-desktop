use agent_desktop_core::{action::DragParams, error::AdapterError};
use core_graphics::{
    event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton},
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint,
};

pub(crate) fn drag_sequence(params: DragParams) -> Result<(), AdapterError> {
    tracing::debug!(
        "mouse: drag ({:.0},{:.0}) -> ({:.0},{:.0}) duration={}ms",
        params.from.x,
        params.from.y,
        params.to.x,
        params.to.y,
        params.duration_ms.unwrap_or(300)
    );
    use std::thread::sleep;
    use std::time::Duration;

    const PICKUP_DELAY_MS: u64 = 200;
    const DEFAULT_DROP_DELAY_MS: u64 = 500;
    const DWELL_TICK_MS: u64 = 16;

    let from = CGPoint::new(params.from.x, params.from.y);
    let to = CGPoint::new(params.to.x, params.to.y);
    let duration_ms = params.duration_ms.unwrap_or(300);
    let steps = (duration_ms / DWELL_TICK_MS).max(4) as usize;
    let step_delay = Duration::from_millis(duration_ms / steps as u64);
    let source = event_source()?;

    post_event_with_source(
        &source,
        CGEventType::LeftMouseDown,
        from,
        CGMouseButton::Left,
    )?;
    let mut release = MouseUpGuard {
        source: &source,
        origin: from,
        armed: true,
    };
    sleep(Duration::from_millis(PICKUP_DELAY_MS));

    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let x = params.from.x + (params.to.x - params.from.x) * t;
        let y = params.from.y + (params.to.y - params.from.y) * t;
        post_event_with_source(
            &source,
            CGEventType::LeftMouseDragged,
            CGPoint::new(x, y),
            CGMouseButton::Left,
        )?;
        sleep(step_delay);
    }

    dwell_over_destination(
        &source,
        to,
        params.drop_delay_ms.unwrap_or(DEFAULT_DROP_DELAY_MS),
        DWELL_TICK_MS,
    )?;
    release.release_at(to)
}

/// Releases the left mouse button exactly once. Every fallible step between
/// `LeftMouseDown` and the final `LeftMouseUp` would otherwise leave the
/// button logically held down system-wide on error. The happy path calls
/// `release_at(to)`, which disarms only after the up event actually posts.
/// On any early return, `Drop` cancels the gesture by dragging back to the
/// origin and releasing there — never at the unreached destination, where
/// CGEvent's embedded coordinates would silently commit the aborted drag
/// as a completed drop. The cancel is best-effort twice over: the
/// corrective posts themselves can fail (typically the same systemic
/// CGEventSource failure that aborted the drag, leaving the button held
/// and the cursor wherever the last successful event put it), and a
/// drop target under the origin still sees a self-drop, which most
/// targets treat as a no-op.
struct MouseUpGuard<'a> {
    source: &'a CGEventSource,
    origin: CGPoint,
    armed: bool,
}

impl MouseUpGuard<'_> {
    fn release_at(&mut self, point: CGPoint) -> Result<(), AdapterError> {
        post_event_with_source(
            self.source,
            CGEventType::LeftMouseUp,
            point,
            CGMouseButton::Left,
        )?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for MouseUpGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = post_event_with_source(
                self.source,
                CGEventType::LeftMouseDragged,
                self.origin,
                CGMouseButton::Left,
            );
            let _ = post_event_with_source(
                self.source,
                CGEventType::LeftMouseUp,
                self.origin,
                CGMouseButton::Left,
            );
        }
    }
}

/// Holds the dragged item over the destination while the drop target
/// activates. Posting `LeftMouseDragged` on each tick (instead of a dead
/// sleep) keeps the destination engaged so the release registers as a
/// drop rather than a bare drag — macOS targets can drop the highlight if
/// no movement arrives. A zero delay still posts one settling event.
fn dwell_over_destination(
    source: &CGEventSource,
    to: CGPoint,
    drop_delay_ms: u64,
    tick_ms: u64,
) -> Result<(), AdapterError> {
    use std::thread::sleep;
    use std::time::Duration;

    let ticks = drop_delay_ms.div_ceil(tick_ms).max(1);
    for _ in 0..ticks {
        post_event_with_source(
            source,
            CGEventType::LeftMouseDragged,
            to,
            CGMouseButton::Left,
        )?;
        sleep(Duration::from_millis(tick_ms));
    }
    Ok(())
}

fn event_source() -> Result<CGEventSource, AdapterError> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AdapterError::internal("Failed to create CGEventSource"))
}

fn post_event_with_source(
    source: &CGEventSource,
    event_type: CGEventType,
    point: CGPoint,
    button: CGMouseButton,
) -> Result<(), AdapterError> {
    let event = CGEvent::new_mouse_event(source.clone(), event_type, point, button)
        .map_err(|_| AdapterError::internal("Failed to create mouse event"))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
