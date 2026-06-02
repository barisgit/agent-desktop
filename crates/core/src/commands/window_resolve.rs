use crate::{
    action::WindowOp,
    adapter::{PlatformAdapter, WindowFilter},
    commands::helpers::AppArgs,
    error::AppError,
    node::WindowInfo,
    window_lookup,
};
use serde_json::{Value, json};

pub(crate) fn resolve_app_pid(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<i32, AppError> {
    if let Some(name) = app {
        let apps = adapter.list_apps()?;
        apps.into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.pid)
            .ok_or_else(|| AppError::invalid_input(format!("App '{name}' not found")))
    } else {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = adapter.list_windows(&filter)?;
        windows
            .first()
            .map(|w| w.pid)
            .ok_or_else(|| AppError::invalid_input("No focused window. Use --app to specify."))
    }
}

pub fn window_op_command(
    args: AppArgs,
    adapter: &dyn PlatformAdapter,
    op: WindowOp,
    response_key: &'static str,
) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let win = match find_window_for_pid(pid, adapter) {
        Ok(win) => win,
        Err(_) if matches!(op, WindowOp::Restore) => WindowInfo {
            id: String::new(),
            title: String::new(),
            app: args.app.unwrap_or_default(),
            pid,
            bounds: None,
            is_focused: false,
        },
        Err(err) => return Err(err),
    };
    adapter.window_op(&win, op)?;
    Ok(json!({ response_key: true }))
}

pub fn find_window_for_pid(
    pid: i32,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    window_lookup::find_window_for_pid(pid, adapter)
}

pub fn resolve_window_by_id(
    window_id: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    adapter
        .list_windows(&filter)?
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or_else(|| AppError::invalid_input(format!("Window '{window_id}' not found")))
}

pub(crate) fn resolve_window_for_app(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let pid = resolve_app_pid(app, adapter)?;
    find_window_for_pid(pid, adapter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::NativeHandle;
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
}
