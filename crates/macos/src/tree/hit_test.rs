use agent_desktop_core::{
    action::Point,
    adapter::{HitTestResult, NativeHandle},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::{
        AXElement, capabilities, copy_ax_array, copy_string_attr, element::child_attributes,
        read_bounds, roles::ax_role_to_str,
    };
    use accessibility_sys::{
        AXUIElementCopyElementAtPosition, AXUIElementCreateApplication,
        AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout,
        kAXErrorSuccess, kAXRoleAttribute,
    };
    use agent_desktop_core::node::Rect;

    const HIT_WALK_MAX_DEPTH: u8 = 60;

    pub fn hit_test_at_position(
        point: Point,
        target_pid: Option<i32>,
    ) -> Result<Option<HitTestResult>, AdapterError> {
        match target_pid {
            Some(pid) => hit_test_pid(pid, point),
            None => hit_test_system_wide(point),
        }
    }

    fn hit_test_pid(pid: i32, point: Point) -> Result<Option<HitTestResult>, AdapterError> {
        let root = unsafe {
            let r = AXElement(AXUIElementCreateApplication(pid));
            if !r.0.is_null() {
                AXUIElementSetMessagingTimeout(r.0, 2.0);
            }
            r
        };
        if root.0.is_null() {
            return Err(AdapterError::internal(
                "Failed to obtain AX root for hit-test",
            ));
        }

        let mut best: Option<AXElement> = None;
        let mut best_area: f64 = f64::INFINITY;
        walk_for_hit(
            &root,
            point.x,
            point.y,
            0,
            HIT_WALK_MAX_DEPTH,
            &mut best,
            &mut best_area,
        );

        let Some(hit) = best else {
            return Ok(None);
        };
        Ok(Some(build_result(hit)))
    }

    fn hit_test_system_wide(point: Point) -> Result<Option<HitTestResult>, AdapterError> {
        let root = unsafe { AXElement(AXUIElementCreateSystemWide()) };
        if root.0.is_null() {
            return Err(AdapterError::internal(
                "Failed to obtain AX root for hit-test",
            ));
        }

        let mut element_ref: AXUIElementRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyElementAtPosition(
                root.0,
                point.x as f32,
                point.y as f32,
                &mut element_ref,
            )
        };

        if err != kAXErrorSuccess || element_ref.is_null() {
            return Ok(None);
        }

        Ok(Some(build_result(AXElement(element_ref))))
    }

    fn walk_for_hit(
        element: &AXElement,
        x: f64,
        y: f64,
        depth: u8,
        max_depth: u8,
        best: &mut Option<AXElement>,
        best_area: &mut f64,
    ) {
        if depth >= max_depth {
            return;
        }

        let ax_role = copy_string_attr(element, kAXRoleAttribute);
        let bounds = read_bounds(element);

        if let Some(rect) = bounds {
            if rect.contains_point(x, y) {
                let area = rect.area();
                if area > 0.0 && area <= *best_area {
                    if let Some(prev) = best.take() {
                        drop(prev);
                    }
                    *best = Some(element.clone());
                    *best_area = area;
                }
            } else if depth > 0 {
                return;
            }
        }

        let children = collect_children(element, ax_role.as_deref());
        for child in children {
            walk_for_hit(&child, x, y, depth + 1, max_depth, best, best_area);
        }
    }

    fn collect_children(element: &AXElement, ax_role: Option<&str>) -> Vec<AXElement> {
        let mut seen: Vec<AXElement> = Vec::new();
        for attr in child_attributes(ax_role) {
            if let Some(found) = copy_ax_array(element, attr) {
                if !found.is_empty() {
                    seen = found;
                    break;
                }
            }
        }
        seen
    }

    fn build_result(element: AXElement) -> HitTestResult {
        let ax_role = copy_string_attr(&element, kAXRoleAttribute).unwrap_or_default();
        let role = ax_role_to_str(&ax_role).to_string();
        let name = crate::tree::resolve_element_name(&element);
        let bounds = read_bounds(&element);
        let bounds_hash = bounds.as_ref().map(Rect::bounds_hash);
        let available_actions = capabilities::copy_action_names(&element);
        let pid = crate::system::app_ops::pid_from_element(&element);

        let handle = unsafe { NativeHandle::from_ptr(element.0 as *const std::ffi::c_void) };
        std::mem::forget(element);

        HitTestResult {
            handle,
            role,
            name,
            bounds,
            bounds_hash,
            available_actions,
            pid,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn hit_test_at_position(
        _point: Point,
        _target_pid: Option<i32>,
    ) -> Result<Option<HitTestResult>, AdapterError> {
        Err(AdapterError::not_supported("hit_test_at_position"))
    }
}

pub use imp::hit_test_at_position;
