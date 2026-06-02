use agent_desktop_core::{
    action::{DragParams, MouseButton, MouseEvent, MouseEventKind, Point},
    error::AdapterError,
};

use super::mouse::{synthesize_drag_to_pid, synthesize_mouse_to_pid};

#[test]
fn synthesize_mouse_to_pid_self_pid_smoke() {
    let pid = std::process::id() as i32;
    let event = MouseEvent {
        kind: MouseEventKind::Move,
        button: MouseButton::Left,
        point: Point { x: 0.0, y: 0.0 },
    };
    let _ = synthesize_mouse_to_pid(event, pid);
}

#[test]
fn synthesize_drag_to_pid_symbol_referenced() {
    let _ = synthesize_drag_to_pid as fn(DragParams, i32) -> Result<(), AdapterError>;
}
