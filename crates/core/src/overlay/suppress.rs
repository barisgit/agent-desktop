//! Suppression clears to zero on guard drop, preserving the action-scope contract where synthetic AX click suppression must not leak beyond the guarded dispatch.

use std::cell::Cell;

thread_local! {
    pub(crate) static SUPPRESS_BROADCAST: Cell<u32> = const { Cell::new(0) };
}

/// Clear any pending cursor broadcast suppression on this thread.
pub fn clear_suppress() {
    SUPPRESS_BROADCAST.with(|count| count.set(0));
}

/// Guard that clears cursor broadcast suppression when dropped.
pub struct SuppressClearGuard;

impl Drop for SuppressClearGuard {
    fn drop(&mut self) {
        clear_suppress();
    }
}

pub(crate) fn bump_suppress(amount: u32) {
    SUPPRESS_BROADCAST.with(|count| count.set(count.get().saturating_add(amount)));
}

pub(crate) fn consume_suppress() -> bool {
    SUPPRESS_BROADCAST.with(|count| {
        let current = count.get();
        if current > 0 {
            count.set(current - 1);
            true
        } else {
            false
        }
    })
}

#[cfg(test)]
pub(crate) fn suppress_count() -> u32 {
    SUPPRESS_BROADCAST.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{MouseButton, MouseEvent, MouseEventKind, Point};
    use crate::overlay::{notify_mouse, notify_synthetic_click};

    fn event() -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Move,
            point: Point { x: 1.0, y: 2.0 },
            button: MouseButton::Left,
        }
    }

    #[test]
    fn guard_decrements_to_zero() {
        clear_suppress();
        let point = Point { x: 10.0, y: 20.0 };
        notify_synthetic_click(point.clone(), MouseButton::Left, 1, None);
        notify_synthetic_click(point.clone(), MouseButton::Left, 1, None);
        notify_synthetic_click(point, MouseButton::Left, 1, None);
        assert_eq!(suppress_count(), 6);

        for expected in (0..6).rev() {
            notify_mouse(&event());
            assert_eq!(suppress_count(), expected);
        }

        notify_synthetic_click(Point { x: 30.0, y: 40.0 }, MouseButton::Left, 1, None);
        assert_eq!(suppress_count(), 2);
        let guard = SuppressClearGuard;
        drop(guard);
        assert_eq!(suppress_count(), 0);
    }

    #[test]
    fn clear_suppress_idempotent() {
        clear_suppress();
        clear_suppress();
        assert_eq!(suppress_count(), 0);
    }
}
