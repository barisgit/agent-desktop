use crate::{AdapterError, CursorOverlayControl, CursorPhase};
use std::sync::Mutex;

/// What the target does while the overlay cursor travels to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TravelChange {
    #[default]
    Nothing,
    /// The window moves, so its bounds change.
    WindowMoves,
    /// The element moves inside a window that stays put, as when its
    /// content scrolls.
    ElementMoves,
    /// The window is replaced by one from another process.
    WindowReplaced,
}

/// The cursor overlay controls a test adapter received, and how the adapter
/// and its target react to them.
#[derive(Default)]
pub(crate) struct OverlayRecorder {
    /// Every control is refused, as when the renderer is unavailable.
    pub(crate) fail: bool,
    pub(crate) on_travel: TravelChange,
    /// Each control paired with how many events had been delivered when it
    /// was sent, so tests can prove cue order around delivery.
    controls: Mutex<Vec<(usize, CursorOverlayControl)>>,
}

impl OverlayRecorder {
    pub(crate) fn record(
        &self,
        delivered: usize,
        control: &CursorOverlayControl,
    ) -> Result<(), AdapterError> {
        if self.fail {
            return Err(AdapterError::internal("renderer unavailable"));
        }
        self.controls
            .lock()
            .unwrap()
            .push((delivered, control.clone()));
        Ok(())
    }

    pub(crate) fn controls(&self) -> Vec<(usize, CursorOverlayControl)> {
        self.controls.lock().unwrap().clone()
    }

    /// Whether `change` has happened: it is the scripted reaction and a travel
    /// cue was presented.
    pub(crate) fn changed(&self, change: TravelChange) -> bool {
        self.on_travel == change
            && self
                .controls
                .lock()
                .unwrap()
                .iter()
                .any(|(_, control)| phase(control) == Some(CursorPhase::Travel))
    }
}

pub(crate) fn phase(control: &CursorOverlayControl) -> Option<CursorPhase> {
    control.instruction().map(|instruction| instruction.phase())
}
