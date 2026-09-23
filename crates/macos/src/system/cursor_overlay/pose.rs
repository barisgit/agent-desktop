use agent_desktop_core::{
    CursorOverlayControl, CursorOverlayInstruction, CursorOverlayStyle, Point,
};
use std::time::{Duration, Instant};

/// Idle time after the latest targeted action before its cue fades for good.
pub(super) const TARGET_POSE_IDLE_MS: u64 = 3_000;

/// Renderer-side memory for one agent cursor.
///
/// `at` is where the next travel animation starts. `pose_deadline` bounds how
/// long the native renderer may keep presenting the latest targeted pose. Only a
/// new targeted instruction moves the deadline; target visibility changes and
/// Hide/Show lifecycle controls never extend, reset, or replay it.
#[derive(Default)]
pub(super) struct OverlayState {
    pub(super) style: CursorOverlayStyle,
    pub(super) at: Option<Point>,
    pub(super) pose_deadline: Option<Instant>,
}

impl OverlayState {
    pub(super) fn record_target_pose(&mut self, now: Instant) {
        self.pose_deadline = Some(now + Duration::from_millis(TARGET_POSE_IDLE_MS));
    }

    fn expire_pose(&mut self, now: Instant) -> bool {
        if self.pose_deadline.is_none_or(|deadline| now < deadline) {
            return false;
        }
        self.at = None;
        self.pose_deadline = None;
        true
    }
}

/// Fades the retained pose once its idle deadline passes. Callers skip this
/// while a drag is in progress so an active drag is never expired mid-gesture.
pub(super) fn expire_pose(state: &mut OverlayState, now: Instant, fade: impl FnOnce()) {
    if state.expire_pose(now) {
        fade();
    }
}

/// Only instructions bound to an exact target window are presented, so enabling
/// the overlay or an untargeted control never shows a free-floating cursor.
pub(super) fn target_instruction(
    control: &CursorOverlayControl,
) -> Option<&CursorOverlayInstruction> {
    control
        .instruction()
        .filter(|instruction| instruction.window().is_some())
}

/// Updates where the next travel animation starts.
///
/// Hide forgets the landing so a later travel never animates from a stale drag
/// origin, but it leaves `pose_deadline` and the native retained pose intact: a
/// Show before the deadline brings the same cue back, and after the deadline the
/// cue stays gone. Show changes nothing here.
pub(super) fn apply_landing_memory(
    control: &CursorOverlayControl,
    state: &mut OverlayState,
    instruction: Option<&CursorOverlayInstruction>,
) {
    if control.is_hide() {
        state.at = None;
        return;
    }
    if control.is_show() {
        return;
    }
    let Some(instruction) = instruction else {
        return;
    };
    state.at = Some(
        if instruction.phase() == agent_desktop_core::CursorPhase::Drag {
            instruction
                .drag_from()
                .unwrap_or(instruction.destination())
                .clone()
        } else {
            instruction.destination().clone()
        },
    );
}

#[cfg(test)]
#[path = "pose_tests.rs"]
mod tests;
