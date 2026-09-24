use agent_desktop_core::{
    AppError, PlatformAdapter,
    commands::{
        background_keyboard::{
            BackgroundKeyboardArgs, BackgroundKeyboardInput, BackgroundKeyboardTarget, execute,
        },
        helpers,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::actions::{PressArgs, TypeArgs};

/// `press --background` names its window with `--window-id` because a bare
/// combo has no ref to derive one from; `--app` is rejected rather than
/// ignored since background keys never resolve, focus, or search an app.
pub(super) fn press(
    args: PressArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if args.app.is_some() {
        return Err(AppError::invalid_input_with_suggestion(
            "--app cannot be combined with press --background",
            "Name the exact window with --window-id (from list-windows) instead of --app.",
        ));
    }
    let Some(window_id) = args.window_id else {
        return Err(AppError::invalid_input_with_suggestion(
            "press --background requires --window-id",
            "Run list-windows and pass the target window, e.g. --window-id w-15592.",
        ));
    };
    execute(
        BackgroundKeyboardArgs {
            input: BackgroundKeyboardInput::Press {
                combo: args.combo,
                force: args.force,
            },
            target: BackgroundKeyboardTarget::Window { window_id },
            timeout_ms: None,
        },
        adapter,
        context,
    )
}

pub(super) fn type_text(
    args: TypeArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute(
        BackgroundKeyboardArgs {
            input: BackgroundKeyboardInput::Type { text: args.text },
            target: BackgroundKeyboardTarget::Ref {
                ref_id: args.ref_id,
                snapshot_id: args.snapshot,
            },
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

#[cfg(test)]
#[path = "background_keyboard_tests.rs"]
mod tests;
