use agent_desktop_core::{
    AdapterError, BackgroundPointerReport, Deadline, ErrorCode, MouseEvent, WindowInfo,
};
use core_graphics::event::CGEvent;
use core_graphics::geometry::CGPoint;
use std::time::{Duration, Instant};

use crate::actions::DeliveryTracker;
use crate::input::background_activation::focus_target_window;
use crate::input::background_events::{EventRouting, build, plan, window_local};
use crate::input::background_focus_guard::{FocusGuard, GUARD_WINDOW, GuardIo};
use crate::input::background_frontmost::frontmost_pid;
use crate::input::background_layers::BackgroundLayers;
use crate::input::mouse::{ensure_budget, event_flags, sleep_bounded, validate_point};
use crate::input::skylight;

/// Pause after the focus record so the target processes it before the
/// pointer events arrive (background-computer-use waits 50 ms).
const ACTIVATION_SETTLE: Duration = Duration::from_millis(50);
/// Settle time before the after-sample when the focus guard is off.
const FOCUS_SETTLE: Duration = Duration::from_millis(100);

/// Delivers a mouse event to the process that owns `window` without the HID
/// tap, so the system cursor stays put and the window server hit-tests
/// nothing: offscreen or covered windows are fine.
///
/// Fields 91/92 name the exact window and the click state is always set;
/// [`BackgroundLayers`] (from `AGENT_DESKTOP_BG_LAYERS`) adds the rest. Why a
/// bare `CGEventPostToPid` was not enough (live on macOS 26: no effect in
/// Finder, VS Code, or ClickUp): AppKit routes a pid-targeted event by its
/// window fields and window-local location, and Chromium ignores `mouseMoved`
/// and first-mouse clicks unless its window is main/key in its own app
/// (`render_widget_host_view_cocoa.mm`). `route` supplies the missing routing,
/// `activate` makes the target believe its window is focused without touching
/// the user's app, `skylight` posts the way the window server's own clients
/// do, and `guard` puts the user's app back if the target activates itself.
/// Callers must still observe the effect with a fresh snapshot.
pub(crate) fn deliver(
    window: &WindowInfo,
    event: MouseEvent,
    deadline: Deadline,
) -> Result<BackgroundPointerReport, AdapterError> {
    let prepared =
        prepare(window, &event).map_err(|error| DeliveryTracker::default().annotate(error))?;
    let Prepared {
        pid,
        window_number,
        layers,
        events,
        mut degradations,
    } = prepared;

    let before = frontmost_pid(deadline);
    let mut guard = start_guard(layers, before, &mut degradations);
    let mut io = SystemGuardIo::new(deadline);
    let mut delivery = DeliveryTracker::default();
    ensure_budget(deadline, delivery)?;

    if layers.activate {
        match focus_target_window(pid, window_number) {
            Ok(()) => {
                delivery.mark_delivered();
                std::thread::sleep(ACTIVATION_SETTLE.min(deadline.remaining()));
            }
            Err(reason) => degradations.push(reason),
        }
        sample(&mut guard, &mut io);
    }

    for (built, pause_after) in &events {
        post(pid, built, layers.skylight, &mut degradations);
        delivery.mark_delivered();
        sample(&mut guard, &mut io);
        std::thread::sleep((*pause_after).min(deadline.remaining()));
    }

    match guard.as_mut() {
        Some(guard) => guard.watch(&mut io, GUARD_WINDOW.min(deadline.remaining())),
        None => {
            let _ = sleep_bounded(deadline, FOCUS_SETTLE, delivery);
        }
    }
    if io.restore_unavailable {
        degradations.push("guard:restore_unavailable".to_string());
    }

    let after = frontmost_pid(deadline);
    Ok(BackgroundPointerReport {
        frontmost_pid_before: before.and_then(to_process_id),
        frontmost_pid_after: after.and_then(to_process_id),
        layers: layers.names(),
        degradations,
        focus_guard: guard.map(|guard| guard.finish(after)),
    })
}

struct Prepared {
    pid: libc::pid_t,
    window_number: u32,
    layers: BackgroundLayers,
    events: Vec<(CGEvent, Duration)>,
    degradations: Vec<String>,
}

/// Everything that can fail runs here, before anything is posted, so those
/// failures are reported as not delivered.
fn prepare(window: &WindowInfo, event: &MouseEvent) -> Result<Prepared, AdapterError> {
    validate_point(&event.point)?;
    let layers = BackgroundLayers::from_env()?;
    let window_number = crate::system::window_resolve::parse_window_number(&window.id)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("'{}' is not a window id", window.id),
            )
        })?;
    let pid = crate::system::process_identity::to_pid_t(window.pid)?;

    let origin = window
        .bounds
        .as_ref()
        .map(|bounds| CGPoint::new(bounds.x, bounds.y));
    let mut degradations = Vec::new();
    if origin.is_none() && (layers.route || layers.primer) {
        degradations.push("route:window_origin_unavailable".to_string());
    }

    let planned = plan(event, layers.route, origin.filter(|_| layers.primer))?;
    let routing = EventRouting {
        pid,
        window_number: i64::from(window_number),
        flags: event_flags(&event.modifiers),
        route: layers.route,
    };
    let mut events = Vec::with_capacity(planned.len());
    for planned_event in &planned {
        let built = build(planned_event, &routing)?;
        if let Some(origin) = origin.filter(|_| layers.route) {
            let local = window_local(planned_event.global, origin);
            if !skylight::set_window_location(&built, local) {
                note_once(
                    &mut degradations,
                    "route:CGEventSetWindowLocation_unavailable",
                );
            }
        }
        events.push((built, planned_event.pause_after));
    }

    Ok(Prepared {
        pid,
        window_number,
        layers,
        events,
        degradations,
    })
}

/// Posts through exactly one path: SkyLight when requested and available,
/// otherwise `CGEventPostToPid`, so an event is never delivered twice.
fn post(pid: libc::pid_t, event: &CGEvent, use_skylight: bool, degradations: &mut Vec<String>) {
    if use_skylight {
        if skylight::post_to_pid(pid, event) {
            return;
        }
        note_once(degradations, "skylight:SLEventPostToPid_unavailable");
    }
    event.post_to_pid(pid);
}

fn start_guard(
    layers: BackgroundLayers,
    before: Option<i32>,
    degradations: &mut Vec<String>,
) -> Option<FocusGuard> {
    if !layers.guard {
        return None;
    }
    if before.is_none() {
        degradations.push("guard:frontmost_unknown".to_string());
    }
    before.map(FocusGuard::new)
}

fn sample(guard: &mut Option<FocusGuard>, io: &mut SystemGuardIo) {
    if let Some(guard) = guard.as_mut() {
        guard.sample(io);
    }
}

fn note_once(degradations: &mut Vec<String>, reason: &str) {
    if !degradations.iter().any(|existing| existing == reason) {
        degradations.push(reason.to_string());
    }
}

fn to_process_id(pid: i32) -> Option<agent_desktop_core::ProcessId> {
    crate::system::process_identity::from_pid_t(pid).ok()
}

struct SystemGuardIo {
    started: Instant,
    deadline: Deadline,
    restore_unavailable: bool,
}

impl SystemGuardIo {
    fn new(deadline: Deadline) -> Self {
        Self {
            started: Instant::now(),
            deadline,
            restore_unavailable: false,
        }
    }
}

impl GuardIo for SystemGuardIo {
    fn now(&mut self) -> Duration {
        self.started.elapsed()
    }

    fn frontmost(&mut self) -> Option<i32> {
        frontmost_pid(self.deadline)
    }

    fn restore(&mut self, pid: i32) -> bool {
        let restored = skylight::restore_front_process(pid);
        self.restore_unavailable |= restored.is_none();
        restored.unwrap_or(false)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration.min(self.deadline.remaining()));
    }
}

#[cfg(test)]
#[path = "mouse_background_tests.rs"]
mod tests;
