use super::*;
use crate::error::ErrorCode;

struct WindowListAdapter {
    windows: Vec<WindowInfo>,
}

fn window(id: &str, pid: i32) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: format!("title-{id}"),
        app: "TestApp".into(),
        pid,
        bounds: None,
        is_focused: false,
    }
}

impl PlatformAdapter for WindowListAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> Result<Vec<WindowInfo>, crate::error::AdapterError> {
        Ok(self.windows.clone())
    }
}

#[test]
fn resolve_window_by_id_returns_matching_window() {
    let adapter = WindowListAdapter {
        windows: vec![window("w-1", 10), window("w-2", 20)],
    };
    let win = resolve_window_by_id("w-2", &adapter).unwrap();
    assert_eq!(win.id, "w-2");
    assert_eq!(win.pid, 20);
}

#[test]
fn resolve_window_by_id_errors_when_absent() {
    let adapter = WindowListAdapter {
        windows: vec![window("w-1", 10)],
    };
    let err = resolve_window_by_id("w-99", &adapter).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgs.as_str());
}
