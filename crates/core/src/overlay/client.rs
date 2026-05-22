use crate::action::{MouseButton, MouseEvent, MouseEventKind, Point};
use crate::node::Bounds;
use crate::overlay::protocol::{
    KeyEvent, format_click, format_error, format_key, format_move, format_scroll, format_set_color,
    format_set_visible, format_target_clear, format_target_set, format_thinking,
};
use crate::overlay::suppress;
#[cfg(unix)]
use crate::overlay::transport::Conn;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

pub(crate) const AGENT_CURSOR_SOCKET: &str = "AGENT_CURSOR_SOCKET";
pub(crate) const AGENT_CURSOR_START_CMD: &str = "AGENT_CURSOR_START_CMD";
pub const ENV_VAR: &str = AGENT_CURSOR_SOCKET;
#[cfg(unix)]
const CONNECT_BUDGET: Duration = Duration::from_millis(50);
#[cfg(unix)]
const POLL_BUDGET: Duration = Duration::from_millis(300);
#[cfg(unix)]
const POLL_INTERVAL: Duration = Duration::from_millis(25);
#[cfg(unix)]
const WRITE_TIMEOUT_MS: u64 = 50;

#[cfg(unix)]
static CLIENT: OnceLock<Mutex<OverlayClient>> = OnceLock::new();

#[cfg(test)]
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(any(test, feature = "test-hooks"))]
/// Counts start-command spawn attempts for lifecycle tests and test-hook builds.
pub(crate) static SPAWN_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Socket client that writes newline-delimited overlay protocol messages.
pub struct OverlayClient {
    #[cfg(unix)]
    stream: Option<Conn>,
    #[cfg(unix)]
    path: String,
}

impl OverlayClient {
    #[cfg(unix)]
    fn new(path: String) -> Self {
        Self { stream: None, path }
    }

    #[cfg(unix)]
    fn ensure_connected(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }

        if let Ok(stream) = try_connect_with_budget(&self.path, CONNECT_BUDGET) {
            self.cache_stream(stream);
            return true;
        }

        let Some(start_cmd) = std::env::var(AGENT_CURSOR_START_CMD)
            .ok()
            .filter(|cmd| !cmd.is_empty())
        else {
            return false;
        };

        if let Err(error) = spawn_start_cmd(&start_cmd) {
            tracing::debug!(?error, "failed to spawn cursor overlay start command");
            return false;
        }

        match poll_connect(&self.path) {
            Some(stream) => {
                self.cache_stream(stream);
                true
            }
            None => false,
        }
    }

    #[cfg(unix)]
    fn cache_stream(&mut self, stream: Conn) {
        let _ = stream.set_write_timeout(Some(Duration::from_millis(WRITE_TIMEOUT_MS)));
        self.stream = Some(stream);
    }

    #[cfg(unix)]
    fn reset_path(&mut self, path: String) {
        if self.path != path {
            self.stream = None;
            self.path = path;
        }
    }

    #[cfg(unix)]
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

    #[cfg(not(unix))]
    fn send_line(&mut self, _line: &str) {}
}

#[cfg(unix)]
/// Uses plain Unix socket connect because std exposes no hard connect timeout for UnixStream; local socket connects fail fast, and the budget is applied to subsequent I/O.
pub(crate) fn try_connect_with_budget(path: &str, budget: Duration) -> std::io::Result<Conn> {
    let stream = Conn::connect(path)?;
    let _ = stream.set_read_timeout(Some(budget));
    let _ = stream.set_write_timeout(Some(budget));
    Ok(stream)
}

#[cfg(unix)]
fn poll_connect(path: &str) -> Option<Conn> {
    let deadline = Instant::now() + POLL_BUDGET;
    loop {
        if let Ok(stream) = try_connect_with_budget(path, POLL_INTERVAL) {
            return Some(stream);
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        thread::sleep(std::cmp::min(POLL_INTERVAL, deadline.duration_since(now)));
    }
}

#[cfg(unix)]
fn spawn_start_cmd(start_cmd: &str) -> std::io::Result<()> {
    bump_spawn_count();

    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(start_cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        unsafe {
            command.pre_exec(|| {
                let result = libc::setsid();
                if result == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    command.spawn().map(|_| ())
}

#[cfg(any(test, feature = "test-hooks"))]
fn bump_spawn_count() {
    use std::sync::atomic::Ordering;

    SPAWN_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(any(test, feature = "test-hooks")))]
fn bump_spawn_count() {}

#[cfg(unix)]
/// Returns the process-global overlay client when socket configuration exists.
pub fn client() -> Option<&'static Mutex<OverlayClient>> {
    let Some(path) = std::env::var(ENV_VAR).ok().filter(|path| !path.is_empty()) else {
        if let Some(client) = CLIENT.get() {
            if let Ok(mut guard) = client.lock() {
                guard.stream = None;
            }
        }
        return None;
    };

    let client = CLIENT.get_or_init(|| Mutex::new(OverlayClient::new(path.clone())));
    if let Ok(mut guard) = client.lock() {
        guard.reset_path(path);
    }
    Some(client)
}

#[cfg(not(unix))]
/// Returns no overlay client on targets without a socket transport.
pub fn client() -> Option<&'static Mutex<OverlayClient>> {
    None
}

/// Returns whether overlay emission is configured for this process.
pub fn is_enabled() -> bool {
    client().is_some()
}

/// Dispatches a physical mouse event to the overlay, respecting suppression.
pub fn notify_mouse(event: &MouseEvent) {
    if suppress::consume_suppress() {
        return;
    }
    notify_mouse_for_pid_inner(event, None);
}

/// Dispatches a synthetic move and click pair for accessibility-driven clicks.
pub fn notify_synthetic_click(
    point: Point,
    button: MouseButton,
    count: u32,
    target_pid: Option<i32>,
) {
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
    suppress::bump_suppress(2);
}

/// Dispatches a scroll event to the overlay.
pub fn notify_scroll(point: Point, dx: f64, dy: f64, target_pid: Option<i32>) {
    send_line(&format_scroll(point.x, point.y, dx, dy, target_pid));
}

/// Dispatches one keyboard text event containing the full input string.
pub fn notify_key_text(text: &str) {
    send_line(&format_key(KeyEvent::Text(text.to_string())));
}

/// Dispatches one keyboard combo event.
pub fn notify_key_combo(combo: &str) {
    send_line(&format_key(KeyEvent::Combo(combo.to_string())));
}

/// Dispatches a target highlight event to the overlay.
pub fn target_set(bounds: Bounds, target_pid: Option<i32>) {
    send_line(&format_target_set(&bounds, target_pid));
}

/// Dispatches a target clear event to the overlay.
pub fn target_clear() {
    send_line(&format_target_clear());
}

/// Dispatches an error event to the overlay.
pub fn notify_error(point: Option<Point>, code: &str, message: &str) {
    send_line(&format_error(point, code, message));
}

/// Dispatches a thinking-state event to the overlay.
pub fn thinking_set(active: bool) {
    send_line(&format_thinking(active));
}

/// Dispatches an overlay visibility event.
pub fn set_visible(visible: bool) {
    send_line(&format_set_visible(visible));
}

/// Dispatches an overlay color event.
pub fn set_color(r: u8, g: u8, b: u8) {
    send_line(&format_set_color(r, g, b));
}

fn notify_mouse_for_pid_inner(event: &MouseEvent, target_pid: Option<i32>) {
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
    send_line(&line);
}

fn send_line(line: &str) {
    let Some(client) = client() else {
        return;
    };
    if let Ok(mut guard) = client.lock() {
        guard.send_line(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (bool, Option<usize>) {
        (
            is_enabled(),
            client().map(|mutex| mutex as *const Mutex<OverlayClient> as usize),
        )
    }

    #[test]
    fn send_with_no_socket_drops_silently() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var(ENV_VAR);
            std::env::remove_var(AGENT_CURSOR_START_CMD);
        }
        notify_mouse(&MouseEvent {
            kind: MouseEventKind::Move,
            point: Point { x: 1.0, y: 2.0 },
            button: MouseButton::Left,
        });
    }

    #[test]
    fn once_lock_lazy_init() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var(ENV_VAR);
            std::env::remove_var(AGENT_CURSOR_START_CMD);
        }
        let first = std::thread::spawn(state).join().unwrap();
        let second = std::thread::spawn(state).join().unwrap();
        assert_eq!(first, second);
        assert_eq!(first, state());
    }
}
