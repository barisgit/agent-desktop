use crate::{
    AdapterError, CursorPhase, InteractionLease, Point, Rect, WindowInfo, adapter::PlatformAdapter,
    context::CommandContext, cursor_overlay::PointerCue,
};

/// Cursor overlay cue for one opt-in background delivery.
///
/// The real cursor never moves on the background path, so the overlay is the
/// only thing a watching user sees. Every cue is bound to the exact pid and
/// window the event is posted to, which gives it the same native visibility
/// gating as ref actions: it stays hidden while that window is hidden or
/// covered, follows the window when it moves, and fades after the idle rest.
/// Presentation is best effort; failures are logged by the overlay sender and
/// never change the delivery result.
pub(crate) struct BackgroundCue {
    cue: PointerCue,
    target: Option<Rect>,
    click: bool,
}

impl BackgroundCue {
    /// `target` is the element outlined when the effect lands; `click` plays
    /// the click ripple instead of the hover effect.
    pub(crate) fn new(
        window: &WindowInfo,
        point: Point,
        target: Option<Rect>,
        click: bool,
    ) -> Self {
        Self {
            cue: PointerCue {
                point,
                window: Some((window.pid, window.id.clone())),
            },
            target,
            click,
        }
    }

    /// Moves the overlay cursor to the point and waits, up to the arrival
    /// timeout, for it to land. Like ref actions, it is skipped when the
    /// action budget cannot also cover that wait.
    ///
    /// Returns whether a travel was attempted. The target may have moved or
    /// closed during the wait, so the caller must then revalidate it
    /// immediately before delivery.
    pub(crate) fn travel(
        &self,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
        lease: &InteractionLease,
    ) -> bool {
        if !context.cursor_overlay().is_enabled() {
            return false;
        }
        let Some(_scope) = crate::cursor_overlay::travel_scope(lease) else {
            return false;
        };
        crate::cursor_overlay::submit(
            adapter,
            context,
            &self.cue,
            None,
            false,
            CursorPhase::Travel,
        );
        true
    }

    /// Plays the effect only when the event may have reached the target, so an
    /// event refused before posting never looks delivered.
    pub(crate) fn effect<T>(
        &self,
        adapter: &dyn PlatformAdapter,
        context: &CommandContext,
        result: &Result<T, AdapterError>,
    ) {
        if let Err(error) = result
            && !crate::cursor_overlay::confirms_delivery(error.disposition)
        {
            return;
        }
        crate::cursor_overlay::submit(
            adapter,
            context,
            &self.cue,
            self.target,
            self.click,
            CursorPhase::Effect,
        );
    }
}

#[cfg(test)]
#[path = "background_overlay_test_support.rs"]
pub(crate) mod test_support;
