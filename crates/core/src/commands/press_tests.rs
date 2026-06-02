use super::{PressArgs, execute};
use crate::action::KeyCombo;
use crate::action_request::ActionRequest;
use crate::action_result::ActionResult;
use crate::adapter::{NativeHandle, PlatformAdapter};
use crate::error::AdapterError;
use crate::{CommandContext, InteractionPolicy};

struct BlockingAdapter;

impl PlatformAdapter for BlockingAdapter {
    fn is_blocked_combo(&self, _combo: &KeyCombo) -> bool {
        true
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("PressKey"))
    }
}

struct AllowingAdapter;

impl PlatformAdapter for AllowingAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("PressKey"))
    }
}

fn args(combo: &str, force: bool) -> PressArgs {
    PressArgs {
        combo: combo.to_owned(),
        ref_id: None,
        snapshot: None,
        window_id: None,
        app: None,
        target_app: None,
        target_pid: None,
        policy: InteractionPolicy::headed(),
        force,
    }
}

#[test]
fn adapter_blocked_combo_is_refused_when_not_forced() {
    let err = execute(
        args("cmd+q", false),
        &BlockingAdapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "POLICY_DENIED");
    assert!(
        err.to_string().contains("--force"),
        "the refusal must tell the caller how to override, got: {err}"
    );
}

#[test]
fn force_bypasses_the_adapter_block() {
    execute(
        args("cmd+q", true),
        &BlockingAdapter,
        &CommandContext::default(),
    )
    .expect("--force must let the agent send a blocked combo");
}

#[test]
fn core_blocks_nothing_by_default() {
    execute(
        args("cmd+q", false),
        &AllowingAdapter,
        &CommandContext::default(),
    )
    .expect("core must not hardcode any block; the default adapter allows everything");
}

struct WindowPressAdapter {
    windows: Vec<crate::node::WindowInfo>,
    last: std::sync::Mutex<Option<(i32, String, bool)>>,
}

impl PlatformAdapter for WindowPressAdapter {
    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
    ) -> Result<Vec<crate::node::WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }

    fn press_key_for_window(
        &self,
        window: &crate::node::WindowInfo,
        combo: &KeyCombo,
        steal_focus: bool,
    ) -> Result<ActionResult, AdapterError> {
        *self.last.lock().expect("window press state") =
            Some((window.pid, combo.key.clone(), steal_focus));
        Ok(ActionResult::new("press_key"))
    }
}

fn window(id: &str, pid: i32) -> crate::node::WindowInfo {
    crate::node::WindowInfo {
        id: id.into(),
        title: format!("title-{id}"),
        app: "TestApp".into(),
        pid,
        bounds: None,
        is_focused: false,
    }
}

fn window_args(window_id: &str, policy: InteractionPolicy) -> PressArgs {
    PressArgs {
        combo: "return".into(),
        ref_id: None,
        snapshot: None,
        window_id: Some(window_id.into()),
        app: None,
        target_app: None,
        target_pid: None,
        policy,
        force: false,
    }
}

#[test]
fn window_id_routes_headless_press_without_focus_steal() {
    let adapter = WindowPressAdapter {
        windows: vec![window("w-1", 10), window("w-2", 20)],
        last: std::sync::Mutex::new(None),
    };
    execute(
        window_args("w-2", InteractionPolicy::headless()),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(
        *adapter.last.lock().expect("window press state"),
        Some((20, "return".into(), false))
    );
}

#[test]
fn window_id_headed_press_allows_focus_steal() {
    let adapter = WindowPressAdapter {
        windows: vec![window("w-1", 10)],
        last: std::sync::Mutex::new(None),
    };
    execute(
        window_args("w-1", InteractionPolicy::headed()),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(
        *adapter.last.lock().expect("window press state"),
        Some((10, "return".into(), true))
    );
}

#[test]
fn unknown_window_id_returns_invalid_args() {
    let adapter = WindowPressAdapter {
        windows: vec![window("w-1", 10)],
        last: std::sync::Mutex::new(None),
    };
    let error = execute(
        window_args("w-404", InteractionPolicy::headless()),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(adapter.last.lock().expect("window press state").is_none());
}
