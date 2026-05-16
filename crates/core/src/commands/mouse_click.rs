use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::{helpers::resolve_raw_mouse_target_pid, point_resolve::require_cursor_policy},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::{Value, json};

pub struct MouseClickArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub count: u32,
    pub policy: InteractionPolicy,
    pub target_pid: Option<i32>,
    pub target_app: Option<String>,
}

pub fn execute(
    args: MouseClickArgs,
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
        require_cursor_policy(context, "mouse-click")?;
    }
    adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Click { count: args.count },
            point: Point {
                x: args.x,
                y: args.y,
            },
            button: args.button,
        },
        target_pid,
    )?;
    Ok(json!({ "clicked": true, "x": args.x, "y": args.y, "count": args.count }))
}
