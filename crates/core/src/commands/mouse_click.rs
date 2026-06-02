use crate::{
    action::{Action, MouseButton, MouseEvent, MouseEventKind, Point},
    action_request::ActionRequest,
    adapter::{HitTestResult, PlatformAdapter},
    commands::{
        helpers::resolve_raw_mouse_target_pid, hit_test_lookup,
        point_resolve::require_cursor_policy,
    },
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
    refs_store::RefStore,
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

    let point = Point {
        x: args.x,
        y: args.y,
    };
    let hit = adapter
        .hit_test_at_position(point.clone(), target_pid)
        .unwrap_or(None);
    let mut attempted_paths = Vec::new();
    let mut chosen_path = None;

    if let Some(hit) = &hit
        && let Some(label) = ax_action_label(&args.button, args.count, hit)
    {
        attempted_paths.push("ax_press");
        let request = ActionRequest {
            action: ax_action(&args.button),
            policy: args.policy,
        };
        match adapter.execute_ax_only_action(&hit.handle, request) {
            Ok(_) => {
                tracing::debug!(
                    "mouse-click AX {label} succeeded at ({}, {})",
                    args.x,
                    args.y
                );
                chosen_path = Some("ax_press");
            }
            Err(err) => tracing::debug!(
                "mouse-click AX {label} failed ({}); falling back to CGEvent",
                err.message
            ),
        }
    }

    if chosen_path.is_none() {
        let cg_path = cg_path_for(target_pid, args.policy);
        attempted_paths.push(cg_path);
        adapter.mouse_event(
            MouseEvent {
                kind: MouseEventKind::Click { count: args.count },
                point,
                button: args.button.clone(),
            },
            target_pid,
            args.policy,
        )?;
        chosen_path = Some(cg_path);
    }

    let ref_store = RefStore::for_session(context.session_id()).ok();
    let ref_id = hit.as_ref().and_then(|hit| {
        ref_store
            .as_ref()
            .and_then(|store| hit_test_lookup::ref_id_for_hit(hit, store))
    });
    let element = hit
        .as_ref()
        .map(|hit| hit_test_lookup::element_json(hit, ref_id.as_deref()));

    if let Some(hit) = &hit
        && !hit.handle.as_raw().is_null()
    {
        let _ = adapter.release_handle(&hit.handle);
    }

    let mut data = serde_json::Map::new();
    data.insert("clicked".into(), Value::Bool(true));
    data.insert("x".into(), json!(args.x));
    data.insert("y".into(), json!(args.y));
    data.insert("count".into(), json!(args.count));
    if let Some(path) = chosen_path {
        data.insert("path".into(), Value::String(path.into()));
    }
    if let Some(element) = element {
        data.insert("element".into(), element);
    }
    if !attempted_paths.is_empty() {
        data.insert(
            "attempted_paths".into(),
            attempted_paths
                .into_iter()
                .map(|path| Value::String(path.into()))
                .collect(),
        );
    }
    Ok(Value::Object(data))
}

fn ax_action(button: &MouseButton) -> Action {
    match button {
        MouseButton::Right => Action::RightClick,
        _ => Action::Click,
    }
}

fn ax_action_label(button: &MouseButton, count: u32, hit: &HitTestResult) -> Option<&'static str> {
    let actionable: &[&str] = match button {
        MouseButton::Right => &["AXShowMenu", "AXShowAlternateUI"],
        _ => &["AXPress", "AXConfirm", "AXOpen", "AXPick"],
    };
    if count > 1 && !matches!(button, MouseButton::Right) {
        return None;
    }
    hit.available_actions
        .iter()
        .any(|action| actionable.contains(&action.as_str()))
        .then_some(match button {
            MouseButton::Right => "right_click",
            _ => "click",
        })
}

fn cg_path_for(target_pid: Option<i32>, policy: InteractionPolicy) -> &'static str {
    match target_pid {
        Some(_) if policy.allow_focus_steal => "cg_focus_cycle",
        Some(_) => "cg_to_pid",
        None => "cg_broadcast",
    }
}
