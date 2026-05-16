use crate::{
    action::DragParams,
    adapter::PlatformAdapter,
    commands::{
        helpers::resolve_raw_mouse_target_pid_with_ref,
        point_resolve::{
            PointResolveArgs, focus_for_physical_input, require_cursor_policy,
            resolve_point_from_ref_or_xy_with_context,
        },
    },
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::{Value, json};

pub struct DragArgs {
    pub from_ref: Option<String>,
    pub from_xy: Option<(f64, f64)>,
    pub to_ref: Option<String>,
    pub to_xy: Option<(f64, f64)>,
    pub snapshot_id: Option<String>,
    pub duration_ms: Option<u64>,
    pub drop_delay_ms: Option<u64>,
    pub policy: InteractionPolicy,
    pub target_pid: Option<i32>,
    pub target_app: Option<String>,
}

pub fn execute(
    args: DragArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let from = resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: args.from_ref.as_deref(),
            xy: args.from_xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide --from <ref> or --from-xy x,y",
        },
        adapter,
        context,
    )?;
    let to = resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: args.to_ref.as_deref(),
            xy: args.to_xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide --to <ref> or --to-xy x,y",
        },
        adapter,
        context,
    )?;
    let ref_pid = match (from.pid, to.pid) {
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        _ => None,
    };
    let target_pid = resolve_raw_mouse_target_pid_with_ref(
        args.target_pid,
        args.target_app.as_deref(),
        ref_pid,
        args.policy,
        adapter,
    )?;
    let focused = if target_pid.is_none() {
        require_cursor_policy(context, "drag")?;
        focus_for_physical_input(from.pid, adapter, context)?
    } else {
        false
    };
    let params = DragParams {
        from: from.point.clone(),
        to: to.point.clone(),
        duration_ms: args.duration_ms,
        drop_delay_ms: args.drop_delay_ms,
    };
    adapter.drag(params, target_pid)?;
    let mut response = json!({
        "dragged": true,
        "from": { "x": from.point.x, "y": from.point.y },
        "to": { "x": to.point.x, "y": to.point.y }
    });
    if let Some(drop_delay_ms) = args.drop_delay_ms {
        response["drop_delay_ms"] = json!(drop_delay_ms);
    }
    if focused {
        response["focused"] = json!(true);
    }
    Ok(response)
}

#[cfg(test)]
#[path = "drag_tests.rs"]
mod tests;
