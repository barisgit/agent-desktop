use serde_json::{Value, json};

use crate::{
    AppError, BackgroundKeyInput, InteractionLease, RefEntry, WindowInfo,
    action::Action,
    adapter::PlatformAdapter,
    commands::{
        background_delivery::{self, ref_window},
        combo::{ensure_combo_allowed, parse_combo_normalized},
        helpers, window_target,
    },
    context::CommandContext,
};

const MAX_TEXT_LEN: usize = 10_000;

/// Fixed allowance for activation, settling, and the focus guard's watch on
/// top of the resolution timeout.
const DELIVERY_BUDGET_MS: u64 = 2_000;

/// Allowance per character of background text. macOS paces text at about
/// 16 ms per character; this is generous so the deadline never compresses
/// that pacing.
const TEXT_BUDGET_PER_CHAR_MS: u64 = 25;

/// Which window receives the keys.
pub enum BackgroundKeyboardTarget {
    /// A snapshot ref. The process and exact window come from the ref, and
    /// the element is given accessibility focus before the keys are posted.
    Ref {
        ref_id: String,
        snapshot_id: Option<String>,
    },
    /// An explicitly named window; the keys reach whatever that window has
    /// focused.
    Window { window_id: String },
}

pub enum BackgroundKeyboardInput {
    Press { combo: String, force: bool },
    Type { text: String },
}

pub struct BackgroundKeyboardArgs {
    pub input: BackgroundKeyboardInput,
    pub target: BackgroundKeyboardTarget,
    pub timeout_ms: Option<u64>,
}

/// Opt-in background keyboard delivery shared by `press` and `type` when they
/// run with `--background`.
///
/// Keys are posted to the process that owns one exact window, so the app is
/// never activated, the pointer never moves, and the user's frontmost app
/// keeps keyboard focus. Unlike the default `press --app` path this never
/// matches app menu items and never requires the app to report a focused
/// element: inactive Electron apps answer `AXFocusedUIElement` unreliably, so
/// that check would refuse deliveries that land. A ref only adds a
/// best-effort accessibility focus on its element. The app decides what the
/// keys do, so success is `delivered_unverified` and the effect must be
/// observed with a fresh snapshot.
pub fn execute(
    args: BackgroundKeyboardArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    background_delivery::reject_headed(context)?;
    helpers::validate_post_action_wait(context)?;
    let (input, mut response) = key_input(args.input, adapter)?;
    let deadline = delivery_deadline(args.timeout_ms, &input)?;

    let lease = adapter.acquire_interaction_lease(deadline)?;
    let (expected, entry) = resolve_target(args.target, adapter, context)?;
    let window = window_target::revalidate_window_for_mutation(adapter, &expected, &lease)?;
    let ax_focus = match &entry {
        Some(entry) => Some(focus_ref(entry, adapter, context, &lease)?),
        None => None,
    };
    let report = adapter.background_key_input(&window, &input, &lease)?;
    drop(lease);

    response = background_delivery::attach_report(response, &window, &report);
    if let Some(ax_focus) = ax_focus {
        response["background"]["ax_focus"] = ax_focus;
    }
    helpers::apply_post_action_wait(response, entry.as_ref(), adapter, context)
}

/// Validates the input before anything is resolved or posted and returns it
/// with the start of the success response.
fn key_input(
    input: BackgroundKeyboardInput,
    adapter: &dyn PlatformAdapter,
) -> Result<(BackgroundKeyInput, Value), AppError> {
    match input {
        BackgroundKeyboardInput::Press { combo, force } => {
            let parsed = parse_combo_normalized(&combo)?;
            ensure_combo_allowed(&parsed, &combo, force, adapter)?;
            let response = json!({ "pressed": true, "combo": combo });
            Ok((BackgroundKeyInput::Combo(parsed), response))
        }
        BackgroundKeyboardInput::Type { text } => {
            if text.is_empty() {
                return Err(AppError::invalid_input("Text to type must not be empty"));
            }
            if text.len() > MAX_TEXT_LEN {
                return Err(AppError::invalid_input(format!(
                    "Text exceeds maximum length of {MAX_TEXT_LEN} bytes"
                )));
            }
            let response = json!({ "typed": true, "characters": text.chars().count() });
            Ok((BackgroundKeyInput::Text(text), response))
        }
    }
}

fn delivery_deadline(
    timeout_ms: Option<u64>,
    input: &BackgroundKeyInput,
) -> Result<crate::Deadline, AppError> {
    let characters = match input {
        BackgroundKeyInput::Combo(_) => 0,
        BackgroundKeyInput::Text(text) => text.chars().count() as u64,
    };
    let resolution_ms = timeout_ms.unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS);
    let total_ms = resolution_ms
        .saturating_add(DELIVERY_BUDGET_MS)
        .saturating_add(characters.saturating_mul(TEXT_BUDGET_PER_CHAR_MS));
    crate::Deadline::after(total_ms).map_err(AppError::Adapter)
}

fn resolve_target(
    target: BackgroundKeyboardTarget,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(WindowInfo, Option<RefEntry>), AppError> {
    match target {
        BackgroundKeyboardTarget::Ref {
            ref_id,
            snapshot_id,
        } => {
            let entry = helpers::load_ref_entry(&ref_id, snapshot_id.as_deref(), context)?;
            Ok((ref_window(&entry)?, Some(entry)))
        }
        BackgroundKeyboardTarget::Window { window_id } => {
            let mut window =
                window_target::resolve_window_for_app(None, Some(&window_id), adapter)?;
            window.title.clear();
            Ok((window, None))
        }
    }
}

/// Resolving the ref is a gate: a stale ref means the target changed, and
/// typing into whatever the window focuses now could hit the wrong field.
/// Accessibility focus itself is best effort, because Chromium-based apps
/// often reject or fail to confirm `AXFocused` on inactive windows while
/// still delivering keys to the focused DOM element.
fn focus_ref(
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    lease: &InteractionLease,
) -> Result<Value, AppError> {
    let handle = helpers::resolve_handle_within_deadline(adapter, entry, lease.deadline())?;
    let focused = adapter.execute_action(&handle, context.request_base(Action::SetFocus), lease);
    Ok(match focused {
        Ok(_) => json!({ "status": "set" }),
        Err(error) => json!({
            "status": "failed",
            "code": error.code,
            "message": error.message,
        }),
    })
}

#[cfg(test)]
#[path = "background_keyboard_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "background_keyboard_tests.rs"]
mod tests;
