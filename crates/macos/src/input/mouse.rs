use agent_desktop_core::{
    action::{DragParams, MouseButton, MouseEvent, MouseEventKind},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::event::{
        CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;
    use foreign_types::ForeignType;

    pub fn synthesize_mouse(event: MouseEvent) -> Result<(), AdapterError> {
        tracing::debug!(
            "mouse: {:?} {:?} at ({:.0}, {:.0})",
            event.kind,
            event.button,
            event.point.x,
            event.point.y
        );
        let point = CGPoint::new(event.point.x, event.point.y);
        let cg_button = to_cg_button(&event.button);
        match event.kind {
            MouseEventKind::Move => post_event(CGEventType::MouseMoved, point, cg_button),
            MouseEventKind::Down => post_event(down_type(&event.button), point, cg_button),
            MouseEventKind::Up => post_event(up_type(&event.button), point, cg_button),
            MouseEventKind::Click { count } => {
                synthesize_click(point, cg_button, &event.button, count)
            }
        }
    }

    pub fn synthesize_mouse_to_pid(event: MouseEvent, pid: i32) -> Result<(), AdapterError> {
        let window_id = crate::system::cg_window::find_cg_window_id_for_pid(pid);
        tracing::debug!(
            "mouse-to-pid: pid={} window_id={:?} {:?} {:?} at ({:.0}, {:.0})",
            pid,
            window_id,
            event.kind,
            event.button,
            event.point.x,
            event.point.y
        );
        let point = CGPoint::new(event.point.x, event.point.y);
        let cg_button = to_cg_button(&event.button);
        match event.kind {
            MouseEventKind::Move => {
                post_event_to_pid(CGEventType::MouseMoved, point, cg_button, pid, window_id)
            }
            MouseEventKind::Down => {
                post_event_to_pid(down_type(&event.button), point, cg_button, pid, window_id)
            }
            MouseEventKind::Up => {
                post_event_to_pid(up_type(&event.button), point, cg_button, pid, window_id)
            }
            MouseEventKind::Click { count } => {
                synthesize_click_to_pid(point, cg_button, &event.button, count, pid, window_id)
            }
        }
    }

    pub fn synthesize_drag(params: DragParams) -> Result<(), AdapterError> {
        crate::input::mouse_drag::drag_sequence(params).map_err(|err| {
            if err.suggestion.is_some() {
                return err;
            }
            err.with_suggestion(
                "The drag was aborted: the button was released back at the origin (best-effort) and no drop was committed at the destination. The cursor ends at the origin. Re-check the source state before retrying.",
            )
        })
    }

    pub fn synthesize_drag_to_pid(params: DragParams, pid: i32) -> Result<(), AdapterError> {
        let window_id = crate::system::cg_window::find_cg_window_id_for_pid(pid);
        tracing::debug!(
            "mouse-to-pid: drag pid={} window_id={:?} ({:.0},{:.0}) -> ({:.0},{:.0}) duration={}ms",
            pid,
            window_id,
            params.from.x,
            params.from.y,
            params.to.x,
            params.to.y,
            params.duration_ms.unwrap_or(300)
        );
        let from = CGPoint::new(params.from.x, params.from.y);
        let to = CGPoint::new(params.to.x, params.to.y);
        let duration_ms = params.duration_ms.unwrap_or(300);
        let steps = (duration_ms / 16).max(4) as usize;
        let step_delay = std::time::Duration::from_millis(duration_ms / steps as u64);

        post_event_to_pid(
            CGEventType::LeftMouseDown,
            from,
            CGMouseButton::Left,
            pid,
            window_id,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(200));

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let x = params.from.x + (params.to.x - params.from.x) * t;
            let y = params.from.y + (params.to.y - params.from.y) * t;
            post_event_to_pid(
                CGEventType::LeftMouseDragged,
                CGPoint::new(x, y),
                CGMouseButton::Left,
                pid,
                window_id,
            )?;
            std::thread::sleep(step_delay);
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
        post_event_to_pid(
            CGEventType::LeftMouseUp,
            to,
            CGMouseButton::Left,
            pid,
            window_id,
        )
    }

    fn synthesize_click(
        point: CGPoint,
        cg_button: CGMouseButton,
        button: &MouseButton,
        count: u32,
    ) -> Result<(), AdapterError> {
        let down_ty = down_type(button);
        let up_ty = up_type(button);
        for i in 1..=count {
            let down = create_event(down_ty, point, cg_button)?;
            let up = create_event(up_ty, point, cg_button)?;
            set_click_count(&down, i as i64);
            set_click_count(&up, i as i64);
            down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(10));
            up.post(CGEventTapLocation::HID);
            if i < count {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        Ok(())
    }

    const FIELD_MOUSE_EVENT_NUMBER: u32 = 0;
    const FIELD_MOUSE_PRESSURE: u32 = 2;
    const FIELD_WINDOW_UNDER_MOUSE: u32 = 28;
    const FIELD_WINDOW_UNDER_MOUSE_HANDLER: u32 = 29;
    const FIELD_TARGET_PID: u32 = 39;
    const FIELD_SOURCE_USER_DATA: u32 = 42;

    static MOUSE_EVENT_COUNTER: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

    fn set_click_count(event: &CGEvent, count: i64) {
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, count);
    }

    fn set_window_under_pointer(event: &CGEvent, window_id: u32) {
        unsafe {
            let ptr = event.as_ptr() as *const std::ffi::c_void;
            CGEventSetIntegerValueField(ptr, FIELD_WINDOW_UNDER_MOUSE, window_id as i64);
            CGEventSetIntegerValueField(ptr, FIELD_WINDOW_UNDER_MOUSE_HANDLER, window_id as i64);
        }
    }

    fn stamp_pid_targeted(event: &CGEvent, pid: i32, is_down: bool) {
        unsafe {
            let ptr = event.as_ptr() as *const std::ffi::c_void;
            let ev_num = MOUSE_EVENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            CGEventSetIntegerValueField(ptr, FIELD_MOUSE_EVENT_NUMBER, ev_num);
            CGEventSetIntegerValueField(ptr, FIELD_MOUSE_PRESSURE, if is_down { 1 } else { 0 });
            CGEventSetIntegerValueField(ptr, FIELD_TARGET_PID, pid as i64);
            CGEventSetIntegerValueField(ptr, FIELD_SOURCE_USER_DATA, 1);
        }
    }

    fn post_to_pid(event: &CGEvent, pid: i32) {
        unsafe {
            CGEventPostToPid(pid, event.as_ptr() as *const std::ffi::c_void);
        }
    }

    unsafe extern "C" {
        fn CGEventSetIntegerValueField(event: *const std::ffi::c_void, field: u32, value: i64);
        fn CGEventPostToPid(pid: i32, event: *const std::ffi::c_void);
    }

    fn create_event(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<CGEvent, AdapterError> {
        let source = event_source()?;
        create_event_with_source(&source, event_type, point, button)
    }

    fn create_event_with_source(
        source: &CGEventSource,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<CGEvent, AdapterError> {
        CGEvent::new_mouse_event(source.clone(), event_type, point, button)
            .map_err(|()| AdapterError::internal("CGEvent::new_mouse_event failed"))
    }

    fn event_source() -> Result<CGEventSource, AdapterError> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|()| AdapterError::internal("Failed to create CGEventSource"))
    }

    fn post_event(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<(), AdapterError> {
        let ev = create_event(event_type, point, button)?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_event_with_source(
        source: &CGEventSource,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<(), AdapterError> {
        let ev = create_event_with_source(source, event_type, point, button)?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_event_to_pid(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
        pid: i32,
        window_id: Option<u32>,
    ) -> Result<(), AdapterError> {
        let ev = create_event(event_type, point, button)?;
        let is_down = matches!(
            event_type,
            CGEventType::LeftMouseDown
                | CGEventType::RightMouseDown
                | CGEventType::OtherMouseDown
                | CGEventType::LeftMouseDragged
                | CGEventType::RightMouseDragged
                | CGEventType::OtherMouseDragged
        );
        stamp_pid_targeted(&ev, pid, is_down);
        if let Some(wid) = window_id {
            set_window_under_pointer(&ev, wid);
        }
        post_to_pid(&ev, pid);
        Ok(())
    }

    fn synthesize_click_to_pid(
        point: CGPoint,
        cg_button: CGMouseButton,
        button: &MouseButton,
        count: u32,
        pid: i32,
        window_id: Option<u32>,
    ) -> Result<(), AdapterError> {
        let down_ty = down_type(button);
        let up_ty = up_type(button);
        for i in 1..=count {
            let down = create_event(down_ty, point, cg_button)?;
            let up = create_event(up_ty, point, cg_button)?;
            set_click_count(&down, i as i64);
            set_click_count(&up, i as i64);
            stamp_pid_targeted(&down, pid, true);
            stamp_pid_targeted(&up, pid, false);
            if let Some(wid) = window_id {
                set_window_under_pointer(&down, wid);
                set_window_under_pointer(&up, wid);
            }
            post_to_pid(&down, pid);
            std::thread::sleep(std::time::Duration::from_millis(10));
            post_to_pid(&up, pid);
            if i < count {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        Ok(())
    }

    fn to_cg_button(button: &MouseButton) -> CGMouseButton {
        match button {
            MouseButton::Left => CGMouseButton::Left,
            MouseButton::Right => CGMouseButton::Right,
            MouseButton::Middle => CGMouseButton::Center,
        }
    }

    fn down_type(button: &MouseButton) -> CGEventType {
        match button {
            MouseButton::Left => CGEventType::LeftMouseDown,
            MouseButton::Right => CGEventType::RightMouseDown,
            MouseButton::Middle => CGEventType::OtherMouseDown,
        }
    }

    fn up_type(button: &MouseButton) -> CGEventType {
        match button {
            MouseButton::Left => CGEventType::LeftMouseUp,
            MouseButton::Right => CGEventType::RightMouseUp,
            MouseButton::Middle => CGEventType::OtherMouseUp,
        }
    }

    pub fn synthesize_scroll_at(x: f64, y: f64, dy: i32, dx: i32) -> Result<(), AdapterError> {
        tracing::debug!("mouse: scroll at ({x:.0},{y:.0}) dy={dy} dx={dx}");
        use core_graphics::geometry::CGPoint;

        unsafe extern "C" {
            fn CGEventCreateScrollWheelEvent(
                source: *const std::ffi::c_void,
                units: u32,
                wheel_count: u32,
                wheel1: i32,
                wheel2: i32,
            ) -> *mut std::ffi::c_void;
            fn CGEventSetLocation(event: *mut std::ffi::c_void, point: CGPoint);
            fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        }

        let event = unsafe {
            CGEventCreateScrollWheelEvent(std::ptr::null(), ScrollEventUnit::LINE, 2, dy, dx)
        };
        if event.is_null() {
            return Err(AdapterError::internal("scroll event creation failed"));
        }
        unsafe {
            CGEventSetLocation(event, CGPoint::new(x, y));
            CGEventPost(0, event);
            core_foundation::base::CFRelease(event as _);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn synthesize_mouse(_event: MouseEvent) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    pub fn synthesize_mouse_to_pid(_event: MouseEvent, _pid: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event_to_pid"))
    }

    pub fn synthesize_drag(_params: DragParams) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    pub fn synthesize_drag_to_pid(_params: DragParams, _pid: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag_to_pid"))
    }

    pub fn synthesize_scroll_at(_x: f64, _y: f64, _dy: i32, _dx: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll"))
    }
}

pub use imp::{
    synthesize_drag, synthesize_drag_to_pid, synthesize_mouse, synthesize_mouse_to_pid,
    synthesize_scroll_at,
};
