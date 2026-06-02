#[cfg(target_os = "macos")]
use agent_desktop_core::InteractionPolicy;

#[cfg(target_os = "macos")]
pub(crate) fn physical_click_permitted(policy: InteractionPolicy) -> bool {
    policy.allow_focus_steal && policy.allow_cursor_move
}

#[cfg(target_os = "macos")]
pub(crate) fn headless_pid_click_permitted(policy: InteractionPolicy) -> bool {
    !policy.allow_focus_steal && !policy.allow_cursor_move
}

#[cfg(target_os = "macos")]
pub(crate) fn focus_cycle_permitted(policy: InteractionPolicy) -> bool {
    policy.allow_focus_steal
}

#[cfg(test)]
mod tests {
    use super::{focus_cycle_permitted, headless_pid_click_permitted, physical_click_permitted};
    use agent_desktop_core::InteractionPolicy;

    #[test]
    fn focus_cycle_requires_focus_steal() {
        assert!(focus_cycle_permitted(InteractionPolicy::headed()));
        assert!(focus_cycle_permitted(InteractionPolicy::focus_fallback()));
        assert!(!focus_cycle_permitted(InteractionPolicy::headless()));
    }

    #[test]
    fn pid_delivery_requires_headless_policy() {
        assert!(headless_pid_click_permitted(InteractionPolicy::headless()));
        assert!(!headless_pid_click_permitted(
            InteractionPolicy::focus_fallback()
        ));
        assert!(!headless_pid_click_permitted(InteractionPolicy::headed()));
    }

    #[test]
    fn physical_delivery_requires_headed_policy() {
        assert!(physical_click_permitted(InteractionPolicy::headed()));
        assert!(!physical_click_permitted(
            InteractionPolicy::focus_fallback()
        ));
        assert!(!physical_click_permitted(InteractionPolicy::headless()));
    }

    #[test]
    fn delivery_predicates_are_disjoint() {
        for policy in [
            InteractionPolicy::headed(),
            InteractionPolicy::focus_fallback(),
            InteractionPolicy::headless(),
        ] {
            assert!(!(focus_cycle_permitted(policy) && headless_pid_click_permitted(policy)));
            assert!(!(physical_click_permitted(policy) && headless_pid_click_permitted(policy)));
        }
    }
}
