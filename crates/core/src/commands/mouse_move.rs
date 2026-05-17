use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::{helpers::resolve_raw_mouse_target_pid, point_resolve::require_cursor_policy},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::{Value, json};

pub struct MouseMoveArgs {
    pub x: f64,
    pub y: f64,
    pub policy: InteractionPolicy,
    pub target_pid: Option<i32>,
    pub target_app: Option<String>,
}

pub fn execute(
    args: MouseMoveArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let target_pid = resolve_raw_mouse_target_pid(
        args.target_pid,
        args.target_app.as_deref(),
        args.policy,
        adapter,
    )?;
    if target_pid.is_none() {
        require_cursor_policy(context, "mouse-move")?;
    }
    adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: Point {
                x: args.x,
                y: args.y,
            },
            button: MouseButton::Left,
        },
        target_pid,
        args.policy,
    )?;
    Ok(json!({ "moved": true, "x": args.x, "y": args.y }))
}
