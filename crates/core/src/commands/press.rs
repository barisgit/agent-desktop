use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::{
        combo::{ensure_combo_allowed, parse_combo_normalized},
        helpers::{resolve_raw_mouse_target_pid, resolve_ref_with_context, resolve_window_by_id},
    },
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::Value;

pub struct PressArgs {
    pub combo: String,
    pub ref_id: Option<String>,
    pub snapshot: Option<String>,
    pub window_id: Option<String>,
    pub app: Option<String>,
    pub target_app: Option<String>,
    pub target_pid: Option<i32>,
    pub policy: InteractionPolicy,
    pub force: bool,
}

pub fn execute(
    args: PressArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;
    let request = context.request(Action::PressKey(combo.clone()), args.policy);
    let policy = request.policy;
    let headless = policy == InteractionPolicy::headless();

    if let Some(ref_id) = &args.ref_id {
        let (_, handle) =
            resolve_ref_with_context(ref_id, args.snapshot.as_deref(), adapter, context)?;
        let result = adapter.press_key_at_element(handle.handle(), &combo)?;
        return Ok(serde_json::to_value(result)?);
    }

    if let Some(window_id) = &args.window_id {
        let window = resolve_window_by_id(window_id, adapter)?;
        let result = adapter.press_key_for_window(&window, &combo, !headless)?;
        return Ok(serde_json::to_value(result)?);
    }

    let target_pid = if args.target_pid.is_some() || args.target_app.is_some() {
        resolve_raw_mouse_target_pid(args.target_pid, args.target_app.as_deref(), policy, adapter)?
    } else if headless && args.app.is_some() {
        resolve_raw_mouse_target_pid(None, args.app.as_deref(), policy, adapter)?
    } else if headless {
        return Err(AppError::invalid_input_with_suggestion(
            "Headless press requires --target-app, --target-pid, --window-id, or a ref",
            "Pass a target or rerun with --headed",
        ));
    } else {
        None
    };

    if let Some(pid) = target_pid {
        let result = adapter.press_key_for_pid(pid, &combo, !headless)?;
        return Ok(serde_json::to_value(result)?);
    }

    if let Some(app_name) = &args.app {
        let result = adapter.press_key_for_app(app_name, &combo)?;
        return Ok(serde_json::to_value(result)?);
    }

    let handle = crate::adapter::NativeHandle::null();
    let result = adapter.execute_action(&handle, request)?;
    Ok(serde_json::to_value(result)?)
}

#[cfg(test)]
#[path = "press_tests.rs"]
mod tests;
