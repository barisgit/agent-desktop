use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind},
    adapter::PlatformAdapter,
    commands::{
        helpers::{resolve_raw_mouse_target_pid, resolve_raw_mouse_target_pid_with_ref},
        mouse_click,
    },
    context::CommandContext,
    error::{AdapterError, AppError, ErrorCode},
    interaction_policy::InteractionPolicy,
    node::AppInfo,
};
use std::sync::Mutex;

struct AppsAdapter {
    apps: Vec<AppInfo>,
    last_event: Mutex<Option<(MouseEvent, Option<i32>)>>,
}

impl AppsAdapter {
    fn new(apps: Vec<AppInfo>) -> Self {
        Self {
            apps,
            last_event: Mutex::new(None),
        }
    }
}

impl PlatformAdapter for AppsAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(self.apps.clone())
    }

    fn mouse_event(&self, event: MouseEvent, target_pid: Option<i32>) -> Result<(), AdapterError> {
        *self.last_event.lock().unwrap() = Some((event, target_pid));
        Ok(())
    }
}

fn app(name: &str, pid: i32) -> AppInfo {
    AppInfo {
        name: name.into(),
        pid,
        bundle_id: None,
    }
}

#[test]
fn resolve_returns_target_pid_when_provided() {
    let adapter = AppsAdapter::new(vec![]);
    let pid =
        resolve_raw_mouse_target_pid(Some(4321), None, InteractionPolicy::headless(), &adapter)
            .unwrap();
    assert_eq!(pid, Some(4321));
}

#[test]
fn resolve_target_pid_and_target_app_rejected_as_mutually_exclusive() {
    let adapter = AppsAdapter::new(vec![app("TextEdit", 100)]);
    let err = resolve_raw_mouse_target_pid(
        Some(4321),
        Some("TextEdit"),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap_err();
    match err {
        AppError::Adapter(e) => {
            assert_eq!(e.code, ErrorCode::InvalidArgs);
            assert!(
                e.suggestion
                    .as_deref()
                    .is_some_and(|s| s.contains("--target-pid") && s.contains("--target-app"))
            );
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn resolve_app_name_case_insensitive_single_match() {
    let adapter = AppsAdapter::new(vec![app("TextEdit", 100), app("Finder", 200)]);
    let pid = resolve_raw_mouse_target_pid(
        None,
        Some("textedit"),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap();
    assert_eq!(pid, Some(100));
}

#[test]
fn resolve_app_unknown_returns_app_not_found() {
    let adapter = AppsAdapter::new(vec![app("TextEdit", 100)]);
    let err =
        resolve_raw_mouse_target_pid(None, Some("Bogus"), InteractionPolicy::headless(), &adapter)
            .unwrap_err();
    match err {
        AppError::Adapter(e) => {
            assert_eq!(e.code, ErrorCode::AppNotFound);
            assert!(e.message.contains("Bogus"));
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn resolve_app_multiple_matches_returns_invalid_args_with_pids() {
    let adapter = AppsAdapter::new(vec![app("Helper", 100), app("helper", 200)]);
    let err = resolve_raw_mouse_target_pid(
        None,
        Some("Helper"),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap_err();
    match err {
        AppError::Adapter(e) => {
            assert_eq!(e.code, ErrorCode::InvalidArgs);
            let detail = e.platform_detail.unwrap();
            assert!(detail.contains("100"));
            assert!(detail.contains("200"));
            assert!(e.suggestion.unwrap().contains("--target-pid"));
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn resolve_headless_no_target_returns_invalid_args_with_suggestion() {
    let adapter = AppsAdapter::new(vec![app("TextEdit", 100)]);
    let err = resolve_raw_mouse_target_pid(None, None, InteractionPolicy::headless(), &adapter)
        .unwrap_err();
    match err {
        AppError::Adapter(e) => {
            assert_eq!(e.code, ErrorCode::InvalidArgs);
            let suggestion = e.suggestion.unwrap();
            assert!(suggestion.contains("--target-app"));
            assert!(suggestion.contains("--target-pid"));
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn resolve_headed_no_target_returns_none() {
    let adapter = AppsAdapter::new(vec![]);
    let pid =
        resolve_raw_mouse_target_pid(None, None, InteractionPolicy::headed(), &adapter).unwrap();
    assert_eq!(pid, None);
}

#[test]
fn resolve_focus_fallback_no_target_returns_none() {
    let adapter = AppsAdapter::new(vec![]);
    let pid =
        resolve_raw_mouse_target_pid(None, None, InteractionPolicy::focus_fallback(), &adapter)
            .unwrap();
    assert_eq!(pid, None);
}

#[test]
fn mouse_click_headed_default_passes_none_target_pid() {
    let adapter = AppsAdapter::new(vec![]);
    mouse_click::execute(
        mouse_click::MouseClickArgs {
            x: 100.0,
            y: 200.0,
            button: MouseButton::Left,
            count: 1,
            policy: InteractionPolicy::headed(),
            target_pid: None,
            target_app: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();
    let (event, target_pid) = adapter.last_event.lock().unwrap().clone().unwrap();
    assert_eq!(target_pid, None);
    assert_eq!(event.point.x, 100.0);
    assert_eq!(event.point.y, 200.0);
    assert!(matches!(event.kind, MouseEventKind::Click { count: 1 }));
}

#[test]
fn mouse_click_headless_with_target_pid_routes_to_adapter() {
    let adapter = AppsAdapter::new(vec![]);
    mouse_click::execute(
        mouse_click::MouseClickArgs {
            x: 50.0,
            y: 75.0,
            button: MouseButton::Right,
            count: 2,
            policy: InteractionPolicy::headless(),
            target_pid: Some(7777),
            target_app: None,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    let (_event, target_pid) = adapter.last_event.lock().unwrap().clone().unwrap();
    assert_eq!(target_pid, Some(7777));
}

#[test]
fn resolve_with_ref_uses_ref_pid_under_headless_when_no_target() {
    let adapter = AppsAdapter::new(vec![]);
    let pid = resolve_raw_mouse_target_pid_with_ref(
        None,
        None,
        Some(9090),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap();
    assert_eq!(pid, Some(9090));
}

#[test]
fn resolve_with_ref_rejects_both_explicit_targets() {
    let adapter = AppsAdapter::new(vec![app("Finder", 222)]);
    let err = resolve_raw_mouse_target_pid_with_ref(
        Some(1111),
        Some("Finder"),
        Some(9090),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap_err();
    match err {
        AppError::Adapter(e) => assert_eq!(e.code, ErrorCode::InvalidArgs),
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn resolve_with_ref_explicit_target_pid_wins_over_ref_pid() {
    let adapter = AppsAdapter::new(vec![]);
    let pid = resolve_raw_mouse_target_pid_with_ref(
        Some(1111),
        None,
        Some(9090),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap();
    assert_eq!(pid, Some(1111));
}

#[test]
fn resolve_with_ref_explicit_target_app_wins_over_ref_pid() {
    let adapter = AppsAdapter::new(vec![app("Finder", 222)]);
    let pid = resolve_raw_mouse_target_pid_with_ref(
        None,
        Some("Finder"),
        Some(9090),
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap();
    assert_eq!(pid, Some(222));
}

#[test]
fn resolve_with_ref_physical_ignores_ref_pid_and_returns_none() {
    let adapter = AppsAdapter::new(vec![]);
    let pid = resolve_raw_mouse_target_pid_with_ref(
        None,
        None,
        Some(9090),
        InteractionPolicy::headed(),
        &adapter,
    )
    .unwrap();
    assert_eq!(pid, None);
}

#[test]
fn resolve_with_ref_headless_no_ref_pid_still_errors() {
    let adapter = AppsAdapter::new(vec![]);
    let err = resolve_raw_mouse_target_pid_with_ref(
        None,
        None,
        None,
        InteractionPolicy::headless(),
        &adapter,
    )
    .unwrap_err();
    match err {
        AppError::Adapter(e) => assert_eq!(e.code, ErrorCode::InvalidArgs),
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn mouse_click_headless_no_target_short_circuits_before_adapter() {
    let adapter = AppsAdapter::new(vec![]);
    let err = mouse_click::execute(
        mouse_click::MouseClickArgs {
            x: 0.0,
            y: 0.0,
            button: MouseButton::Left,
            count: 1,
            policy: InteractionPolicy::headless(),
            target_pid: None,
            target_app: None,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert!(matches!(err, AppError::Adapter(ref e) if e.code == ErrorCode::InvalidArgs));
    assert!(adapter.last_event.lock().unwrap().is_none());
}
