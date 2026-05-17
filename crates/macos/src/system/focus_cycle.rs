use agent_desktop_core::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use agent_desktop_core::adapter::WindowFilter;

    pub fn focus_cycle_click_via_bounds(
        el: &crate::tree::AXElement,
        button: MouseButton,
        count: u32,
        policy: agent_desktop_core::InteractionPolicy,
    ) -> Result<(), AdapterError> {
        if !policy.allow_focus_steal {
            return Err(AdapterError::policy_denied(
                "Focus-cycle click requires a policy that permits focus steal",
            ));
        }
        let target_pid = crate::system::app_ops::pid_from_element(el).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                "Element has no resolvable process id",
            )
            .with_suggestion("AX action failed and focus-cycle click is unavailable")
        })?;
        let bounds = crate::tree::read_bounds(el).ok_or_else(|| {
            AdapterError::new(ErrorCode::ActionFailed, "Element has no readable bounds")
                .with_suggestion("AX action failed and focus-cycle click is unavailable")
        })?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Err(
                AdapterError::new(ErrorCode::ActionFailed, "Element has zero-size bounds")
                    .with_suggestion("Element may be hidden or off-screen. Try 'scroll-to' first."),
            );
        }
        let center = Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        };
        focus_cycle_click_at(center, button, count, target_pid)
    }

    pub fn focus_cycle_raw_click(
        point: Point,
        button: MouseButton,
        count: u32,
        target_pid: i32,
        policy: agent_desktop_core::InteractionPolicy,
    ) -> Result<(), AdapterError> {
        if !policy.allow_focus_steal {
            return Err(AdapterError::policy_denied(
                "Focus-cycle raw click requires a policy that permits focus steal",
            ));
        }
        focus_cycle_click_at(point, button, count, target_pid)
    }

    fn focus_cycle_click_at(
        center: Point,
        button: MouseButton,
        count: u32,
        target_pid: i32,
    ) -> Result<(), AdapterError> {
        let prior_pid = current_frontmost_pid();
        tracing::debug!(
            target_pid,
            prior_pid = ?prior_pid,
            x = center.x,
            y = center.y,
            ?button,
            count,
            "focus-cycle: activate target -> broadcast click -> restore prior"
        );
        crate::system::app_ops::ensure_app_focused(target_pid)?;
        std::thread::sleep(std::time::Duration::from_millis(30));
        let click = crate::input::mouse::synthesize_mouse(MouseEvent {
            kind: MouseEventKind::Click { count },
            point: center,
            button,
        });
        if let Some(prior) = prior_pid {
            if prior != target_pid {
                if let Err(err) = crate::system::app_ops::ensure_app_focused(prior) {
                    tracing::debug!(
                        prior,
                        target_pid,
                        ?err,
                        "focus-cycle: failed to restore prior frontmost (best-effort)"
                    );
                }
            }
        }
        click
    }

    fn current_frontmost_pid() -> Option<i32> {
        let wins = crate::system::window_list::list_windows_impl(&WindowFilter {
            focused_only: true,
            app: None,
        })
        .ok()?;
        wins.into_iter().find(|w| w.is_focused).map(|w| w.pid)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn focus_cycle_click_via_bounds(
        _el: &crate::tree::AXElement,
        _button: MouseButton,
        _count: u32,
        _policy: agent_desktop_core::InteractionPolicy,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_cycle_click"))
    }

    pub fn focus_cycle_raw_click(
        _point: Point,
        _button: MouseButton,
        _count: u32,
        _target_pid: i32,
        _policy: agent_desktop_core::InteractionPolicy,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_cycle_raw_click"))
    }
}

pub use imp::{focus_cycle_click_via_bounds, focus_cycle_raw_click};
