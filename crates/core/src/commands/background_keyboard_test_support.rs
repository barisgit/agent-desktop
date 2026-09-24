use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::{
    ActionRequest, ActionResult, AdapterError, BackgroundDeliveryReport, ErrorCode, KeyCombo,
    ProcessId, WindowState, capability,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
};
use std::sync::Mutex;

pub(super) const PID: u32 = 4242;
pub(super) const WINDOW_ID: &str = "w-15592";

pub(super) fn live_window(pid: u32) -> WindowInfo {
    WindowInfo {
        id: WINDOW_ID.into(),
        title: "live title".into(),
        app: "Code".into(),
        pid: ProcessId::new(pid),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: WindowState::default(),
    }
}

/// Records, in order, every accessibility action, background key delivery,
/// and app-level key press so tests can prove which path a command took.
pub(super) struct KeyboardCaptureAdapter {
    pub(super) live_pid: u32,
    pub(super) focus_error: Option<ErrorCode>,
    pub(super) stale_ref: bool,
    pub(super) report: BackgroundDeliveryReport,
    pub(super) calls: Mutex<Vec<String>>,
    pub(super) focus_policies: Mutex<Vec<crate::InteractionPolicy>>,
    pub(super) delivered: Mutex<Vec<(WindowInfo, BackgroundKeyInput)>>,
    pub(super) expected_windows: Mutex<Vec<WindowInfo>>,
}

impl KeyboardCaptureAdapter {
    pub(super) fn new() -> Self {
        Self {
            live_pid: PID,
            focus_error: None,
            stale_ref: false,
            report: BackgroundDeliveryReport {
                frontmost_pid_before: Some(ProcessId::new(7)),
                frontmost_pid_after: Some(ProcessId::new(7)),
                layers: vec!["route".into(), "skylight".into()],
                ..BackgroundDeliveryReport::default()
            },
            calls: Mutex::new(Vec::new()),
            focus_policies: Mutex::new(Vec::new()),
            delivered: Mutex::new(Vec::new()),
            expected_windows: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(super) fn delivered(&self) -> Vec<(WindowInfo, BackgroundKeyInput)> {
        self.delivered.lock().unwrap().clone()
    }
}

impl ObservationOps for KeyboardCaptureAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        if self.stale_ref {
            return Err(AdapterError::new(ErrorCode::StaleRef, "element changed"));
        }
        Ok(NativeHandle::null())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![live_window(PID)])
    }
}

impl ActionOps for KeyboardCaptureAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("action:{}", request.action.name()));
        self.focus_policies.lock().unwrap().push(request.policy);
        match &self.focus_error {
            Some(code) => Err(AdapterError::new(code.clone(), "AXFocused did not stick")),
            None => Ok(ActionResult::delivered_unverified("focus")),
        }
    }
}

impl InputOps for KeyboardCaptureAdapter {
    fn background_key_input(
        &self,
        window: &WindowInfo,
        input: &BackgroundKeyInput,
        _lease: &crate::InteractionLease,
    ) -> Result<BackgroundDeliveryReport, AdapterError> {
        self.calls.lock().unwrap().push("background_keys".into());
        self.delivered
            .lock()
            .unwrap()
            .push((window.clone(), input.clone()));
        Ok(self.report.clone())
    }
}

impl SystemOps for KeyboardCaptureAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.expected_windows.lock().unwrap().push(window.clone());
        Ok(live_window(self.live_pid))
    }

    fn is_blocked_combo(&self, combo: &KeyCombo) -> bool {
        combo.key == "q" && !combo.modifiers.is_empty()
    }

    fn press_key_for_app(
        &self,
        _process: crate::ProcessIdentity,
        _combo: &KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.calls.lock().unwrap().push("press_key_for_app".into());
        Ok(ActionResult::delivered_unverified("press_key"))
    }
}

pub(super) fn ref_snapshot(source_window_id: Option<&str>) -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        process: crate::RefProcess {
            pid: ProcessId::new(PID),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            retained_object: None,
            role: "textfield".into(),
            name: Some("The editor is not accessible at this time".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: None,
            bounds_hash: None,
        },
        capabilities: crate::RefCapabilities {
            states: vec!["focused".into()],
            available_actions: vec![capability::SET_VALUE.into()],
        },
        source: crate::RefSource {
            source_app: Some("Code".into()),
            source_window_id: source_window_id.map(str::to_string),
            source_window_title: Some("stale title".into()),
            source_window_bounds_hash: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    });
    store.save_new_snapshot(&refmap).unwrap()
}

pub(super) fn press_args(combo: &str, force: bool) -> BackgroundKeyboardArgs {
    BackgroundKeyboardArgs {
        input: BackgroundKeyboardInput::Press {
            combo: combo.into(),
            force,
        },
        target: BackgroundKeyboardTarget::Window {
            window_id: WINDOW_ID.into(),
        },
        timeout_ms: None,
    }
}

pub(super) fn type_args(text: &str, snapshot_id: String) -> BackgroundKeyboardArgs {
    BackgroundKeyboardArgs {
        input: BackgroundKeyboardInput::Type { text: text.into() },
        target: BackgroundKeyboardTarget::Ref {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
        },
        timeout_ms: None,
    }
}
