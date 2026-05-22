#![cfg(unix)]

use agent_desktop_core::action::*;
use agent_desktop_core::overlay::*;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Serializes process-global environment mutation in this integration-test crate.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn key_text_aggregates_single_event() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_key_text("hello world");

    wait_for_len(&recording, 1);
    let recording = settle(&recording);
    assert_eq!(recording.len(), 1);
    assert_eq!(
        recording[0].get("kind").and_then(Value::as_str),
        Some("key")
    );
    assert_eq!(
        recording[0].get("text").and_then(Value::as_str),
        Some("hello world")
    );
    assert!(recording[0].get("combo").is_none());
}

#[test]
fn success_emits_no_error_event() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_scroll(Point { x: 1.0, y: 2.0 }, 0.0, -3.0, None);
    notify_key_text("ok");
    notify_key_combo("cmd+s");

    wait_for_len(&recording, 3);
    let recording = settle(&recording);
    assert!(
        recording
            .iter()
            .all(|entry| entry.get("kind").and_then(Value::as_str) != Some("error"))
    );
}

#[test]
fn single_click_emits_no_thinking() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_mouse(&MouseEvent {
        kind: MouseEventKind::Click { count: 1 },
        point: Point { x: 10.0, y: 20.0 },
        button: MouseButton::Left,
    });

    wait_for_len(&recording, 1);
    let recording = settle(&recording);
    assert!(
        recording
            .iter()
            .all(|entry| entry.get("kind").and_then(Value::as_str) != Some("thinking"))
    );
}

#[test]
fn scroll_records_dx_dy() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_scroll(Point { x: 100.0, y: 200.0 }, 1.0, -3.0, None);

    wait_for_len(&recording, 1);
    let recording = settle(&recording);
    assert_eq!(
        recording[0].get("kind").and_then(Value::as_str),
        Some("scroll")
    );
    assert_eq!(recording[0].get("x").and_then(Value::as_i64), Some(100));
    assert_eq!(recording[0].get("y").and_then(Value::as_i64), Some(200));
    assert_eq!(recording[0].get("dx").and_then(Value::as_i64), Some(1));
    assert_eq!(recording[0].get("dy").and_then(Value::as_i64), Some(-3));
}

#[test]
fn key_combo_records_combo_field_only() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_key_combo("cmd+s");

    wait_for_len(&recording, 1);
    let recording = settle(&recording);
    assert_eq!(
        recording[0].get("kind").and_then(Value::as_str),
        Some("key")
    );
    assert_eq!(
        recording[0].get("combo").and_then(Value::as_str),
        Some("cmd+s")
    );
    assert!(recording[0].get("text").is_none());
}

#[test]
fn error_omits_xy_when_none() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = TempSocket::new();
    let _env = EnvGuard::set_socket(socket.path());
    let recording = spawn_listener(socket.path());

    notify_error(None, "STALE_REF", "refresh snapshot");

    wait_for_len(&recording, 1);
    let recording = settle(&recording);
    assert_eq!(
        recording[0].get("kind").and_then(Value::as_str),
        Some("error")
    );
    assert_eq!(
        recording[0].get("code").and_then(Value::as_str),
        Some("STALE_REF")
    );
    assert!(recording[0].get("x").is_none());
    assert!(recording[0].get("y").is_none());
}

fn spawn_listener(path: &Path) -> Arc<Mutex<Vec<Value>>> {
    let listener = UnixListener::bind(path).unwrap();
    let recording = Arc::new(Mutex::new(Vec::new()));
    let thread_recording = Arc::clone(&recording);
    thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        read_lines(stream, &thread_recording);
    });
    recording
}

fn read_lines(stream: UnixStream, recording: &Arc<Mutex<Vec<Value>>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(75)));
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    recording.lock().unwrap().push(value);
                }
            }
        }
    }
}

fn wait_for_len(recording: &Arc<Mutex<Vec<Value>>>, len: usize) {
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if recording.lock().unwrap().len() >= len {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn settle(recording: &Arc<Mutex<Vec<Value>>>) -> Vec<Value> {
    thread::sleep(Duration::from_millis(100));
    recording.lock().unwrap().clone()
}

struct EnvGuard {
    socket: Option<std::ffi::OsString>,
    start_cmd: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_socket(path: &Path) -> Self {
        let guard = Self {
            socket: std::env::var_os("AGENT_CURSOR_SOCKET"),
            start_cmd: std::env::var_os("AGENT_CURSOR_START_CMD"),
        };
        unsafe {
            std::env::set_var("AGENT_CURSOR_SOCKET", path);
            std::env::remove_var("AGENT_CURSOR_START_CMD");
        }
        guard
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.socket {
                Some(value) => std::env::set_var("AGENT_CURSOR_SOCKET", value),
                None => std::env::remove_var("AGENT_CURSOR_SOCKET"),
            }
            match &self.start_cmd {
                Some(value) => std::env::set_var("AGENT_CURSOR_START_CMD", value),
                None => std::env::remove_var("AGENT_CURSOR_START_CMD"),
            }
        }
    }
}

struct TempSocket {
    dir: PathBuf,
    path: PathBuf,
}

impl TempSocket {
    fn new() -> Self {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = PathBuf::from(format!(
            "/tmp/ad-overlay-wire-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("overlay.sock");
        Self { dir, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir_all(&self.dir);
    }
}
