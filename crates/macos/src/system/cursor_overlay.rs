use agent_desktop_core::action::{MouseButton, MouseEvent, MouseEventKind};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const ENV_VAR: &str = "AGENT_CURSOR_SOCKET";
const WRITE_TIMEOUT_MS: u64 = 50;

thread_local! {
    static SUPPRESS_BROADCAST: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct OverlayClient {
    stream: Option<UnixStream>,
    path: String,
}

impl OverlayClient {
    fn new(path: String) -> Self {
        Self { stream: None, path }
    }

    fn ensure_connected(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }
        match UnixStream::connect(&self.path) {
            Ok(s) => {
                let _ = s.set_write_timeout(Some(Duration::from_millis(WRITE_TIMEOUT_MS)));
                self.stream = Some(s);
                true
            }
            Err(_) => false,
        }
    }

    fn send_line(&mut self, line: &str) {
        if !self.ensure_connected() {
            return;
        }
        let Some(stream) = self.stream.as_mut() else {
            return;
        };
        if stream.write_all(line.as_bytes()).is_err() || stream.write_all(b"\n").is_err() {
            self.stream = None;
        }
    }
}

fn client() -> Option<&'static Mutex<OverlayClient>> {
    static CLIENT: OnceLock<Option<Mutex<OverlayClient>>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            std::env::var(ENV_VAR)
                .ok()
                .filter(|s| !s.is_empty())
                .map(|path| Mutex::new(OverlayClient::new(path)))
        })
        .as_ref()
}

pub fn notify_mouse(event: &MouseEvent) {
    let suppressed = SUPPRESS_BROADCAST.with(|c| {
        let n = c.get();
        if n > 0 {
            c.set(n - 1);
            true
        } else {
            false
        }
    });
    if suppressed {
        return;
    }
    notify_mouse_for_pid_inner(event, None);
}

/// Clear any pending broadcast-suppression count on this thread.
pub fn clear_suppress() {
    SUPPRESS_BROADCAST.with(|c| c.set(0));
}

/// Emit a synthetic move+click pair for AX-driven actions that never touch the
/// raw mouse pipeline. Bumps the broadcast-suppression counter so any CGEvent
/// fallback that fires during the same action does not double-emit.
pub fn notify_ax_click(
    point: agent_desktop_core::action::Point,
    button: MouseButton,
    count: u32,
    target_pid: Option<i32>,
) {
    if client().is_none() {
        return;
    }
    notify_mouse_for_pid_inner(
        &MouseEvent {
            kind: MouseEventKind::Move,
            point: point.clone(),
            button: button.clone(),
        },
        target_pid,
    );
    notify_mouse_for_pid_inner(
        &MouseEvent {
            kind: MouseEventKind::Click { count },
            point,
            button,
        },
        target_pid,
    );
    SUPPRESS_BROADCAST.with(|c| c.set(c.get().saturating_add(2)));
}

pub fn notify_mouse_for_pid(event: &MouseEvent, target_pid: Option<i32>) {
    notify_mouse_for_pid_inner(event, target_pid);
}

/// Pre-notify the overlay with target_pid, then suppress the next broadcast notify_mouse
/// on this thread. Used by focus_cycle, which routes through broadcast synthesize_mouse
/// after pre-tagging the event with the target app.
pub fn notify_mouse_for_pid_then_suppress(event: &MouseEvent, target_pid: i32) {
    notify_mouse_for_pid_inner(event, Some(target_pid));
    SUPPRESS_BROADCAST.with(|c| c.set(c.get().saturating_add(1)));
}

fn notify_mouse_for_pid_inner(event: &MouseEvent, target_pid: Option<i32>) {
    let Some(client) = client() else {
        return;
    };
    let line = match &event.kind {
        MouseEventKind::Move => format_move(event.point.x, event.point.y, target_pid),
        MouseEventKind::Click { count } => format_click(
            event.point.x,
            event.point.y,
            &event.button,
            *count,
            target_pid,
        ),
        MouseEventKind::Down | MouseEventKind::Up => return,
    };
    if let Ok(mut guard) = client.lock() {
        guard.send_line(&line);
    }
}

fn format_move(x: f64, y: f64, target_pid: Option<i32>) -> String {
    match target_pid {
        Some(pid) => format!(
            r#"{{"kind":"move","x":{},"y":{},"target_pid":{}}}"#,
            fmt_num(x),
            fmt_num(y),
            pid
        ),
        None => format!(r#"{{"kind":"move","x":{},"y":{}}}"#, fmt_num(x), fmt_num(y)),
    }
}

fn format_click(
    x: f64,
    y: f64,
    button: &MouseButton,
    count: u32,
    target_pid: Option<i32>,
) -> String {
    match target_pid {
        Some(pid) => format!(
            r#"{{"kind":"click","x":{},"y":{},"button":"{}","count":{},"target_pid":{}}}"#,
            fmt_num(x),
            fmt_num(y),
            button_str(button),
            count,
            pid
        ),
        None => format!(
            r#"{{"kind":"click","x":{},"y":{},"button":"{}","count":{}}}"#,
            fmt_num(x),
            fmt_num(y),
            button_str(button),
            count
        ),
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v)
    }
}

fn button_str(b: &MouseButton) -> &'static str {
    match b {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::action::Point;

    #[test]
    fn format_move_integer_coords() {
        assert_eq!(
            format_move(100.0, 200.0, None),
            r#"{"kind":"move","x":100,"y":200}"#
        );
    }

    #[test]
    fn format_move_fractional_coords() {
        assert_eq!(
            format_move(100.5, 200.75, None),
            r#"{"kind":"move","x":100.50,"y":200.75}"#
        );
    }

    #[test]
    fn format_move_with_target_pid() {
        assert_eq!(
            format_move(10.0, 20.0, Some(4242)),
            r#"{"kind":"move","x":10,"y":20,"target_pid":4242}"#
        );
    }

    #[test]
    fn format_click_left_single() {
        assert_eq!(
            format_click(50.0, 60.0, &MouseButton::Left, 1, None),
            r#"{"kind":"click","x":50,"y":60,"button":"left","count":1}"#
        );
    }

    #[test]
    fn format_click_right_double() {
        assert_eq!(
            format_click(0.0, 0.0, &MouseButton::Right, 2, None),
            r#"{"kind":"click","x":0,"y":0,"button":"right","count":2}"#
        );
    }

    #[test]
    fn format_click_with_target_pid() {
        assert_eq!(
            format_click(1.0, 2.0, &MouseButton::Left, 1, Some(99)),
            r#"{"kind":"click","x":1,"y":2,"button":"left","count":1,"target_pid":99}"#
        );
    }

    #[test]
    fn notify_silent_when_env_unset() {
        unsafe {
            std::env::remove_var(ENV_VAR);
        }
        notify_mouse(&MouseEvent {
            kind: MouseEventKind::Move,
            point: Point { x: 1.0, y: 2.0 },
            button: MouseButton::Left,
        });
    }
}
