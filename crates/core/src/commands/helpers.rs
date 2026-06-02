use crate::{
    action::WindowOp,
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    commands::{wait_selector, wait_selector::WaitSelectorInput},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
    node::WindowInfo,
    refs::{RefEntry, validate_ref_id},
    refs_store::RefStore,
    resolved_element::ResolvedElement,
    window_lookup,
};
use serde_json::{Value, json};

/// Resolves the target pid for a raw mouse / drag command using the
/// explicit `--target-pid` and `--target-app` flags, considering the
/// requested `InteractionPolicy`.
///
/// - `target_pid` and `target_app` are mutually exclusive; passing both
///   returns `INVALID_ARGS`. (Clap also enforces this on the CLI, but
///   batch JSON bypasses clap, so the check lives here.)
/// - Otherwise `target_pid`, when set, is returned directly.
/// - Otherwise looks up `target_app` case-insensitively via
///   `list_apps`. Exactly one match returns `Some(pid)`. Zero matches
///   returns `APP_NOT_FOUND`. Multiple matches return `INVALID_ARGS`
///   with the candidate pids in `platform_detail`.
/// - When neither flag is set the function returns `Ok(None)` under
///   physical / focus-fallback policies (broadcast HID retains the
///   existing behavior). Under `headless()` it returns `INVALID_ARGS`
///   pointing at `--target-app` / `--target-pid` because broadcasting
///   would defeat the policy.
pub fn resolve_raw_mouse_target_pid(
    target_pid: Option<i32>,
    target_app: Option<&str>,
    policy: InteractionPolicy,
    adapter: &dyn PlatformAdapter,
) -> Result<Option<i32>, AppError> {
    if target_pid.is_some() && target_app.is_some() {
        return Err(AppError::invalid_input_with_suggestion(
            "target-pid and target-app are mutually exclusive",
            "Pass either --target-pid <pid> or --target-app <name>, not both",
        ));
    }
    if let Some(pid) = target_pid {
        return Ok(Some(pid));
    }
    if let Some(name) = target_app {
        let apps = adapter.list_apps()?;
        let matches: Vec<_> = apps
            .into_iter()
            .filter(|a| a.name.eq_ignore_ascii_case(name))
            .collect();
        return match matches.len() {
            0 => Err(AppError::Adapter(
                crate::error::AdapterError::new(
                    crate::error::ErrorCode::AppNotFound,
                    format!("App '{name}' not found"),
                )
                .with_suggestion("Run 'list-apps' to see running applications"),
            )),
            1 => Ok(Some(matches[0].pid)),
            _ => {
                let pids = matches
                    .iter()
                    .map(|a| format!("{}#{}", a.name, a.pid))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(AppError::Adapter(
                    crate::error::AdapterError::new(
                        crate::error::ErrorCode::InvalidArgs,
                        format!("App name '{name}' matches multiple processes; use --target-pid"),
                    )
                    .with_platform_detail(pids)
                    .with_suggestion("Pass --target-pid <pid> to disambiguate"),
                ))
            }
        };
    }
    if policy == InteractionPolicy::headless() {
        return Err(AppError::invalid_input_with_suggestion(
            "Headless mouse commands require --target-app or --target-pid",
            "Pass --target-app <name> or --target-pid <pid>, or use --policy physical",
        ));
    }
    Ok(None)
}

/// Variant of [`resolve_raw_mouse_target_pid`] that uses a ref-derived pid as
/// an implicit target under `--policy headless` when no explicit
/// `--target-pid`/`--target-app` was provided. Explicit flags always win.
/// Under `physical` and `focus_fallback` policies this behaves identically to
/// [`resolve_raw_mouse_target_pid`] and ignores `ref_pid` so broadcast HID
/// semantics are preserved.
pub fn resolve_raw_mouse_target_pid_with_ref(
    target_pid: Option<i32>,
    target_app: Option<&str>,
    ref_pid: Option<i32>,
    policy: InteractionPolicy,
    adapter: &dyn PlatformAdapter,
) -> Result<Option<i32>, AppError> {
    if target_pid.is_none() && target_app.is_none() && policy == InteractionPolicy::headless() {
        if let Some(pid) = ref_pid {
            return Ok(Some(pid));
        }
    }
    resolve_raw_mouse_target_pid(target_pid, target_app, policy, adapter)
}

pub struct AppArgs {
    pub app: Option<String>,
}

pub struct RefArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
}

pub struct RefClickArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub policy: InteractionPolicy,
}

impl From<RefClickArgs> for RefArgs {
    fn from(args: RefClickArgs) -> Self {
        Self {
            ref_id: args.ref_id,
            snapshot_id: args.snapshot_id,
        }
    }
}

pub(crate) fn resolve_ref_with_context<'a>(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &'a dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(RefEntry, ResolvedElement<'a>), AppError> {
    validate_ref_id(ref_id)?;
    let store = RefStore::for_session(context.session_id())?;
    context.trace_lazy(
        "ref.resolve.start",
        || json!({ "ref": ref_id, "snapshot_id": snapshot_id }),
    )?;
    let refmap = store.load(snapshot_id).inspect_err(|e| {
        tracing::debug!("refmap load failed: {e}");
        let _ = context.trace_lazy("ref.resolve.error", || {
            json!({
                "ref": ref_id,
                "snapshot_id": snapshot_id,
                "code": e.code(),
                "message": e.to_string()
            })
        });
    })?;
    let entry = match refmap.get(ref_id) {
        Some(entry) => entry.clone(),
        None => {
            context.trace_lazy("ref.resolve.error", || {
                json!({
                    "ref": ref_id,
                    "snapshot_id": snapshot_id,
                    "code": "STALE_REF",
                    "message": "ref not found in current RefMap"
                })
            })?;
            return Err(AppError::stale_ref(ref_id));
        }
    };
    tracing::debug!(
        "resolve: {} -> pid={} role={} name_chars={:?}",
        ref_id,
        entry.pid,
        entry.role,
        entry.name.as_deref().map(|name| name.chars().count())
    );
    context.trace_lazy("ref.resolve.entry", || {
        json!({
            "ref": ref_id,
            "pid": entry.pid,
            "role": entry.role,
            "name": entry.name
        })
    })?;
    let handle = adapter.resolve_element_strict(&entry).inspect_err(|err| {
        let _ = context.trace_lazy("ref.resolve.error", || {
            json!({
                "ref": ref_id,
                "snapshot_id": snapshot_id,
                "code": err.code.as_str(),
                "message": err.message.clone(),
                "details": err.details.clone()
            })
        });
    })?;
    tracing::debug!("resolve: {} resolved successfully", ref_id);
    context.trace_lazy("ref.resolve.ok", || json!({ "ref": ref_id }))?;
    Ok((entry, ResolvedElement::new(adapter, handle)))
}

pub(crate) fn resolve_app_pid(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<i32, AppError> {
    if let Some(name) = app {
        let apps = adapter.list_apps()?;
        apps.into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.pid)
            .ok_or_else(|| AppError::invalid_input(format!("App '{name}' not found")))
    } else {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = adapter.list_windows(&filter)?;
        windows
            .first()
            .map(|w| w.pid)
            .ok_or_else(|| AppError::invalid_input("No focused window. Use --app to specify."))
    }
}

pub(crate) fn execute_ref_action_with_context(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, result) = execute_ref_action_result_with_context(
        &args.ref_id,
        args.snapshot_id.as_deref(),
        adapter,
        request,
        context,
    )?;
    apply_post_action_wait(serde_json::to_value(result)?, &entry, adapter, context)
}

/// Resolves the app name a ref belongs to for post-action polling. Normal
/// refmaps always carry `source_app`; the pid lookup is a fallback for legacy
/// or partially-populated entries so the wait never silently polls the focused
/// window instead of the acted-on app.
pub(crate) fn probe_app_name(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> Option<String> {
    if entry.source_app.is_some() {
        return entry.source_app.clone();
    }
    window_lookup::find_window_for_pid(entry.pid, adapter)
        .ok()
        .map(|window| window.app)
}

pub(crate) fn apply_post_action_wait(
    result: Value,
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let Some(wait) = context.wait_selector() else {
        return Ok(result);
    };
    match wait_selector::execute(
        WaitSelectorInput {
            query_raw: wait.query_raw.clone(),
            gone: wait.gone,
            app: probe_app_name(adapter, entry),
            window_id: entry.source_window_id.clone(),
            opts: TreeOptions::default(),
            timeout_ms: wait.timeout_ms,
        },
        adapter,
        context,
    ) {
        Ok(mut snapshot) => {
            if let Some(body) = snapshot.as_object_mut() {
                body.insert("after_action".into(), result);
            }
            Ok(snapshot)
        }
        Err(AppError::Adapter(mut adapter_err)) => {
            let mut details = adapter_err.details.take().unwrap_or_else(|| json!({}));
            if let Some(obj) = details.as_object_mut() {
                obj.insert("after_action".into(), result);
            }
            Err(AppError::Adapter(adapter_err.with_details(details)))
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn execute_ref_action_result_with_context(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<(RefEntry, ActionResult), AppError> {
    let (entry, handle) = resolve_ref_with_context(ref_id, snapshot_id, adapter, context)?;
    let result = crate::ref_action::execute_resolved(
        crate::ref_action::ResolvedRefAction {
            adapter,
            entry: &entry,
            handle: handle.handle(),
            ref_id,
            context,
        },
        request,
    )?;
    Ok((entry, result))
}

pub(crate) fn window_op_command(
    args: AppArgs,
    adapter: &dyn PlatformAdapter,
    op: WindowOp,
    response_key: &'static str,
) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let win = match window_lookup::find_window_for_pid(pid, adapter) {
        Ok(win) => win,
        Err(_) if matches!(op, WindowOp::Restore) => WindowInfo {
            id: String::new(),
            title: String::new(),
            app: args.app.unwrap_or_default(),
            pid,
            bounds: None,
            is_focused: false,
        },
        Err(err) => return Err(err),
    };
    adapter.window_op(&win, op)?;
    Ok(json!({ response_key: true }))
}

pub fn find_window_for_pid(
    pid: i32,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    window_lookup::find_window_for_pid(pid, adapter)
}

pub fn resolve_window_by_id(
    window_id: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    adapter
        .list_windows(&filter)?
        .into_iter()
        .find(|window| window.id == window_id)
        .ok_or_else(|| AppError::invalid_input(format!("Window '{window_id}' not found")))
}

pub(crate) fn resolve_window_for_app(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let pid = resolve_app_pid(app, adapter)?;
    window_lookup::find_window_for_pid(pid, adapter)
}

#[cfg(test)]
#[path = "helpers_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "helpers_ref_action_tests.rs"]
mod ref_action_tests;
