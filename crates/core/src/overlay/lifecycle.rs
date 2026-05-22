use super::client::{self, AGENT_CURSOR_START_CMD, ENV_VAR, notify_mouse};
use crate::action::{MouseButton, MouseEvent, MouseEventKind, Point};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct EnvGuard {
    socket: Option<std::ffi::OsString>,
    start_cmd: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            socket: std::env::var_os(ENV_VAR),
            start_cmd: std::env::var_os(AGENT_CURSOR_START_CMD),
        }
    }

    fn set_socket(&self, path: &Path) {
        unsafe {
            std::env::set_var(ENV_VAR, path);
        }
    }

    fn set_start_cmd(&self, command: &str) {
        unsafe {
            std::env::set_var(AGENT_CURSOR_START_CMD, command);
        }
    }

    fn unset_socket(&self) {
        unsafe {
            std::env::remove_var(ENV_VAR);
        }
    }

    fn unset_start_cmd(&self) {
        unsafe {
            std::env::remove_var(AGENT_CURSOR_START_CMD);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.socket {
                Some(value) => std::env::set_var(ENV_VAR, value),
                None => std::env::remove_var(ENV_VAR),
            }
            match &self.start_cmd {
                Some(value) => std::env::set_var(AGENT_CURSOR_START_CMD, value),
                None => std::env::remove_var(AGENT_CURSOR_START_CMD),
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
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = PathBuf::from(format!(
            "/tmp/agent-desktop-overlay-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("overlay.sock");
        Self { dir, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn hot_path_no_spawn() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    env.set_socket(socket.path());
    env.unset_start_cmd();
    let (reader, lines) = spawn_bound_recorder(socket.path().to_path_buf(), 1);

    notify_mouse(&event());

    let received = lines.recv_timeout(Duration::from_millis(500)).unwrap();
    reader.join().unwrap();
    assert_eq!(spawn_count(), 0);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].trim_end(), r#"{"kind":"move","x":12,"y":34}"#);
}

#[test]
fn cold_path_start_cmd_opens_socket() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    let marker = socket.dir().join("co_spawned");
    env.set_socket(socket.path());
    env.set_start_cmd(&format!("touch {}", marker.display()));
    let (start_binding, reader, lines) =
        spawn_delayed_recorder(socket.path().to_path_buf(), Duration::from_millis(75), 1);

    start_binding.send(()).unwrap();
    notify_mouse(&event());

    let received = lines.recv_timeout(Duration::from_millis(500)).unwrap();
    reader.join().unwrap();
    assert_eq!(spawn_count(), 1);
    assert!(marker.exists());
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].trim_end(), r#"{"kind":"move","x":12,"y":34}"#);
}

#[test]
fn backward_compat() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    env.set_socket(socket.path());
    env.unset_start_cmd();

    notify_mouse(&event());

    assert_eq!(spawn_count(), 0);
}

#[test]
fn failing_start_cmd_drops_silently() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    env.set_socket(socket.path());
    env.set_start_cmd("/bin/false");

    notify_mouse(&event());

    assert_eq!(spawn_count(), 1);
}

#[test]
fn missing_binary_no_panic() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    env.set_socket(socket.path());
    env.set_start_cmd("/does/not/exist/anywhere");

    notify_mouse(&event());

    assert_eq!(spawn_count(), 1);
}

#[test]
fn concurrent_summon() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    let socket = TempSocket::new();
    env.set_socket(socket.path());
    env.set_start_cmd(&format!("touch {}/co_spawn_$$", socket.dir().display()));
    let (start_binding, reader, lines) =
        spawn_delayed_recorder(socket.path().to_path_buf(), Duration::from_millis(75), 4);
    let barrier = Arc::new(Barrier::new(5));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                notify_mouse(&event());
            })
        })
        .collect();
    let started = Instant::now();

    start_binding.send(()).unwrap();
    barrier.wait();
    for handle in handles {
        handle.join().unwrap();
    }

    assert!(started.elapsed() < Duration::from_secs(1));
    let marker_count = wait_marker_count(socket.dir(), Duration::from_millis(500));
    assert!((1..=4).contains(&marker_count));
    let received = lines.recv_timeout(Duration::from_millis(500)).unwrap();
    reader.join().unwrap();
    assert!(!received.is_empty());
    assert!(received.len() <= 4);
}

#[test]
fn both_unset_no_activity() {
    let _g = client::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = EnvGuard::new();
    reset_spawn_count();
    env.unset_socket();
    env.unset_start_cmd();

    notify_mouse(&event());

    assert_eq!(spawn_count(), 0);
}

fn event() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        point: Point { x: 12.0, y: 34.0 },
        button: MouseButton::Left,
    }
}

fn reset_spawn_count() {
    client::SPAWN_COUNT.store(0, Ordering::Relaxed);
}

fn spawn_count() -> u32 {
    client::SPAWN_COUNT.load(Ordering::Relaxed)
}

fn spawn_bound_recorder(
    path: PathBuf,
    expected: usize,
) -> (thread::JoinHandle<()>, mpsc::Receiver<Vec<String>>) {
    let listener = UnixListener::bind(path).unwrap();
    spawn_recorder(listener, expected)
}

fn spawn_delayed_recorder(
    path: PathBuf,
    delay: Duration,
    expected: usize,
) -> (
    mpsc::Sender<()>,
    thread::JoinHandle<()>,
    mpsc::Receiver<Vec<String>>,
) {
    let (start_tx, start_rx) = mpsc::channel();
    let (lines_tx, lines_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        start_rx.recv().unwrap();
        thread::sleep(delay);
        let listener = UnixListener::bind(path).unwrap();
        let lines = read_lines(listener, expected);
        lines_tx.send(lines).unwrap();
    });
    (start_tx, handle, lines_rx)
}

fn spawn_recorder(
    listener: UnixListener,
    expected: usize,
) -> (thread::JoinHandle<()>, mpsc::Receiver<Vec<String>>) {
    let (lines_tx, lines_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let lines = read_lines(listener, expected);
        lines_tx.send(lines).unwrap();
    });
    (handle, lines_rx)
}

fn read_lines(listener: UnixListener, expected: usize) -> Vec<String> {
    let (stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    read_stream_lines(stream, expected)
}

fn read_stream_lines(stream: UnixStream, expected: usize) -> Vec<String> {
    let mut reader = BufReader::new(stream);
    let mut lines = Vec::new();
    for _ in 0..expected {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => lines.push(line),
        }
    }
    lines
}

fn wait_marker_count(dir: &Path, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let count = marker_count(dir);
        if count > 0 || Instant::now() >= deadline {
            return count;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn marker_count(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("co_spawn_"))
        .count()
}
