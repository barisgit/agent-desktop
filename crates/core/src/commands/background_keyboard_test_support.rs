use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::commands::background_cue::test_support::{OverlayRecorder, TravelChange, phase};
use crate::{
    ActionRequest, ActionResult, AdapterError, BackgroundDeliveryReport, CursorOverlayControl,
    ErrorCode, KeyCombo, ProcessId, Rect, WindowState, capability,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
};
use std::sync::Mutex;
use std::time::Duration;

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

/// What the element's window does with an accessibility focus request.
pub(super) enum FocusBehavior {
    /// Focus moves to the element and the read-back confirms it.
    Moves,
    /// The write is accepted but focus stays on another field, so the
    /// read-back shows that field instead of the element.
    StaysOnAnotherField,
    /// The focus write itself fails.
    Fails(ErrorCode),
}

/// Everything the adapter was asked to do, in order.
#[derive(Default)]
pub(super) struct Recorded {
    pub(super) calls: Vec<String>,
    pub(super) focus_policies: Vec<crate::InteractionPolicy>,
    pub(super) delivered: Vec<(WindowInfo, BackgroundKeyInput)>,
    pub(super) expected_windows: Vec<WindowInfo>,
    /// The deadline budget each element bounds read was given.
    pub(super) bounds_budgets: Vec<Duration>,
}

/// Records every accessibility action, background key delivery, and
/// app-level key press so tests can prove which path a command took.
pub(super) struct KeyboardCaptureAdapter {
    pub(super) live_pid: u32,
    pub(super) focus: FocusBehavior,
    pub(super) stale_ref: bool,
    pub(super) report: BackgroundDeliveryReport,
    pub(super) recorded: Mutex<Recorded>,
    pub(super) overlay: OverlayRecorder,
}

impl KeyboardCaptureAdapter {
    pub(super) fn new() -> Self {
        Self {
            live_pid: PID,
            focus: FocusBehavior::Moves,
            stale_ref: false,
            report: BackgroundDeliveryReport {
                frontmost_pid_before: Some(ProcessId::new(7)),
                frontmost_pid_after: Some(ProcessId::new(7)),
                layers: vec!["route".into(), "skylight".into()],
                ..BackgroundDeliveryReport::default()
            },
            recorded: Mutex::new(Recorded::default()),
            overlay: OverlayRecorder::default(),
        }
    }

    pub(super) fn calls(&self) -> Vec<String> {
        self.recorded.lock().unwrap().calls.clone()
    }

    pub(super) fn delivered(&self) -> Vec<(WindowInfo, BackgroundKeyInput)> {
        self.recorded.lock().unwrap().delivered.clone()
    }

    fn call(&self, name: impl Into<String>) {
        self.recorded.lock().unwrap().calls.push(name.into());
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

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        self.call("bounds");
        self.recorded
            .lock()
            .unwrap()
            .bounds_budgets
            .push(deadline.remaining());
        Ok(Some(element_bounds()))
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
        self.call(format!("action:{}", request.action.name()));
        self.recorded
            .lock()
            .unwrap()
            .focus_policies
            .push(request.policy);
        match &self.focus {
            FocusBehavior::Moves => {
                Ok(ActionResult::delivered_unverified("focus").with_verified_delivery())
            }
            FocusBehavior::StaysOnAnotherField => Ok(ActionResult::delivered_unverified("focus")),
            FocusBehavior::Fails(code) => {
                Err(AdapterError::new(code.clone(), "AXFocused did not stick"))
            }
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
        self.call("background_keys");
        self.recorded
            .lock()
            .unwrap()
            .delivered
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
        self.recorded
            .lock()
            .unwrap()
            .expected_windows
            .push(window.clone());
        if self.overlay.changed(TravelChange::WindowReplaced) {
            return Ok(live_window(self.live_pid + 1));
        }
        Ok(live_window(self.live_pid))
    }

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.call(format!("overlay:{:?}", phase(control)));
        let delivered = self.recorded.lock().unwrap().delivered.len();
        self.overlay.record(delivered, control)
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
        self.call("press_key_for_app");
        Ok(ActionResult::delivered_unverified("press_key"))
    }
}

pub(super) fn element_bounds() -> Rect {
    Rect {
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 40.0,
    }
}

pub(super) fn ref_snapshot(source_window_id: Option<&str>) -> String {
    ref_snapshot_on(source_window_id, crate::adapter::SnapshotSurface::Window)
}

pub(super) fn ref_snapshot_in(session_id: Option<&str>, source_window_id: Option<&str>) -> String {
    store_ref_snapshot(
        session_id,
        source_window_id,
        crate::adapter::SnapshotSurface::Window,
    )
}

pub(super) fn ref_snapshot_on(
    source_window_id: Option<&str>,
    source_surface: crate::adapter::SnapshotSurface,
) -> String {
    store_ref_snapshot(None, source_window_id, source_surface)
}

fn store_ref_snapshot(
    session_id: Option<&str>,
    source_window_id: Option<&str>,
    source_surface: crate::adapter::SnapshotSurface,
) -> String {
    let store = RefStore::for_session(session_id).unwrap();
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
            source_surface,
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

pub(super) fn window_type_args(text: &str) -> BackgroundKeyboardArgs {
    BackgroundKeyboardArgs {
        input: BackgroundKeyboardInput::Type { text: text.into() },
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
