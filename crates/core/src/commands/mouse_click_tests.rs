use super::*;
use crate::{
    adapter::NativeHandle,
    error::{AdapterError, ErrorCode},
};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};

struct FailingMouseAdapter {
    releases: AtomicUsize,
}

impl PlatformAdapter for FailingMouseAdapter {
    fn hit_test_at_position(
        &self,
        _point: Point,
        _target_pid: Option<i32>,
    ) -> Result<Option<HitTestResult>, AdapterError> {
        let ptr = std::ptr::dangling::<u8>().cast::<c_void>();
        Ok(Some(HitTestResult {
            handle: unsafe { NativeHandle::from_ptr(ptr) },
            role: "button".into(),
            name: Some("Target".into()),
            bounds: None,
            bounds_hash: None,
            available_actions: Vec::new(),
            pid: Some(42),
        }))
    }

    fn mouse_event(
        &self,
        _event: MouseEvent,
        _target_pid: Option<i32>,
        _policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "synthetic mouse failure",
        ))
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn releases_hit_test_handle_when_cg_fallback_fails() {
    let adapter = FailingMouseAdapter {
        releases: AtomicUsize::new(0),
    };
    let args = MouseClickArgs {
        x: 10.0,
        y: 20.0,
        button: MouseButton::Left,
        count: 1,
        policy: InteractionPolicy::headless(),
        target_pid: Some(42),
        target_app: None,
    };

    let error = execute(args, &adapter, &CommandContext::default()).unwrap_err();

    assert_eq!(error.code(), "ACTION_FAILED");
    assert_eq!(adapter.releases.load(Ordering::Relaxed), 1);
}
